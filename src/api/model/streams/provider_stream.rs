use crate::api::api_utils::{HeaderFilter};
use crate::api::model::streams::custom_video_stream::CustomVideoStream;
use crate::model::{Config};
use crate::model::PlaylistItemType;
use log::{trace};
use reqwest::StatusCode;
use std::sync::Arc;
use axum::response::IntoResponse;
use crate::api::model::stream::ProviderStreamResponse;

pub enum CustomVideoStreamType {
    ChannelUnavailable,
    UserConnectionsExhausted,
    ProviderConnectionsExhausted,
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
        PlaylistItemType::Live | PlaylistItemType::LiveHls | PlaylistItemType::LiveDash | PlaylistItemType::LiveUnknown => {
            Some(Box::new(|key| key != "accept-ranges" && key != "range" && key != "content-range"))
        }
        _ => None,
    }
}
