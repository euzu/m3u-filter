use crate::model::config::ConfigInput;
use crate::utils::request_utils;
use actix_web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use async_std::stream::StreamExt;
use bytes::Bytes;
use core::time::Duration;
use reqwest::{Error, RequestBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use url::Url;
use crate::utils::request_utils::get_request_headers;

const BUFFER_SIZE: usize = 8092;

pub struct BufferedStreamHandler {
    client: Arc<RequestBuilder>,
    bytes: Arc<Option<AtomicU64>>,
    headers: Arc<HeaderMap>,
}

impl BufferedStreamHandler {
    pub fn new(url: &Url, req: &HttpRequest, input: Option<&ConfigInput>, send_bytes: bool) -> Self {
        let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
        let headers = Arc::new(get_request_headers(input.map(|i| &i.headers), Some(&req_headers)));
        let client = Arc::new(request_utils::get_client_request(input, url, Some(&req_headers)));
        BufferedStreamHandler {
            client,
            bytes: Arc::new(if send_bytes { Some(AtomicU64::new(0)) } else { None }),
            headers
        }
    }

    pub fn get_stream(&mut self) -> impl Stream<Item = Result<Bytes, Error>> + Unpin + 'static {
        let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(BUFFER_SIZE);
        let client = Arc::clone(&self.client);
        let headers = Arc::clone(&self.headers);
        let bytes_counter = Arc::clone(&self.bytes);
        actix_web::rt::spawn({
            async move {
                loop {
                    let Some(client) = client.try_clone() else { break };
                    debug!("Connection to stream");
                    let req_client =  if let Some(bytes)  = bytes_counter.as_ref() {
                        // on reconnect send range header to avoid starting from beginning for vod
                        let mut req_headers = headers.as_ref().clone();
                        let range = format!("bytes={}-", bytes.load(Ordering::Relaxed));
                        req_headers.insert(RANGE, HeaderValue::from_bytes(range.as_bytes()).unwrap());
                        client.headers(req_headers)
                    } else {
                        client
                    };
                    match req_client.send().await {
                        Ok(response) => {
                            if !response.status().is_success() {
                                continue;
                            }
                            let mut byte_stream = response.bytes_stream();
                            while let Some(chunk) = byte_stream.next().await {
                                match chunk {
                                    Ok(chunk) => {
                                        if chunk.is_empty() {
                                            debug!("Stream finished ?");
                                            return;
                                        }
                                        if let Some(bytes) = bytes_counter.as_ref() {
                                            bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                                        }
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            debug!("Stream finished, client disconnect ?");
                                            return;
                                        }
                                    }
                                    Err(err) => {
                                        debug!("Stream disconnected, cant read from server {err}");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if err.is_timeout() {
                                actix_web::rt::time::sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                            debug!("Stream finished {err}");
                            break;
                        }
                    }
                    actix_web::rt::time::sleep(Duration::from_secs(1)).await;
                }
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