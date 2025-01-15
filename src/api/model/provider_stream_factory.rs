use crate::api::api_utils::get_headers_from_request;
use crate::debug_if_enabled;
use crate::model::config::ConfigInput;
use crate::model::playlist::PlaylistItemType;
use crate::utils::request_utils::get_request_headers;
use actix_web::HttpRequest;
use bytes::Bytes;
use futures::stream::{self, BoxStream};
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, RANGE};
use reqwest::{StatusCode};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::{Duration, Instant};
use url::Url;
use crate::api::model::buffered_stream::BufferedStream;
use crate::api::model::model_utils::get_response_headers;

// TODO make this configurable
pub const STREAM_QUEUE_SIZE: usize = 1024; // mpsc channel holding messages. with 8092byte chunks and 2Mbit/s approx 8MB

pub type ResponseStream = BoxStream<'static, Result<Bytes, reqwest::Error>>;
type ResponseInfo = Option<(Vec<(String, String)>, StatusCode)>;
type ProviderStreamResponse = (ResponseStream, ResponseInfo);


struct ClientStream {
    inner: ResponseStream,
    close_signal: Arc<AtomicBool>,
    total_bytes: Arc<Option<AtomicUsize>>,
}

impl ClientStream {
    fn new(inner: ResponseStream, close_signal: Arc<AtomicBool>, total_bytes: Arc<Option<AtomicUsize>>) -> Self {
        Self { inner, close_signal, total_bytes }
    }
}

impl Stream for ClientStream
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if let Some(counter) = self.total_bytes.as_ref() {
                    counter.fetch_add(bytes.len(), Ordering::Relaxed);
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

impl Drop for ClientStream {
    fn drop(&mut self) {
        self.close_signal.store(false, Ordering::Relaxed);
    }
}

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

fn get_client_stream_request_params(
    req: &HttpRequest,
    input: Option<&ConfigInput>,
    options: (PlaylistItemType, bool, bool, usize)) -> (usize, Option<usize>, bool, HeaderMap)
{
    let (_item_type, retry_enabled, buffer_enabled, buffer_size) = options;
    let stream_buffer_size = if buffer_enabled { if buffer_size > 0 { buffer_size } else { STREAM_QUEUE_SIZE } } else { 1 };
    let mut req_headers = get_headers_from_request(req, &None);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // we need the range bytes from client request for seek ing to the right position
    let req_range_start_bytes = get_request_range_start_bytes(&req_headers);
    req_headers.remove("range");
    // These are the configured headers for this input.
    let input_headers = input.map(|i| i.headers.clone());
    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));

    (stream_buffer_size, req_range_start_bytes, retry_enabled, headers)
}

fn prepare_client(request_client: &Arc<reqwest::Client>, url: &Url, headers: &HeaderMap, range_start_bytes_to_request: usize) -> (reqwest::RequestBuilder, bool) {
    let mut client = request_client.get(url.clone()).headers(headers.clone());
    if range_start_bytes_to_request > 0 {
        // on reconnect send range header to avoid starting from beginning for vod
        let range = format!("bytes={range_start_bytes_to_request}-", );
        client = client.header(RANGE, range);
        (client, true) // partial content
    } else {
        (client, false)
    }
}

async fn provider_request(request_client: Arc<reqwest::Client>, url: &Url, initial_info: bool, headers: &HeaderMap, range: usize) -> Option<ProviderStreamResponse> {
    let (client, _partial_content) = prepare_client(&request_client, url, headers, range);
    match client.send().await {
        Ok(mut response) => {
            let status = response.status();
            if status.is_success() {
                let response_info = if initial_info {
                    // Unfortunately, the HEAD request does not work, so we need this workaround.
                    // We need some header information from the provider, we extract the necessary headers and forward them to the client
                    debug_if_enabled!("Provider response headers: {:?}", response.headers_mut());
                    let response_headers: Vec<(String, String)> = get_response_headers(&mut response);
                    // debug!("First  headers {headers:?} {} {}", mask_sensitive_info(url.as_str()));
                    Some((response_headers, response.status()))
                } else {
                    None
                };
                return Some((response.bytes_stream().boxed(), response_info));
            }
        }
        Err(_err) => {}
    }
    None
}

async fn stream_provider(client: Arc<reqwest::Client>, url: &Url, retry: Arc<AtomicBool>, headers: HeaderMap, range: usize) -> Option<ResponseStream> {
    while retry.load(Ordering::Relaxed) {
        let (client, _) = prepare_client(&client, url, &headers, range);
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Some(response.bytes_stream().boxed());
                }
            }
            Err(_err) => {}
        }
        if !retry.load(Ordering::Relaxed) {
            return None;
        }
        actix_web::rt::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

async fn get_initial_stream(client: Arc<reqwest::Client>, url: &Url, reconnect: &Arc<AtomicBool>, headers: &HeaderMap, range: usize) -> Option<ProviderStreamResponse> {
    let start = Instant::now();
    while reconnect.load(Ordering::Relaxed) {
        if let Some(value) = provider_request(Arc::clone(&client), url, true, headers, range).await {
            return Some(value);
        }
        if start.elapsed().as_secs() > 5 {
            return None;
        }
        actix_web::rt::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

// options: (PlaylistItemType, reconnect: bool, buffer: bool, buffer_size: usize)
pub async fn create_provider_stream(client: Arc<reqwest::Client>,
                                    stream_url: &Url,
                                    req: &HttpRequest,
                                    input: Option<&ConfigInput>,
                                    options: (PlaylistItemType, bool, bool, usize)) -> Option<ProviderStreamResponse> {
    let (buffer_size, req_range_start_bytes, reconnect_enabled, headers) = get_client_stream_request_params(req, input, options);
    let range = req_range_start_bytes.map(AtomicUsize::new);
    let reconnect_signal = Arc::new(AtomicBool::new(true));
    let url = stream_url.clone();

    let total_bytes_count = range.as_ref().map_or(0, |atomic| atomic.load(Ordering::Relaxed));
    let range_bytes = Arc::new(range);
    let range_bytes_client = Arc::clone(&range_bytes);

    let client_stream_factory =  |stream, reconnect, range_cnt| {
        let stream = ClientStream::new(stream, reconnect, range_cnt).boxed();
        if buffer_size > 0 {
            BufferedStream::new(stream, buffer_size, Arc::clone(&reconnect_signal), stream_url.as_str()).boxed()
        } else {
            stream
        }
    };

    match get_initial_stream(Arc::clone(&client), &url, &reconnect_signal, &headers, total_bytes_count).await {
        Some((init_stream, info)) => {
            if reconnect_enabled {
                let reconnect = Arc::clone(&reconnect_signal);
                Some((client_stream_factory(init_stream.boxed(), Arc::clone(&reconnect), Arc::clone(&range_bytes)).boxed(), info))
            } else {
                let reconnect = Arc::clone(&reconnect_signal);
                let client_signal = Arc::clone(&reconnect);
                let unfold: ResponseStream = stream::unfold((), move |()| {
                    let url = url.clone();
                    let client = Arc::clone(&client);
                    let provider_signal = Arc::clone(&reconnect);
                    let total_bytes_count = range_bytes_client.as_ref().as_ref().map_or(0, |atomic| atomic.load(Ordering::Relaxed));
                    let headers = headers.clone();
                    async move {
                        let stream = stream_provider(client, &url, Arc::clone(&provider_signal), headers, total_bytes_count).await?;
                        Some((stream, ()))
                    }
                }).flatten().boxed();
                Some((client_stream_factory(init_stream.chain(unfold).boxed(), Arc::clone(&client_signal), Arc::clone(&range_bytes)).boxed(), info))
            }
        }
        None => None
    }
}

#[cfg(test)]
mod tests {
    use crate::api::model::provider_stream_factory::create_provider_stream;
    use crate::api::model::provider_stream_factory::PlaylistItemType;
    use actix_web::test;
    use actix_web::test::TestRequest;
    use actix_web::web;
    use actix_web::App;
    use actix_web::{HttpRequest, HttpResponse};
    use futures::StreamExt;
    use reqwest::Url;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[actix_rt::test]
    async fn test_stream() {
        let req = TestRequest::get().uri("/test").to_request();
        let app = App::new().route("/test", web::get().to(test_stream_handler));
        let server = test::init_service(app).await;
        let response = test::call_service(&server, req).await;
    }
    async fn test_stream_handler(req: HttpRequest) -> HttpResponse {
        let mut counter = 5;
        let client = Arc::new(reqwest::Client::new());
        let url = url::Url::parse("http://10.41.41.41").unwrap();
        let input = None;

        let options = (PlaylistItemType::Live, true, true, 0);
        'outer: while let Some((mut stream, info)) = create_provider_stream(Arc::clone(&client), &url, &req, input, options).await {
            if info.is_some() {
                println!("{:?}", info.unwrap());
            }
            while let Some(result) = stream.next().await {
                match result {
                    Ok(bytes) => {
                        println!("Received {} bytes", bytes.len());
                        counter -= 1;
                        println!("{bytes:?}");
                        if counter < 0 {
                            break 'outer;
                        }
                    }
                    Err(err) => {
                        eprintln!("Error occurred: {}", err);
                        break 'outer;
                    }
                }
            }
        }
        HttpResponse::Ok().finish()
    }
}
