use crate::api::api_utils::get_headers_from_request;
use crate::model::config::ConfigInput;
use crate::utils::request_utils;
use crate::utils::request_utils::mask_sensitive_info;
use actix_web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use bytes::Bytes;
use core::time::Duration;
use log::debug;
use reqwest::header::RANGE;
use reqwest::Error;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio_stream::Stream;
use url::Url;

const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages.
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 5; // Wait timeout secs for connection when server dropped connection, then retry
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

pub fn get_buffered_stream(stream_url: &Url, req: &HttpRequest, input: Option<&ConfigInput>, range_send: bool) -> impl Stream<Item=Result<Bytes, Error>> + Unpin + 'static {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(STREAM_QUEUE_SIZE);
    let req_headers = get_headers_from_request(req);
    let input_headers = input.map(|i| i.headers.clone());
    let url = stream_url.clone();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_stream = Arc::clone(&stop_signal);
    actix_rt::spawn(async move {
        let masked_url = mask_sensitive_info(url.as_str());
        let req_bytes = get_request_bytes(&req_headers);
        let bytes_counter = if range_send { Some(AtomicUsize::new(req_bytes)) } else { None };
        while !stop_signal.load(Ordering::Relaxed) {
            let mut client = request_utils::get_client_request(input_headers.as_ref(), &url, Some(&req_headers));
            client = client.timeout(Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS));
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
                    while !stop_signal.load(Ordering::Relaxed) {
                        match response.chunk().await {
                            Ok(Some(chunk)) => {
                                if chunk.is_empty() {
                                    // debug!("Stream finished ? {masked_url}");
                                    break;
                                }
                                if let Ok(permit) = tx.reserve().await {
                                    let len = chunk.len();
                                    permit.send(Ok(chunk));
                                    if let Some(bytes) = bytes_counter.as_ref() {
                                        bytes.fetch_add(len, Ordering::Relaxed);
                                    }
                                } else {
                                    // debug!("Stream finished, client disconnect ?  {masked_url}");
                                    stop_signal.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                            Err(_err) => {
                                stop_signal.store(true, Ordering::Relaxed);
                                break;
                            }
                            Ok(None) => {
                                // no chunk available
                                // debug!("media stream finished no data available");
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
                    } else {
                        // debug!("Stream finished  {masked_url} {err}");
                        stop_signal.store(true, Ordering::Relaxed);
                    }
                    continue;
                }
            }
            actix_web::rt::time::sleep(Duration::from_secs(1)).await;
        }
        debug!("Reconnecting stream stopped {masked_url}");
        drop(tx);
    });

    BufferedReceiverStream::new(rx, stop_stream)
}


pub fn get_stream_response_with_headers() -> HttpResponseBuilder {
    let mut response_builder = HttpResponse::Ok();
    response_builder.insert_header((actix_web::http::header::CONTENT_TYPE, "application/octet-stream"));
    response_builder.insert_header((actix_web::http::header::CONTENT_LENGTH, 0));
    response_builder.insert_header((actix_web::http::header::CONNECTION, "close"));
    response_builder.insert_header((actix_web::http::header::CACHE_CONTROL, "no-cache"));

    response_builder
}