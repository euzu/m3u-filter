use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::shared_stream::SharedStream;
use crate::debug_if_enabled;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, ConfigTarget};
use crate::model::playlist::PlaylistItemType;
use crate::utils::request_utils;
use crate::utils::request_utils::mask_sensitive_info;
use actix_web::http::header::DATE;
use actix_web::http::header::{HeaderValue, CACHE_CONTROL};
use actix_web::{HttpRequest, HttpResponse};
use bytes::Bytes;
use chrono::Utc;
use log::{error, log_enabled, trace};
use std::collections::HashMap;
use std::path::Path;
use tokio_stream::wrappers::{BroadcastStream};
use url::Url;
use crate::api::model::buffered_stream;
use crate::api::model::buffered_stream::get_stream_response_with_headers;

pub async fn serve_file(file_path: &Path, req: &HttpRequest, mime_type: mime::Mime) -> HttpResponse {
    if file_path.exists() {
        if let Ok(file) = actix_files::NamedFile::open_async(file_path).await {
            let mut result = file.set_content_type(mime_type)
                .disable_content_disposition().into_response(req);
            let headers = result.headers_mut();
            headers.insert(CACHE_CONTROL, HeaderValue::from_bytes(b"no-cache").unwrap());
            return result;
        }
    }
    HttpResponse::NoContent().finish()
}

pub fn get_user_target_by_credentials<'a>(username: &str, password: &str, api_req: &'a UserApiRequest,
                                          app_state: &'a AppState) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    if !username.is_empty() && !password.is_empty() {
        app_state.config.get_target_for_user(username, password)
    } else {
        let token = api_req.token.as_str().trim();
        if token.is_empty() {
            None
        } else {
            app_state.config.get_target_for_user_by_token(token)
        }
    }
}

pub fn get_user_target<'a>(api_req: &'a UserApiRequest, app_state: &'a AppState) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    let username = api_req.username.as_str().trim();
    let password = api_req.password.as_str().trim();
    get_user_target_by_credentials(username, password, api_req, app_state)
}

/// Creates a notify stream for the given URL if a shared stream exists.
async fn create_notify_stream(
    app_state: &AppState,
    stream_url: &str,
) -> Option<BroadcastStream<Bytes>> {
    let notify_stream_url = stream_url.to_string();

    // Acquire lock and check for existing stream
    let shared_streams = app_state.shared_streams.lock().await;
    if let Some(shared_stream) = shared_streams.get(&notify_stream_url) {
        let rx = shared_stream.data_stream.subscribe();
        Some(BroadcastStream::new(rx))
    } else {
        None
    }
}

pub async fn stream_response(app_state: &AppState, stream_url: &str, req: &HttpRequest, input: Option<&ConfigInput>, item_type: PlaylistItemType, target: &ConfigTarget) -> HttpResponse {
    if log_enabled!(log::Level::Trace) { trace!("Try to open stream {}", mask_sensitive_info(stream_url));}

    let share_stream = is_stream_share_enabled(item_type, target);
    if share_stream {
        if let Some(value) = shared_stream_response(app_state, stream_url, None).await {
            return value;
        }
    }

    if let Ok(url) = Url::parse(stream_url) {
        let (stream, provider_response) = buffered_stream::get_buffered_stream(&app_state.http_client, &url, req, input).await;
        return if share_stream {
            SharedStream::register(app_state, stream_url, stream).await;
            if let Some(broadcast_stream) = create_notify_stream(app_state, stream_url).await {
                let body_stream = actix_web::body::BodyStream::new(broadcast_stream);
                let mut response_builder = get_stream_response_with_headers(provider_response, stream_url);
                response_builder.body(body_stream)
            } else {
                HttpResponse::BadRequest().finish()
            }
        } else {
            let mut response_builder = get_stream_response_with_headers(provider_response, stream_url);
            response_builder.streaming(stream)
        }
    }
    error!("Url is malformed {}", mask_sensitive_info(stream_url));
    HttpResponse::BadRequest().finish()
}

async fn shared_stream_response(app_state: &AppState, stream_url: &str, headers: Option<(Vec<(String, String)>, reqwest::StatusCode)>) -> Option<HttpResponse> {
    if let Some(stream) = create_notify_stream(app_state, stream_url).await {
        debug_if_enabled!("Using shared channel {}", mask_sensitive_info(stream_url));
        if app_state.shared_streams.lock().await.get(stream_url).is_some() {
            let mut response_builder = get_stream_response_with_headers(headers, stream_url);
            let current_date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            response_builder.insert_header((DATE, current_date.as_bytes()));
            // response_builder.insert_header((ACCEPT_RANGES, "bytes".as_bytes()));
            return Some(response_builder.body(actix_web::body::BodyStream::new(stream)));
        }
    }
    None
}

pub fn is_stream_share_enabled(item_type: PlaylistItemType, target: &ConfigTarget) -> bool {
    item_type == PlaylistItemType::Live && target.options.as_ref().is_some_and(|opt| opt.share_live_streams)
}


pub fn get_headers_from_request(req: &HttpRequest) -> HashMap<String, Vec<u8>> {
    req.headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
        .collect()
}

pub async fn resource_response(app_state: &AppState, resource_url: &str, req: &HttpRequest, input: Option<&ConfigInput>) -> HttpResponse {
    if resource_url.is_empty() {
        return HttpResponse::NoContent().finish();
    }
    let req_headers = get_headers_from_request(req);
    debug_if_enabled!("Try to open resource {}", mask_sensitive_info(resource_url));

    if let Ok(url) = Url::parse(resource_url) {
        let client = request_utils::get_client_request(&app_state.http_client, input.map(|i| &i.headers), &url, Some(&req_headers));
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let mut response_builder = HttpResponse::Ok();
                    response.headers().iter().for_each(|(k, v)| {
                        response_builder.insert_header((k.as_str(), v.as_ref()));
                    });
                    return response_builder.body(actix_web::body::BodyStream::new(response.bytes_stream()));
                }
                debug_if_enabled!("Failed to open resource got status {} for {}", status, mask_sensitive_info(resource_url));
            }
            Err(err) => {
                error!("Received failure from server {}:  {}", mask_sensitive_info(resource_url), err);
            }
        }
    } else {
        error!("Url is malformed {}", mask_sensitive_info(resource_url));
    }
    HttpResponse::BadRequest().finish()
}
