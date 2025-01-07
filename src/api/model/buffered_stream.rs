use crate::model::config::ConfigInput;
use crate::utils::request_utils;
use crate::utils::request_utils::{get_request_headers, mask_sensitive_info};
use actix_web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use async_std::stream::StreamExt;
use bytes::Bytes;
use core::time::Duration;
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use reqwest::{Error, RequestBuilder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc,};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use url::Url;

const BUFFER_SIZE: usize = 8092;
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 5; // Wait timeout secs for connection when server dropped connection, then retry
const CLIENT_RETRY_TIMEOUT_SECS: u64 = 10; // If connect status is 4xx or 5xx, we wait until we allow next request from client

pub struct BufferedStreamHandler {
    client: Arc<RequestBuilder>,
    bytes: Arc<Option<AtomicUsize>>,
    headers: Arc<HeaderMap>,
    url: String,
}

fn get_request_bytes(req_headers: &HashMap<&str, &[u8]>) -> usize {
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

impl BufferedStreamHandler {
    pub fn new(url: &Url, req: &HttpRequest, input: Option<&ConfigInput>, send_bytes: bool) -> Self {
        let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
        let req_bytes = get_request_bytes(&req_headers);
        let headers = Arc::new(get_request_headers(input.map(|i| &i.headers), Some(&req_headers)));
        let mut builder = request_utils::get_client_request(input, url, Some(&req_headers));
        builder = builder.timeout(Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS));
        let client = Arc::new(builder);
        BufferedStreamHandler {
            url: mask_sensitive_info(url.as_str()),
            client,
            bytes: Arc::new(if send_bytes { Some(AtomicUsize::new(req_bytes)) } else { None }),
            headers,
        }
    }

    pub fn get_stream(&mut self) -> impl Stream<Item=Result<Bytes, Error>> + Unpin + 'static {
        let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(BUFFER_SIZE);
        let client_builder = Arc::clone(&self.client);
        let headers = Arc::clone(&self.headers);
        let bytes_counter = Arc::clone(&self.bytes);
        let url = mask_sensitive_info(&self.url);
        actix_web::rt::spawn({
            async move {
                'outer: loop {
                    let Some(client) = client_builder.try_clone() else {
                        debug!("Cant clone client, exiting reconnect");
                        break;
                    };
                    debug!("Try connection to stream {url}");
                    let bytes_to_request = if let Some(req_bytes) = bytes_counter.as_ref() {
                        req_bytes.load(Ordering::Relaxed)
                    } else {
                        0
                    };
                    let req_client = if bytes_to_request > 0 {
                        // on reconnect send range header to avoid starting from beginning for vod
                        let mut req_headers = headers.as_ref().clone();
                        let range = format!("bytes={bytes_to_request}-", );
                        req_headers.insert(RANGE, HeaderValue::from_bytes(range.as_bytes()).unwrap());
                        client.headers(req_headers)
                    } else {
                        client.headers(headers.as_ref().clone())
                    };

                    match req_client.send().await {
                        Ok(response) => {
                            let status = response.status();
                            if !status.is_success() {
                                debug!("Failed connect  {status}");
                                if status.is_client_error() || status.is_server_error() {
                                    actix_web::rt::time::sleep(Duration::from_secs(CLIENT_RETRY_TIMEOUT_SECS)).await;
                                    break 'outer;
                                }
                                continue;
                            }
                            let mut byte_stream = response.bytes_stream();
                            while let Some(chunk) = byte_stream.next().await {
                                match chunk {
                                    Ok(chunk) => {
                                        if chunk.is_empty() {
                                            debug!("Stream finished ? {url}");
                                            break;
                                        }
                                        if let Some(bytes) = bytes_counter.as_ref() {
                                            bytes.fetch_add(chunk.len(), Ordering::Relaxed);
                                        }
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            debug!("Stream finished, client disconnect ?  {url}");
                                            break 'outer;
                                        }
                                    }
                                    Err(_err) => {
                                        actix_web::rt::time::sleep(Duration::from_millis(100)).await;
                                        // this happens very often, we cant debug this because of flooding
                                        //debug!("stream error, cant read from server  {url} {err} ");
                                        // break 'outer;
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if err.is_timeout() {
                                actix_web::rt::time::sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                            debug!("Stream finished  {url} {err}");
                            break;
                        }
                    }
                    actix_web::rt::time::sleep(Duration::from_secs(1)).await;
                }
                debug!("Reconnecting stream finished {url}");
            }
        });

        ReceiverStream::new(rx)
    }
}


pub fn get_stream_response_with_headers() -> HttpResponseBuilder {
    let mut response_builder = HttpResponse::Ok();
    response_builder.insert_header((actix_web::http::header::CONTENT_TYPE, "application/octet-stream"));
    response_builder.insert_header((actix_web::http::header::CONTENT_LENGTH, 0));
    response_builder.insert_header((actix_web::http::header::CONNECTION, "keep-alive"));
    response_builder.insert_header((actix_web::http::header::CACHE_CONTROL, "no-cache"));

    response_builder
}