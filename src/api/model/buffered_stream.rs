use crate::api::api_utils::get_headers_from_request;
use crate::model::config::ConfigInput;
use actix_web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use bytes::Bytes;
use core::time::Duration;
use reqwest::header::RANGE;
use reqwest::Error;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio_stream::Stream;
use url::Url;
use crate::utils::request_utils::{get_request_headers};

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages.
const ERR_RETRY_TIMEOUT_SECS: u64 = 10; // If connect status is 4xx or 5xx, we wait until we allow next request from client

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
                                 (impl Stream<Item=Result<Bytes, Error>> + Unpin + 'static, Option<Vec<(String, String)>>) {
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
    let (header_sender, mut header_receiver) = mpsc::channel::<Option<Vec<(String, String)>>>(1);
    let first_run_header_sender = Arc::new(header_sender);
    let timeout_header_sender = Arc::clone(&first_run_header_sender);
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
                    // let mut byte_stream = response.bytes_stream();
                    if !status.is_success() {
                        // debug!("Failed connect  {status}");
                        if status.is_client_error() || status.is_server_error() {
                            actix_web::rt::time::sleep(Duration::from_secs(ERR_RETRY_TIMEOUT_SECS)).await;
                            stop_signal.store(true, Ordering::Relaxed);
                        }
                        continue;
                    }
                    if first_run {
                        first_run = false;
                        let headers:  Vec<(String, String)> = response.headers_mut().iter()
                            .filter(|(key, _)| MEDIA_STREAM_HEADERS.contains(&key.as_str()))
                            .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string())).collect();
                        let _ = first_run_header_sender.send(Some(headers)).await;
                    }
                    while !stop_signal.load(Ordering::Relaxed) {
                        match response.chunk().await {
                            Ok(Some(chunk)) => {
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
                            Err(_err) => {
                                // debug!("Media stream error {masked_url} {err:?}");
                                stop_signal.store(true, Ordering::Relaxed);
                                break;
                            }
                            Ok(None) => {
                                // no chunk available
                                // debug!("Media stream finished no data available {masked_url}");
                                stop_signal.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                    drop(response);
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

    actix_rt::spawn(async move {
       actix_web::rt::time::sleep(Duration::from_secs(5)).await;
       let _ = timeout_header_sender.send(None).await;
    });

    let header = header_receiver.recv().await.and_then(|o| o);
    drop(header_receiver);
    (BufferedReceiverStream::new(rx, stop_stream), header)
}

pub fn get_stream_response_with_headers(custom: Option<Vec<(String, String)>>) -> HttpResponseBuilder {
    let mut response_builder = HttpResponse::Ok();
    let mut added_headers: HashSet<String> = HashSet::new();
    if let Some(custom_headers) = custom {
        for header in custom_headers {
            added_headers.insert(header.1.to_string());
            response_builder.insert_header(header);
        }
    }
    if !added_headers.contains(actix_web::http::header::CONTENT_TYPE.as_str()) {
        response_builder.insert_header((actix_web::http::header::CONTENT_TYPE, "application/octet-stream"));
    }
    if !added_headers.contains(actix_web::http::header::CONTENT_LENGTH.as_str()) {
        response_builder.insert_header((actix_web::http::header::CONTENT_LENGTH, 0));
    }
    if !added_headers.contains(actix_web::http::header::CONNECTION.as_str()) {
        response_builder.insert_header((actix_web::http::header::CONNECTION, "close"));
    }
    if !added_headers.contains(actix_web::http::header::CACHE_CONTROL.as_str()) {
        response_builder.insert_header((actix_web::http::header::CACHE_CONTROL, "no-cache"));
    }

    response_builder
}