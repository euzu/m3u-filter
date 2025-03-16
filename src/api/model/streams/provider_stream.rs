use crate::api::api_utils::{get_headers_from_request, HeaderFilter, StreamOptions};
use crate::api::model::model_utils::get_response_headers;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::custom_video_stream::CustomVideoStream;
use crate::api::model::streams::provider_stream_factory::{create_provider_stream, BufferStreamOptions};
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::PlaylistItemType;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::{get_request_headers, sanitize_sensitive_info};
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::TryStreamExt;
use log::{debug, error};
use reqwest::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use url::Url;

type BoxedProviderStream = BoxStream<'static, Result<Bytes, StreamError>>;
type ProviderStreamHeader = Vec<(String, String)>;
type ProviderStreamResponse = (Option<BoxedProviderStream>, Option<(ProviderStreamHeader, StatusCode)>);

pub enum CustomVideoStreamType {
    ChannelUnavailable,
    UserConnectionsExhausted,
    ProviderConnectionsExhausted,
}

fn create_video_stream(video: Option<&Arc<Vec<u8>>>, headers: &[(String, String)], log_message: &str) -> Option<(BoxedProviderStream, (ProviderStreamHeader, StatusCode))> {
    if let Some(video) = video {
        debug!("{}", log_message);
        let mut response_headers: Vec<(String, String)> = headers.iter()
            .filter(|(key, _)| !(key.eq("content-type") || key.eq("content-length") || key.contains("range")))
            .map(|(key, value)| (key.to_string(), value.to_string())).collect();
        response_headers.push(("content-type".to_string(), "video/mp2t".to_string()));
        Some((Box::pin(CustomVideoStream::new(Arc::clone(video))), (response_headers, StatusCode::OK)))
    } else {
        None
    }
}

pub fn create_channel_unavailable_stream(cfg: &Config, headers: &[(String, String)], status: StatusCode) -> Option<(BoxedProviderStream, (ProviderStreamHeader, StatusCode))> {
    create_video_stream(cfg.t_channel_unavailable_video.as_ref(), headers, &format!("Streaming response channel unavailable for status {status}"))
}

pub fn create_user_connections_exhausted_stream(cfg: &Config, headers: &[(String, String)]) -> Option<(BoxedProviderStream, (ProviderStreamHeader, StatusCode))> {
    create_video_stream(cfg.t_user_connections_exhausted_video.as_ref(), headers, "Streaming response user connections exhausted")
}

pub fn create_provider_connections_exhausted_stream(cfg: &Config, headers: &[(String, String)]) -> Option<(BoxedProviderStream, (ProviderStreamHeader, StatusCode))> {
    create_video_stream(cfg.t_provider_connections_exhausted_video.as_ref(), headers, "Streaming response provider connections exhausted")
}

pub fn create_custom_video_stream_response(config: &Config, video_response: &CustomVideoStreamType) -> impl axum::response::IntoResponse + Send {
    if let Some((stream, (headers, status_code))) = match video_response {
        CustomVideoStreamType::ChannelUnavailable => create_channel_unavailable_stream(config, &[], StatusCode::BAD_REQUEST),
        CustomVideoStreamType::UserConnectionsExhausted => create_user_connections_exhausted_stream(config, &[]),
        CustomVideoStreamType::ProviderConnectionsExhausted => create_provider_connections_exhausted_stream(config, &[]),
    } {
        let mut builder = axum::response::Response::builder()
            .status(status_code);
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
        return builder.body(axum::body::Body::from_stream(stream)).unwrap().into_response();
    }
    axum::http::StatusCode::FORBIDDEN.into_response()
}


pub fn get_header_filter_for_item_type(item_type: PlaylistItemType) -> HeaderFilter {
    match item_type {
        PlaylistItemType::Live | PlaylistItemType::LiveUnknown | PlaylistItemType::LiveHls => {
            Some(Box::new(|key| key != "accept-ranges" && key != "range" && key != "content-range"))
        }
        _ => None,
    }
}

pub async fn get_provider_pipe_stream(cfg: &Config,
                                      http_client: &Arc<reqwest::Client>,
                                      stream_url: &Url,
                                      req_headers: &HeaderMap,
                                      input: Option<&ConfigInput>,
                                      item_type: PlaylistItemType,
                                      stream_options: &StreamOptions) -> ProviderStreamResponse {
    let filter_header = get_header_filter_for_item_type(item_type);
    let req_headers = get_headers_from_request(req_headers, &filter_header);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // These are the configured headers for this input.
    let input_headers = input.map(|i| i.headers.clone());
    // The stream url, we need to clone it because of move to async block.
    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));
    let client_builder = http_client.get(stream_url.clone()).headers(headers.clone());
    let client = if stream_options.stream_connect_timeout_secs > 0 {
        client_builder.timeout(Duration::from_secs(u64::from(stream_options.stream_connect_timeout_secs)))
    } else {
        client_builder
    };
    match client.send().await {
        Ok(response) => {
            let response_headers = get_response_headers(response.headers());
            // TODO hls handling
            let status = response.status();
            if status.is_success() {
                (Some(Box::pin(response.bytes_stream().map_err(|err| StreamError::reqwest(&err)))), Some((response_headers, status)))
            } else if let Some((boxed_provider_stream, response_info)) = create_channel_unavailable_stream(cfg, &response_headers, status) {
                (Some(boxed_provider_stream), Some(response_info))
            } else {
                (None, Some((response_headers, status)))
            }
        }
        Err(err) => {
            let masked_url = sanitize_sensitive_info(stream_url.as_str());
            error!("Failed to open stream {masked_url} {err}");
            if let Some((boxed_provider_stream, response_info)) = create_channel_unavailable_stream(cfg, &get_response_headers(&headers), StatusCode::BAD_GATEWAY) {
                (Some(boxed_provider_stream), Some(response_info))
            } else {
                (None, None)
            }
        }
    }
}

pub async fn get_provider_reconnect_buffered_stream(cfg: &Config,
                                                    http_client: &Arc<reqwest::Client>,
                                                    stream_url: &Url,
                                                    req_headers: &HeaderMap,
                                                    input: Option<&ConfigInput>,
                                                    options: BufferStreamOptions) -> ProviderStreamResponse {
    match create_provider_stream(cfg, Arc::clone(http_client), stream_url, req_headers, input, options).await {
        None => (None, None),
        Some((stream, info)) => {
            (Some(stream), info)
        }
    }
}
