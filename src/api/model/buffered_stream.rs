use crate::api::api_utils::get_headers_from_request;
use crate::model::config::ConfigInput;
use crate::utils::request_utils::{get_request_headers, mask_sensitive_info};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{HttpRequest, HttpResponseBuilder};
use bytes::Bytes;
use core::time::Duration;
use reqwest::header::RANGE;
use reqwest::{Error, StatusCode};
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use log::debug;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use url::Url;
use crate::debug_if_enabled;

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages. with 8092byte chunks and 2Mbit/s approx 8MB
const ERR_RETRY_TIMEOUT_SECS: u64 = 5; // If connect status is 4xx or 5xx, we wait until we allow next request from client

const MEDIA_STREAM_HEADERS: &[&str] = &["content-type", "content-length", "connection", "accept-ranges", "content-range", "vary"];

fn get_request_range_start_bytes(req_headers: &HashMap<String, Vec<u8>>) -> Option<usize> {
    // range header looks like  bytes=1234-5566/2345345 or bytes=0-
    if let Some(req_range) = req_headers.get(actix_web::http::header::RANGE.as_str()) {
        if let Some(bytes_range) = req_range.strip_prefix(b"bytes=") {
            if let Some(index) = bytes_range.iter().position(|&x| x == b'-') {
                let start_bytes = &bytes_range[..index];
                if let Ok(start_str) = std::str::from_utf8(start_bytes) {
                    if let Ok(bytes_requested) = start_str.parse::<usize>() {
                        return Some(bytes_requested);
                    }
                }
            }
        }
    }
    None
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
        self.stop_signal.store(true, Ordering::Relaxed);
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
        self.close();
    }
}

#[allow(clippy::too_many_lines)]
pub async fn get_buffered_stream(http_client: &Arc<reqwest::Client>, stream_url: &Url,
                                 req: &HttpRequest, input: Option<&ConfigInput>) ->
                                 (impl Stream<Item=Result<Bytes, Error>> + Unpin + 'static, Option<(Vec<(String, String)>, StatusCode)>) {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(STREAM_QUEUE_SIZE);
    let mut req_headers = get_headers_from_request(req);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // we need the range bytes from client request for seek ing to the right position
    let req_range_start_bytes = get_request_range_start_bytes(&req_headers);
    req_headers.remove("range");
    // These are the configured headers for this input.
    let input_headers = input.map(|i| i.headers.clone());
    // The stream url, we need to clone it because of move to async block.
    let url = stream_url.clone();
    // We need an atomic to for signalling the end of the loop,
    let stop_signal_on_provider_disconnect = Arc::new(AtomicBool::new(false));
    // We need a copy for the stream to send a stop signal when client disconnects,
    let stop_signal_on_client_disconnect = Arc::clone(&stop_signal_on_provider_disconnect);
    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));
    let request_client = Arc::clone(http_client);
    // when we first connect to the provider, we need some header information about content-type, range ...
    let (provider_response_sender, mut provider_response_receiver) = mpsc::channel::<Option<(Vec<(String, String)>, StatusCode)>>(1);
    let first_run_response_sender = Arc::new(provider_response_sender);
    // If the reconnect loop is stuck because of provider error we need to reply to the client request,
    // after a time we send the request back without the required header information
    // to avoid deadlock.
    let timeout_provider_response_sender = Arc::clone(&first_run_response_sender);

    actix_rt::spawn(async move {
        let masked_url = if log::log_enabled!(log::Level::Debug) { mask_sensitive_info(url.as_str()) } else { String::new() };
        let range_start_bytes_counter = req_range_start_bytes.map(AtomicUsize::new);
        let mut first_run = true;

        let prepare_client = || {
            let mut client = request_client.get(url.clone()).headers(headers.clone());
            let range_start_bytes_to_request = range_start_bytes_counter.as_ref().map_or(0, |atomic| atomic.load(Ordering::Relaxed));
            if range_start_bytes_to_request > 0 {
                // on reconnect send range header to avoid starting from beginning for vod
                let range = format!("bytes={range_start_bytes_to_request}-", );
                client = client.header(RANGE, range);
                (client, true) // partial content
            } else {
                (client, false)
            }
        };

        while !stop_signal_on_provider_disconnect.load(Ordering::Relaxed) {
            let (client, partial_content) = prepare_client();
            match client.send().await {
                Ok(mut response) => {
                    let status = response.status();
                    if !status.is_success() {
                        debug!("Failed to connect to provider stream. Status:{status} {masked_url}");
                        if status.is_client_error() || status.is_server_error() {
                            // We stop reconnecting, it seems the stream is not available
                            stop_signal_on_provider_disconnect.store(true, Ordering::Relaxed);
                            actix_web::rt::time::sleep(Duration::from_secs(ERR_RETRY_TIMEOUT_SECS)).await;
                        }
                        continue;
                    }
                    if first_run {
                        // Unfortunately, the HEAD request does not work, so we need this workaround.
                        // We need some header information from the provider, we extract the neccessary headers and forard them to the client
                        debug_if_enabled!("Provider response headers: {:?}", response.headers_mut());
                        first_run = false;
                        let headers: Vec<(String, String)> = response.headers_mut().iter()
                            .filter(|(key, _)| MEDIA_STREAM_HEADERS.contains(&key.as_str()))
                            .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string())).collect();
                        // debug!("First  headers {headers:?} {} {}", mask_sensitive_info(url.as_str()));
                        let mut status = response.status();
                        if partial_content && status.is_success() {
                            status = StatusCode::PARTIAL_CONTENT;
                        }
                        let _ = first_run_response_sender.send(Some((headers, status))).await;
                    }
                    let mut byte_stream = response.bytes_stream();
                    while !stop_signal_on_provider_disconnect.load(Ordering::Relaxed) {
                        match byte_stream.next().await {
                            Some(Ok(chunk)) => {
                                if chunk.is_empty() {
                                    debug!("Provider finished stream {masked_url}");
                                    break;
                                }
                                // this is for backpressure, we fill the buffer and wait for the receiver
                                if let Ok(permit) = tx.reserve().await {
                                    let len = chunk.len();
                                    permit.send(Ok(chunk));
                                    // in case of reconnect we can send the start range to the server.
                                    if let Some(bytes) = range_start_bytes_counter.as_ref() {
                                        bytes.fetch_add(len, Ordering::Relaxed);
                                    }
                                } else {
                                    debug!("Client has disconnected from stream {masked_url}");
                                    stop_signal_on_provider_disconnect.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                            Some(Err(err)) => {
                                debug!("Provider stream error {masked_url} {err:?}");
                                stop_signal_on_provider_disconnect.store(true, Ordering::Relaxed);
                                break;
                            }
                            None => {
                                debug!("Provider stream finished no data available {masked_url}");
                                stop_signal_on_provider_disconnect.store(true, Ordering::Relaxed);
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
                    debug!("Provider stream finished with error {masked_url} {err}");
                    stop_signal_on_provider_disconnect.store(true, Ordering::Relaxed);
                    continue;
                }
            }
            actix_web::rt::time::sleep(Duration::from_secs(1)).await;
        }
        debug!("Streaming stopped and no reconnect  for {masked_url}");
        drop(tx);
    });


    let timeout_url = stream_url.clone();
    // We need to reply to the client in case of no connection to the provider to avoid deadlock.
    actix_rt::spawn(async move {
        actix_web::rt::time::sleep(Duration::from_secs(5)).await;
        if !timeout_provider_response_sender.is_closed() {
            let _ = timeout_provider_response_sender.send(None).await;
            debug_if_enabled!("Provider connection is unseccessfull, timeout {}", mask_sensitive_info(timeout_url.as_str()));
        }
    });

    let provider_response = provider_response_receiver.recv().await.and_then(|o| o);
    drop(provider_response_receiver);
    (BufferedReceiverStream::new(rx, stop_signal_on_client_disconnect), provider_response)
}

pub fn get_stream_response_with_headers(custom: Option<(Vec<(String, String)>, StatusCode)>, stream_url: &str) -> HttpResponseBuilder {
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

    let default_headers = vec![
        (actix_web::http::header::CONTENT_TYPE, HeaderValue::from_str("application/octet-stream").unwrap()),
        (actix_web::http::header::CONTENT_LENGTH, HeaderValue::from(0)),
        (actix_web::http::header::CONNECTION, HeaderValue::from_str("keep-alive").unwrap()),
        //(actix_web::http::header::CACHE_CONTROL, HeaderValue::from_str("no-cache").unwrap()),
        (actix_web::http::header::VARY, HeaderValue::from_str("accept-encoding").unwrap())
    ];

    for header in default_headers {
        if !added_headers.contains(header.0.as_str()) {
            headers.push(header);
        }
    }

    headers.push((actix_web::http::header::DATE, HeaderValue::from_str(&chrono::Utc::now().to_rfc2822()).unwrap()));

    let mut response_builder = actix_web::HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());
    debug_if_enabled!("Opening stream {} with status {status}, headers {headers:?}", mask_sensitive_info(stream_url));
    for header in headers {
        response_builder.insert_header(header);
    }
    response_builder
}