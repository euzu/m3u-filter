use std::collections::HashMap;
use crate::api::api_utils::{get_headers_from_request, HeaderFilter};
use crate::api::model::model_utils::get_response_headers;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::custom_video_stream::CustomVideoStream;
use crate::api::model::streams::provider_stream_factory::{create_provider_stream, BufferStreamOptions};
use crate::model::config::{Config};
use crate::model::playlist::PlaylistItemType;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::{get_request_headers, sanitize_sensitive_info};
use futures::TryStreamExt;
use log::{error, trace};
use reqwest::StatusCode;
use std::sync::Arc;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use url::Url;
use crate::api::model::app_state::AppState;
use crate::api::model::stream::ProviderStreamResponse;

pub enum CustomVideoStreamType {
    ChannelUnavailable,
    UserConnectionsExhausted,
    // ProviderConnectionsExhausted,
}

fn create_video_stream(video: Option<&Arc<Vec<u8>>>, headers: &[(String, String)], log_message: &str) -> ProviderStreamResponse {
    if let Some(video) = video {
        trace!("{log_message}");
        let mut response_headers: Vec<(String, String)> = headers.iter()
            .filter(|(key, _)| !(key.eq("content-type") || key.eq("content-length") || key.contains("range")))
            .map(|(key, value)| (key.to_string(), value.to_string())).collect();
        response_headers.push(("content-type".to_string(), "video/mp2t".to_string()));
        (Some(Box::pin(CustomVideoStream::new(Arc::clone(video)))), Some((response_headers, StatusCode::OK)))
    } else {
        (None, None)
    }
}

pub fn create_channel_unavailable_stream(cfg: &Config, headers: &[(String, String)], status: StatusCode) -> ProviderStreamResponse {
    create_video_stream(cfg.t_channel_unavailable_video.as_ref(), headers, &format!("Streaming response channel unavailable for status {status}"))
}

pub fn create_user_connections_exhausted_stream(cfg: &Config, headers: &[(String, String)]) -> ProviderStreamResponse {
    create_video_stream(cfg.t_user_connections_exhausted_video.as_ref(), headers, "Streaming response user connections exhausted")
}

pub fn create_provider_connections_exhausted_stream(cfg: &Config, headers: &[(String, String)]) -> ProviderStreamResponse {
    create_video_stream(cfg.t_provider_connections_exhausted_video.as_ref(), headers, "Streaming response provider connections exhausted")
}

pub fn create_custom_video_stream_response(config: &Config, video_response: &CustomVideoStreamType) -> impl axum::response::IntoResponse + Send {
    if let (Some(stream), Some((headers, status_code))) = match video_response {
        CustomVideoStreamType::ChannelUnavailable => create_channel_unavailable_stream(config, &[], StatusCode::BAD_REQUEST),
        CustomVideoStreamType::UserConnectionsExhausted => create_user_connections_exhausted_stream(config, &[]),
        // CustomVideoStreamType::ProviderConnectionsExhausted => create_provider_connections_exhausted_stream(config, &[]),
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
        PlaylistItemType::Live | PlaylistItemType::LiveHls | PlaylistItemType::LiveDash | PlaylistItemType::LiveUnknown => {
            Some(Box::new(|key| key != "accept-ranges" && key != "range" && key != "content-range"))
        }
        _ => None,
    }
}

pub async fn get_provider_pipe_stream(app_state: &AppState,
                                      stream_url: &Url,
                                      req_headers: &HeaderMap,
                                      input_headers: Option<&HashMap<String, String>>,
                                      item_type: PlaylistItemType) -> ProviderStreamResponse {
    let filter_header = get_header_filter_for_item_type(item_type);
    let req_headers = get_headers_from_request(req_headers, &filter_header);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // These are the configured headers for this input.
    // The stream url, we need to clone it because of move to async block.
    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers, Some(&req_headers));
    let client = app_state.http_client.get(stream_url.clone()).headers(headers.clone());
    match client.send().await {
        Ok(response) => {
            let response_headers = get_response_headers(response.headers());
            // TODO hls handling
            let status = response.status();
            if status.is_success() {
                (Some(Box::pin(response.bytes_stream().map_err(|err| StreamError::reqwest(&err)))), Some((response_headers, status)))
            } else if let (Some(boxed_provider_stream), response_info) =  create_channel_unavailable_stream(&app_state.config, &response_headers, status) {
                (Some(boxed_provider_stream), response_info)
            } else {
                (None, Some((response_headers, status)))
            }
        }
        Err(err) => {
            let masked_url = sanitize_sensitive_info(stream_url.as_str());
            error!("Failed to open stream {masked_url} {err}");
            if let (Some(boxed_provider_stream), response_info) = create_channel_unavailable_stream(&app_state.config, &get_response_headers(&headers), StatusCode::BAD_GATEWAY) {
                (Some(boxed_provider_stream), response_info)
            } else {
                (None, None)
            }
        }
    }
}

pub async fn get_provider_reconnect_buffered_stream(app_state: &AppState,
                                                    stream_url: &Url,
                                                    req_headers: &HeaderMap,
                                                    input_headers: Option<&HashMap<String, String>>,
                                                    options: BufferStreamOptions) -> ProviderStreamResponse {
    match create_provider_stream(&app_state.config, Arc::clone(&app_state.http_client), stream_url, req_headers, input_headers, options).await {
        None => (None, None),
        Some((stream, info)) => {
            (Some(stream), info)
        }
    }
}
