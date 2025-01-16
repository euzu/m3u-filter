use crate::api::model::app_state::AppState;
use crate::api::model::provider_stream;
use crate::api::model::provider_stream::{get_provider_pipe_stream};
use crate::api::model::request::UserApiRequest;
use crate::api::model::shared_stream::SharedStream;
use crate::debug_if_enabled;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, ConfigTarget};
use crate::model::playlist::PlaylistItemType;
use crate::utils::request_utils;
use crate::utils::request_utils::mask_sensitive_info;
use actix_files::NamedFile;
use actix_web::body::{BodyStream};
use actix_web::http::header::DATE;
use actix_web::http::header::{HeaderValue, CACHE_CONTROL};
use actix_web::{HttpRequest, HttpResponse};
use bytes::Bytes;
use chrono::Utc;
use log::{error, log_enabled, trace};
use std::collections::HashMap;
use std::path::{Path};
use std::sync::Arc;
use async_std::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use url::Url;
use crate::api::model::model_utils::get_stream_response_with_headers;
use crate::api::model::persist_pipe_stream::PersistPipeStream;
use crate::utils::file_utils::create_new_file_for_write;
use crate::utils::lru_cache::LRUResourceCache;

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

/// Creates a broadcast notify stream for the given URL if a shared stream exists.
async fn create_broadcast_stream(
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

pub async fn stream_response(app_state: &AppState, stream_url: &str,
                             req: &HttpRequest, input: Option<&ConfigInput>,
                             item_type: PlaylistItemType, target: &ConfigTarget) -> HttpResponse {
    if log_enabled!(log::Level::Trace) { trace!("Try to open stream {}", mask_sensitive_info(stream_url)); }

    let share_stream = is_stream_share_enabled(item_type, target);
    if share_stream {
        if let Some(value) = shared_stream_response(app_state, stream_url, None).await {
            return value;
        }
    }

    let (stream_retry, buffer_enabled, buffer_size) = app_state
        .config
        .reverse_proxy
        .as_ref()
        .and_then(|reverse_proxy| reverse_proxy.stream.as_ref())
        .map_or((false, false, 0), |stream| {
            let (buffer_enabled, buffer_size) = stream
                .buffer
                .as_ref()
                .map_or((false, 0), |buffer| (buffer.enabled, buffer.size));
            (stream.retry, buffer_enabled, buffer_size)
        });


    if let Ok(url) = Url::parse(stream_url) {
        let direct_pipe_provider_stream = !stream_retry && !buffer_enabled;
        let (stream_opt, provider_response) = if direct_pipe_provider_stream {
            get_provider_pipe_stream(&app_state.http_client, &url, req, input).await
        } else {
            let buffer_stream_options = (item_type, stream_retry, buffer_enabled, buffer_size);
            provider_stream::get_provider_reconnect_buffered_stream(&app_state.http_client, &url, req, input, buffer_stream_options).await
        };
        if let Some(stream) = stream_opt {
            let use_buffer = !buffer_enabled || direct_pipe_provider_stream;
            return if share_stream {
                SharedStream::register(app_state, stream_url, stream, use_buffer).await;
                if let Some(broadcast_stream) = create_broadcast_stream(app_state, stream_url).await {
                    let body_stream = BodyStream::new(broadcast_stream);
                    let mut response_builder = get_stream_response_with_headers(provider_response, stream_url);
                    response_builder.body(body_stream)
                } else {
                    HttpResponse::BadRequest().finish()
                }
            } else {
                let mut response_builder = get_stream_response_with_headers(provider_response, stream_url);
                response_builder.streaming(stream)
            };
        }
    }
    error!("Cant open stream {}", mask_sensitive_info(stream_url));
    HttpResponse::BadRequest().finish()
}

async fn shared_stream_response(app_state: &AppState, stream_url: &str, headers: Option<(Vec<(String, String)>, reqwest::StatusCode)>) -> Option<HttpResponse> {
    if let Some(stream) = create_broadcast_stream(app_state, stream_url).await {
        debug_if_enabled!("Using shared channel {}", mask_sensitive_info(stream_url));
        if app_state.shared_streams.lock().await.get(stream_url).is_some() {
            let mut response_builder = get_stream_response_with_headers(headers, stream_url);
            let current_date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            response_builder.insert_header((DATE, current_date.as_bytes()));
            // response_builder.insert_header((ACCEPT_RANGES, "bytes".as_bytes()));
            return Some(response_builder.body(BodyStream::new(stream)));
        }
    }
    None
}

pub fn is_stream_share_enabled(item_type: PlaylistItemType, target: &ConfigTarget) -> bool {
    item_type == PlaylistItemType::Live && target.options.as_ref().is_some_and(|opt| opt.share_live_streams)
}

pub type HeaderFilter = Option<Box<dyn Fn(&str) -> bool>>;
pub fn get_headers_from_request(req: &HttpRequest, filter: &HeaderFilter) -> HashMap<String, Vec<u8>> {
    req.headers()
        .iter()
        .filter(|(k, _)| match &filter {
            None => true,
            Some(predicate) => predicate(k.as_str())
        })
        .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
        .collect()
}

fn get_add_cache_content(res_url: &str, cache: &Arc<Option<Mutex<LRUResourceCache>>>) -> Box<dyn Fn(usize)> {
    let resource_url = String::from(res_url);
    let cache = Arc::clone(cache);
    let add_cache_content: Box<dyn Fn(usize)> = Box::new(move|size| {
        let res_url = resource_url.clone();
        let cache = Arc::clone(&cache);
        actix_rt::spawn(async move {
            if let Some(cache) = cache.as_ref() {
                let mut guard = cache.lock().await;
                let _ = guard.add_content(&res_url, size).await;
            }
        });
    });
    add_cache_content
}

pub async fn resource_response(app_state: &AppState, resource_url: &str, req: &HttpRequest, input: Option<&ConfigInput>) -> HttpResponse {
    if resource_url.is_empty() {
        return HttpResponse::NoContent().finish();
    }
    let filter: HeaderFilter = Some(Box::new(|key| key != "if-none-match" && key != "if-modified-since"));
    let req_headers = get_headers_from_request(req, &filter);
    if let Some(cache) = app_state.cache.as_ref() {
        let mut guard = cache.lock().await;
        if let Some(resource_path) = guard.get_content(resource_url).await {
            if let Ok(named_file) = NamedFile::open_async(resource_path).await {
                debug_if_enabled!("Cached resource {}", mask_sensitive_info(resource_url));
                return named_file.into_response(req);
            }
        }
    }
    debug_if_enabled!("Try to fetch resource {}", mask_sensitive_info(resource_url));
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

                    let byte_stream = response.bytes_stream();
                    if let Some(cache) = app_state.cache.as_ref() {
                       let resource_path = {
                            let guard = cache.lock().await;
                            guard.store_path(resource_url)
                        };
                        if let Ok(file) = create_new_file_for_write(&resource_path) {
                            let writer = Arc::new(file);
                            let add_cache_content = get_add_cache_content(resource_url, &app_state.cache);
                            let stream = PersistPipeStream::new(byte_stream, writer, add_cache_content);
                            return response_builder.body(BodyStream::new(stream));
                        }
                    }
                   return response_builder.body(BodyStream::new(byte_stream));
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
