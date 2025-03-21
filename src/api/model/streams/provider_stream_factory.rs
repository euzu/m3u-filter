use crate::api::api_utils::{get_headers_from_request, StreamOptions};
use crate::api::model::model_utils::get_response_headers;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::buffered_stream::BufferedStream;
use crate::api::model::streams::client_stream::ClientStream;
use crate::api::model::streams::provider_stream::{create_channel_unavailable_stream, get_header_filter_for_item_type};
use crate::api::model::streams::timed_client_stream::{TimeoutClientStream};
use crate::model::config::{Config};
use crate::model::playlist::PlaylistItemType;
use crate::tools::atomic_once_flag::AtomicOnceFlag;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::{classify_content_type, get_request_headers, sanitize_sensitive_info, MimeCategory};
use futures::stream::{self};
use futures::{StreamExt, TryStreamExt};
use log::{error, warn};
use reqwest::header::{HeaderMap, RANGE};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;
use crate::api::model::stream::{BoxedProviderStream, ProviderStreamFactoryResponse};

// TODO make this configurable
pub const STREAM_QUEUE_SIZE: usize = 4096; // mpsc channel holding messages. with possible 8192byte chunks

pub struct BufferStreamOptions {
    item_type: PlaylistItemType,
    reconnect_enabled: bool,
    force_reconnect_secs: u32,
    connect_timeout_secs: u32,
    buffer_enabled: bool,
    buffer_size: usize,
    share_stream: bool,
    reconnect_flag: Arc<AtomicOnceFlag>
}

impl BufferStreamOptions {
    pub(crate) fn new(
        item_type: PlaylistItemType,
        share_stream: bool,
        stream_options: &StreamOptions
    ) -> Self {
        Self {
            item_type,
            reconnect_enabled: stream_options.stream_retry,
            force_reconnect_secs: stream_options.stream_force_retry_secs,
            connect_timeout_secs: stream_options.stream_connect_timeout_secs,
            buffer_enabled: stream_options.buffer_enabled,
            buffer_size: stream_options.buffer_size,
            share_stream,
            reconnect_flag: Arc::new(AtomicOnceFlag::new())
        }
    }

    #[inline]
    fn is_buffer_enabled(&self) -> bool {
        self.buffer_enabled
    }

    #[inline]
    fn is_shared_stream(&self) -> bool {
        self.share_stream
    }

    // #[inline]
    // fn get_buffer_size(&self) -> usize {
    //     self.buffer_size
    // }

    #[inline]
    fn is_reconnect_enabled(&self) -> bool {
        self.reconnect_enabled
    }

    #[inline]
    pub(crate) fn get_stream_buffer_size(&self) -> usize {
        if self.buffer_size > 0 { self.buffer_size } else { STREAM_QUEUE_SIZE }
    }

    #[inline]
    pub fn get_reconnect_flag_clone(&self) -> Arc<AtomicOnceFlag> {
        Arc::clone(&self.reconnect_flag)
    }

}


#[derive(Debug, Clone)]
struct ProviderStreamOptions {
    buffer_size: usize,
    continue_flag: Arc<AtomicOnceFlag>,
    url: Url,
    reconnect: bool,
    reconnect_force_secs: u32,
    connect_timeout_secs: u32,
    headers: HeaderMap,
    range_bytes: Arc<Option<AtomicUsize>>,
}

impl ProviderStreamOptions {
    #[inline]
    pub fn is_buffered(&self) -> bool {
        self.buffer_size > 0
    }
    #[inline]
    pub fn get_buffer_size(&self) -> usize {
        self.buffer_size
    }
    #[inline]
    pub fn get_continue_flag_clone(&self) -> Arc<AtomicOnceFlag> {
        Arc::clone(&self.continue_flag)
    }

    // #[inline]
    // pub fn get_continue_flag(&self) -> &Arc<AtomicFlag> {
    //     &self.continue_flag
    // }

    #[inline]
    pub fn cancel_reconnect(&self) {
        self.continue_flag.notify();
    }

    #[inline]
    pub fn get_url(&self) -> &Url {
        &self.url
    }

    #[inline]
    pub fn should_reconnect(&self) -> bool {
        self.reconnect
    }

    #[inline]
    pub fn get_headers(&self) -> &HeaderMap {
        &self.headers
    }

    #[inline]
    pub fn get_total_bytes_send(&self) -> Option<usize> {
        self.range_bytes.as_ref().as_ref().map(|atomic| atomic.load(Ordering::SeqCst))
    }

    // pub fn get_range_bytes(&self) -> &Arc<Option<AtomicUsize>> {
    //     &self.range_bytes
    // }

    #[inline]
    pub fn get_range_bytes_clone(&self) -> Arc<Option<AtomicUsize>> {
        Arc::clone(&self.range_bytes)
    }

    #[inline]
    pub fn should_continue(&self) -> bool {
        self.continue_flag.is_active()
    }
}

fn get_request_range_start_bytes(req_headers: &HashMap<String, Vec<u8>>) -> Option<usize> {
    // range header looks like  bytes=1234-5566/2345345 or bytes=0-
    if let Some(req_range) = req_headers.get(axum::http::header::RANGE.as_str()) {
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
    req_headers: &HeaderMap,
    input_headers: Option<HashMap<String, String>>,
    options: &BufferStreamOptions) -> (usize, Option<usize>, bool, u32, u32, HeaderMap)
{
    let stream_buffer_size = if options.is_buffer_enabled() { options.get_stream_buffer_size() } else { 1 };
    let filter_header = get_header_filter_for_item_type(options.item_type);
    let mut req_headers = get_headers_from_request(req_headers, &filter_header);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // we need the range bytes from client request for seek ing to the right position
    let req_range_start_bytes = get_request_range_start_bytes(&req_headers);
    req_headers.remove("range");

    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));

    (stream_buffer_size, req_range_start_bytes, options.is_reconnect_enabled(), options.force_reconnect_secs, options.connect_timeout_secs, headers)
}

fn prepare_client(request_client: &Arc<reqwest::Client>, stream_options: &ProviderStreamOptions) -> (reqwest::RequestBuilder, bool) {
    let url = stream_options.get_url();
    let range_start = stream_options.get_total_bytes_send();
    let headers = stream_options.get_headers();
    let mut client = request_client.get(url.clone()).headers(headers.clone());

    let (client_builder, partial) = {
        if let Some(range) = range_start {
            // on reconnect send range header to avoid starting from beginning for vod
            let range = format!("bytes={range}-", );
            client = client.header(RANGE, range);
            (client, true) // partial content
        } else {
            (client, false)
        }
    };

    let client = if stream_options.connect_timeout_secs > 0 {
        client_builder.timeout(Duration::from_secs(u64::from(stream_options.connect_timeout_secs)))
    } else {
        client_builder
    };
    (client, partial)
}

async fn provider_initial_request(cfg: &Config, request_client: Arc<reqwest::Client>, stream_options: &ProviderStreamOptions) -> Result<Option<ProviderStreamFactoryResponse>, StatusCode> {
    let (client, _partial_content) = prepare_client(&request_client, stream_options);
    match client.send().await {
        Ok(mut response) => {
            let status = response.status();
            if status.is_success() {
                let response_info = {
                    // Unfortunately, the HEAD request does not work, so we need this workaround.
                    // We need some header information from the provider, we extract the necessary headers and forward them to the client
                    debug_if_enabled!("Provider response  status: '{}' headers: {:?}", response.status(), response.headers_mut());
                    let response_headers: Vec<(String, String)> = get_response_headers(response.headers());
                    // debug!("First  headers {headers:?} {} {}", sanitize_sensitive_info(url.as_str()));
                    Some((response_headers, response.status()))
                };
                return Ok(Some((response.bytes_stream().map_err(|err| {
                    //error!("Failed to read response body: {err}");
                    StreamError::reqwest(&err)
                }).boxed(), response_info)));
            }
            if let (Some(boxed_provider_stream), response_info) =
                create_channel_unavailable_stream(cfg, &get_response_headers(response.headers()), status)
            {
                return Ok(Some((boxed_provider_stream, response_info)));
            }
            Err(status)
        }
        Err(_err) => {
            if let (Some(boxed_provider_stream), response_info) =
                create_channel_unavailable_stream(cfg, &get_response_headers(stream_options.get_headers()), StatusCode::BAD_GATEWAY)
            {
                Ok(Some((boxed_provider_stream, response_info)))
            } else {
                Err(StatusCode::SERVICE_UNAVAILABLE)
            }
        }
    }
}

async fn stream_provider(client: Arc<reqwest::Client>, stream_options: ProviderStreamOptions) -> Option<BoxedProviderStream> {
    let url = stream_options.get_url();
    debug_if_enabled!("stream provider {}", sanitize_sensitive_info(url.as_str()));
    while stream_options.should_continue() {
        debug_if_enabled!("Reconnecting stream {}", sanitize_sensitive_info(url.as_str()));
        let (client, _) = prepare_client(&client, &stream_options);
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let provider_stream = response.bytes_stream().map_err(|err| {
                        error!("Stream error {err}");
                        StreamError::reqwest(&err)
                    }).boxed();
                    return if stream_options.reconnect_force_secs > 0 {
                        Some(TimeoutClientStream::new(provider_stream, stream_options.reconnect_force_secs).boxed())
                    } else {
                        Some(provider_stream)
                    };
                }
                if status.is_client_error() {
                    return None;
                }
                if status.is_server_error() {
                    match status {
                        StatusCode::INTERNAL_SERVER_ERROR |
                        StatusCode::BAD_GATEWAY |
                        StatusCode::SERVICE_UNAVAILABLE |
                        StatusCode::GATEWAY_TIMEOUT => {}
                        _ => return None
                    }
                }
            }
            Err(_err) => {}
        }
        if !stream_options.should_continue() {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    debug_if_enabled!("Stopped reconnecting stream {}", sanitize_sensitive_info(url.as_str()));
    None
}

const RETRY_SECONDS: u64 = 5;
const ERR_MAX_RETRY_COUNT: u32 = 5;
async fn get_initial_stream(cfg: &Config, client: Arc<reqwest::Client>, stream_options: &ProviderStreamOptions) -> Option<ProviderStreamFactoryResponse> {
    let start = Instant::now();
    let mut connect_err: u32 = 1;
    while stream_options.should_continue() {
        match provider_initial_request(cfg, Arc::clone(&client), stream_options).await {
            Ok(Some(value)) => return Some(value),
            Ok(None) => {
                if connect_err > ERR_MAX_RETRY_COUNT {
                    warn!("The stream could be unavailable. {}", sanitize_sensitive_info(stream_options.get_url().as_str()));
                }
            }
            Err(status) => {
                if connect_err > ERR_MAX_RETRY_COUNT {
                    warn!("The stream could be unavailable. ({status}) {}", sanitize_sensitive_info(stream_options.get_url().as_str()));
                }
            }
        };
        if connect_err > ERR_MAX_RETRY_COUNT {
            break;
        }
        if start.elapsed().as_secs() > RETRY_SECONDS {
            warn!("The stream could be unavailable. Giving up after {RETRY_SECONDS} seconds. {}", sanitize_sensitive_info(stream_options.get_url().as_str()));
            break;
        }
        connect_err += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stream_options.cancel_reconnect();
    None
}

fn create_provider_stream_options(stream_url: &Url,
                                  req_headers: &HeaderMap,
                                  input_headers: Option<HashMap<String, String>>,
                                  options: &BufferStreamOptions) -> ProviderStreamOptions {
    let (buffer_size, req_range_start_bytes, reconnect, reconnect_force_secs, connect_timeout_secs, headers)
        = get_client_stream_request_params(req_headers, input_headers, options);
    let url = stream_url.clone();
    let range_bytes = Arc::new(req_range_start_bytes.map(AtomicUsize::new));

    ProviderStreamOptions {
        buffer_size,
        continue_flag: Arc::clone(&options.reconnect_flag),
        url,
        reconnect,
        reconnect_force_secs,
        connect_timeout_secs,
        headers,
        range_bytes,
    }
}

pub async fn create_provider_stream(cfg: &Config,
                                    client: Arc<reqwest::Client>,
                                    stream_url: &Url,
                                    req_headers: &HeaderMap,
                                    input_headers: Option<HashMap<String, String>>,
                                    options: BufferStreamOptions) -> Option<ProviderStreamFactoryResponse> {
    let stream_options = create_provider_stream_options(stream_url, req_headers, input_headers, &options);

    let client_stream_factory = |stream, reconnect_flag, range_cnt| {
        let stream = if stream_options.is_buffered() && !options.is_shared_stream() {
            BufferedStream::new(stream, stream_options.get_buffer_size(), stream_options.get_continue_flag_clone(), stream_url.as_str()).boxed()
        } else {
            stream
        };
        ClientStream::new(stream, reconnect_flag, range_cnt, stream_options.get_url().as_str()).boxed()
    };

    match get_initial_stream(cfg, Arc::clone(&client), &stream_options).await {
        Some((init_stream, info)) => {
            let is_media_stream = if let Some((headers, _)) = &info {
                classify_content_type(headers) == MimeCategory::Video
            } else {
                true // don't know what it is but lets assume it is
            };

            let continue_signal = stream_options.get_continue_flag_clone();
            if is_media_stream && stream_options.should_reconnect() {
                let continue_client_signal = Arc::clone(&continue_signal);
                let continue_streaming_signal = continue_client_signal.clone();
                let stream_options_provider = stream_options.clone();
                let unfold: BoxedProviderStream = stream::unfold((), move |()| {
                    let client = Arc::clone(&client);
                    let stream_opts = stream_options_provider.clone();
                    let continue_streaming = continue_streaming_signal.clone();
                    async move {
                        if continue_streaming.is_active() {
                            let stream = stream_provider(client, stream_opts).await?;
                            Some((stream, ()))
                        } else {
                            None
                        }
                    }
                }).flatten().boxed();
                Some((client_stream_factory(init_stream.chain(unfold).boxed(), Arc::clone(&continue_client_signal), stream_options.get_range_bytes_clone()).boxed(), info))
            } else {
                Some((client_stream_factory(init_stream.boxed(), Arc::clone(&continue_signal), stream_options.get_range_bytes_clone()).boxed(), info))
            }
        }
        None => None
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::api::model::streams::provider_stream_factory::PlaylistItemType;
//     use crate::api::model::streams::provider_stream_factory::{create_provider_stream, BufferStreamOptions};
//     use actix_web::test;
//     use actix_web::test::TestRequest;
//     use actix_web::web;
//     use actix_web::App;
//     use actix_web::{HttpRequest, HttpResponse};
//     use futures::StreamExt;
//     use std::sync::Arc;
//     use crate::model::config::Config;
//
//     #[tokio::test]
//     async fn test_stream() {
//         let app = App::new().route("/test", web::get().to(test_stream_handler));
//         let server = test::init_service(app).await;
//         let req = TestRequest::get().uri("/test").to_request();
//         let _response = test::call_service(&server, req).await;
//     }
//     async fn test_stream_handler(req: axum::http::Request<axum::body::Body>) ->  impl axum::response::IntoResponse + Send {
//         let cfg = Config::default();
//         let mut counter = 5;
//         let client = Arc::new(reqwest::Client::new());
//         let url = url::Url::parse("https://info.cern.ch/hypertext/WWW/TheProject.html").unwrap();
//         let input = None;
//
//         let options = BufferStreamOptions::new(PlaylistItemType::Live, true, true, 0, false);
//         let value = create_provider_stream(&cfg, Arc::clone(&client), &url, &req, input, options);
//         let mut values = value.await;
//         'outer: while let Some((ref mut stream, info)) = values.as_mut() {
//             if info.is_some() {
//                 println!("{:?}", info.as_ref().unwrap());
//             }
//             while let Some(result) = stream.next().await {
//                 match result {
//                     Ok(bytes) => {
//                         println!("Received {} bytes  {bytes:?}", bytes.len());
//                         counter -= 1;
//                         if counter < 0 {
//                             break 'outer;
//                         }
//                     }
//                     Err(err) => {
//                         eprintln!("Error occurred: {}", err);
//                         break 'outer;
//                     }
//                 }
//             }
//         }
//         HttpResponse::Ok().finish()
//     }
// }
