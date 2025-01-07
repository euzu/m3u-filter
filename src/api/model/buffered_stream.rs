use crate::model::config::ConfigInput;
use crate::utils::request_utils;
use actix_web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use async_std::stream::StreamExt;
use bytes::Bytes;
use core::time::Duration;
use reqwest::{Error, RequestBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

const BUFFER_SIZE: usize = 8092;

pub struct BufferedStreamHandler {
    client: Arc<RequestBuilder>,
}

impl BufferedStreamHandler {
    pub fn new(url: &Url, req: &HttpRequest, input: Option<&ConfigInput>) -> Self {
        let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
        let client = Arc::new(request_utils::get_client_request(input, url, Some(&req_headers)));

        BufferedStreamHandler {
            client,
        }
    }

    pub fn get_stream(&mut self) -> impl Stream<Item = Result<Bytes, Error>> + Unpin + 'static {
        let (tx, rx) = mpsc::channel::<Result<Bytes, Error>>(BUFFER_SIZE);
        let client = Arc::clone(&self.client);
        actix_web::rt::spawn({
            async move {
                loop {
                    let Some(client) = client.try_clone() else { break };
                    println!("Connection to stream");
                    match client.send().await {
                        Ok(response) => {
                            if !response.status().is_success() {
                                continue;
                            }
                            let mut byte_stream = response.bytes_stream();
                            while let Some(chunk) = byte_stream.next().await {
                                match chunk {
                                    Ok(chunk) => {
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            println!("Stream finished, client disconnect ?");
                                            return;
                                        }
                                    }
                                    Err(err) => {
                                        println!("Stream disconnected, cant read from server {err}");
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
                            println!("Stream finished {err}");
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