use crate::api::api_utils::get_headers_from_request;
use crate::model::config::ConfigInput;
use actix_web::{HttpRequest, HttpResponseBuilder};
use bytes::Bytes;
use core::time::Duration;
use reqwest::header::{RANGE};
use reqwest::{Error, StatusCode};
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use actix_web::http::header::{HeaderName, HeaderValue};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use url::Url;
use crate::utils::request_utils::{get_request_headers};

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages.
const ERR_RETRY_TIMEOUT_SECS: u64 = 5; // If connect status is 4xx or 5xx, we wait until we allow next request from client

fn get_request_bytes(req_headers: &HashMap<String, Vec<u8>>) -> usize {
    let mut req_bytes: usize = 0;

    if let Some(req_range) = req_headers.get(actix_web::http::header::RANGE.as_str()) {
        if let Some(bytes_range) = req_range.strip_prefix(b"bytes=") {
            if let Some(index) = bytes_range.iter().position(|&x| x == b'-') {
                let start_bytes = &bytes_range[..index];
                if let Ok(start_str) = std::str::from_utf8(start_bytes) {
                    if let Ok(bytes_requested) = start_str.parse::<usize>() {
                        req_bytes = bytes_requested;
                    }
                }
            }
        }
    }

    req_bytes
}

pub struct BufferedReceiverStream<T> {
    inner: Receiver<T>,
    stop_signal: Arc<AtomicBool>,
}

impl<T> BufferedReceiverStream<T> {
    pub fn new(recv: Receiver<T>, stop_signal: Arc<AtomicBool>) -> Self {
        Self { inner: recv, stop_signal }
    }

    pub fn close(&mut self) {
        self.inner.close();
    }
}

impl<T> Stream for BufferedReceiverStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_recv(cx)
    }
}

impl<T> Drop for BufferedReceiverStream<T> {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        self.close();
    }
}

const MEDIA_STREAM_HEADERS: &[&str] = &["content-type", "content-length","connection", "accept-ranges", "content-range"];

pub async fn get_buffered_stream(http_client: &Arc<reqwest::Client>, stream_url: &Url,
                                 req: &HttpRequest, input: Option<&ConfigInput>, range_send: bool) ->
                                 (impl Stream<Item=Result<Bytes, Error>> + Unpin + 'static, Option<(Vec<(String, String)>, StatusCode)>) {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(STREAM_QUEUE_SIZE);
    let mut req_headers = get_headers_from_request(req);
    let req_bytes = get_request_bytes(&req_headers);
    req_headers.remove("range");
    let input_headers = input.map(|i| i.headers.clone());
    let url = stream_url.clone();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_stream = Arc::clone(&stop_signal);
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));
    let base_client = Arc::clone(http_client);
    let (org_response_sender, mut org_response_receiver) = mpsc::channel::<Option<(Vec<(String, String)>, StatusCode)>>(1);
    let first_run_response_sender = Arc::new(org_response_sender);
    let timeout_org_response_sender = Arc::clone(&first_run_response_sender);

    actix_rt::spawn(async move {
        // let masked_url = mask_sensitive_info(url.as_str());
        let bytes_counter = if range_send { Some(AtomicUsize::new(req_bytes)) } else { None };
        let mut first_run = true;
        while !stop_signal.load(Ordering::Relaxed) {
            let mut client = base_client.get(url.clone()).headers(headers.clone());
            let bytes_to_request = bytes_counter.as_ref().map_or(0, |atomic| atomic.load(Ordering::Relaxed));
            if bytes_to_request > 0 {
                // on reconnect send range header to avoid starting from beginning for vod
                let range = format!("bytes={bytes_to_request}-", );
                client = client.header(RANGE, range);
            }

            match client.send().await {
                Ok(mut response) => {
                    let status = response.status();
                    if !status.is_success() {
                        // debug!("Failed connect  {status}");
                        if status.is_client_error() || status.is_server_error() {
                            stop_signal.store(true, Ordering::Relaxed);
                            actix_web::rt::time::sleep(Duration::from_secs(ERR_RETRY_TIMEOUT_SECS)).await;
                        }
                        continue;
                    }
                    if first_run {
                        first_run = false;
                        let headers:  Vec<(String, String)> = response.headers_mut().iter()
                            .filter(|(key, _)| MEDIA_STREAM_HEADERS.contains(&key.as_str()))
                            .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string())).collect();
                        // debug!("First  headers {headers:?} {} {}", mask_sensitive_info(url.as_str()));
                        let status = response.status();
                        let _ = first_run_response_sender.send(Some((headers, status))).await;
                    }
                    let mut byte_stream = response.bytes_stream();
                    while !stop_signal.load(Ordering::Relaxed) {
                        //match response.chunk().await {
                        match byte_stream.next().await {
                            Some(Ok(chunk)) => {
                                if chunk.is_empty() {
                                    // debug!("Download Stream finished ? {masked_url}");
                                    break;
                                }
                                if let Ok(permit) = tx.reserve().await {
                                    let len = chunk.len();
                                    permit.send(Ok(chunk));
                                    if let Some(bytes) = bytes_counter.as_ref() {
                                        bytes.fetch_add(len, Ordering::Relaxed);
                                    }
                                } else {
                                    // debug!("Client disconnect ?  {masked_url}");
                                    stop_signal.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                            Some(Err(_err)) => {
                                // debug!("Media stream error {masked_url} {err:?}");
                                stop_signal.store(true, Ordering::Relaxed);
                                break;
                            }
                            None => {
                                // no chunk available
                                // debug!("Media stream finished no data available {masked_url}");
                                stop_signal.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                    drop(byte_stream);
                }
                Err(err) => {
                    if err.is_timeout() {
                        actix_web::rt::time::sleep(Duration::from_secs(1)).await;
                    }
                    // debug!("Stream finished  {masked_url} {err}");
                    stop_signal.store(true, Ordering::Relaxed);
                    continue;
                }
            }
            actix_web::rt::time::sleep(Duration::from_secs(1)).await;
        }
        // debug!("Reconnecting stream stopped {masked_url}");
        drop(tx);
    });

    // let url2 = stream_url.clone();

    actix_rt::spawn(async move {
       actix_web::rt::time::sleep(Duration::from_secs(5)).await;
        if !timeout_org_response_sender.is_closed() {
            let _ = timeout_org_response_sender.send(None).await;
            // debug!("Header wait timeout {}", mask_sensitive_info(url2.as_str()));
        }
    });

    let org_response = org_response_receiver.recv().await.and_then(|o| o);
    drop(org_response_receiver);
    // debug!("Opening stream {} {header:?}", mask_sensitive_info(stream_url.as_str()));
    (BufferedReceiverStream::new(rx, stop_stream), org_response)
}

pub fn get_stream_response_with_headers(custom: Option<(Vec<(String, String)>, StatusCode)>) -> HttpResponseBuilder {
    let mut headers = Vec::<(HeaderName, HeaderValue)>::with_capacity(12);
    let mut added_headers: HashSet<String> = HashSet::new();
    let mut status = 200_u16;
    if let Some((custom_headers, status_code)) = custom {
        status = status_code.as_u16();
        for header in custom_headers {
            headers.push((HeaderName::from_str(&header.0).unwrap(), HeaderValue::from_str(header.1.as_str()).unwrap()));
            added_headers.insert(header.0.to_string());
        }
    }

    if !added_headers.contains(actix_web::http::header::CONTENT_TYPE.as_str()) {
        headers.push((actix_web::http::header::CONTENT_TYPE, HeaderValue::from_str("application/octet-stream").unwrap()));
    }
    if !added_headers.contains(actix_web::http::header::CONTENT_LENGTH.as_str()) {
        headers.push((actix_web::http::header::CONTENT_LENGTH, HeaderValue::from(0)));
    }
    if !added_headers.contains(actix_web::http::header::CONNECTION.as_str()) {
        headers.push((actix_web::http::header::CONNECTION, HeaderValue::from_str("close").unwrap()));
    }
    if !added_headers.contains(actix_web::http::header::CACHE_CONTROL.as_str()) {
        headers.push((actix_web::http::header::CACHE_CONTROL, HeaderValue::from_str("no-cache").unwrap()));
    }

    headers.push((actix_web::http::header::DATE, HeaderValue::from_str(&chrono::Utc::now().to_rfc2822()).unwrap()));

    let mut response_builder = actix_web::HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());
    //debug!("Response {status} headers {headers:?}");
    for header in headers {
        response_builder.insert_header(header);
    }
    response_builder
}