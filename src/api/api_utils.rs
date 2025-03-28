use crate::api::model::active_provider_manager::{ProviderAllocation, ProviderConfig};
use crate::api::model::app_state::AppState;
use crate::api::model::model_utils::get_stream_response_with_headers;
use crate::api::model::request::UserApiRequest;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::active_client_stream::ActiveClientStream;
use crate::api::model::streams::persist_pipe_stream::PersistPipeStream;
use crate::api::model::streams::provider_stream;
use crate::api::model::streams::provider_stream::create_provider_connections_exhausted_stream;
use crate::api::model::streams::provider_stream_factory::BufferStreamOptions;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::auth::authenticator::Claims;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, ConfigTarget};
use crate::model::playlist::PlaylistItemType;
use crate::tools::lru_cache::LRUResourceCache;
use crate::utils::file::file_utils::create_new_file_for_write;
use crate::utils::network::request;
use crate::utils::network::request::sanitize_sensitive_info;
use crate::utils::{debug_if_enabled, trace_if_enabled};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{StreamExt, TryStreamExt};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use log::{debug, error, log_enabled, trace};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

#[macro_export]
macro_rules! try_option_bad_request {
    ($option:expr, $msg_is_error:expr, $msg:expr) => {
        match $option {
            Some(value) => value,
            None => {
                if $msg_is_error {error!("{}", $msg);} else {debug!("{}", $msg);}
                return axum::http::StatusCode::BAD_REQUEST.into_response();
            }
        }
    };
    ($option:expr) => {
        match $option {
            Some(value) => value,
            None => return axum::http::StatusCode::BAD_REQUEST.into_response(),
        }
    };
}

#[macro_export]
macro_rules! try_result_bad_request {
    ($option:expr, $msg_is_error:expr, $msg:expr) => {
        match $option {
            Ok(value) => value,
            Err(_) => {
                if $msg_is_error {error!("{}", $msg);} else {debug!("{}", $msg);}
                return axum::http::StatusCode::BAD_REQUEST.into_response();
            }
        }
    };
    ($option:expr) => {
        match $option {
            Ok(value) => value,
            Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response(),
        }
    };
}

pub use try_option_bad_request;
pub use try_result_bad_request;
use crate::api::model::stream::{BoxedProviderStream, ProviderStreamInfo, ProviderStreamResponse};
use crate::api::model::streams::throttled_stream::ThrottledStream;
use crate::tools::atomic_once_flag::AtomicOnceFlag;
use crate::utils::default_utils::default_grace_period_millis;

pub async fn serve_file(file_path: &Path, mime_type: mime::Mime) -> impl axum::response::IntoResponse + Send {
    if file_path.exists() {
        return match tokio::fs::File::open(file_path).await {
            Ok(file) => {
                let reader = tokio::io::BufReader::new(file);
                let stream = tokio_util::io::ReaderStream::new(reader);
                let body = axum::body::Body::from_stream(stream);

                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, mime_type.to_string())
                    .header(axum::http::header::CACHE_CONTROL, axum::http::header::HeaderValue::from_static("no-cache"))
                    .body(body)
                    .unwrap()
                    .into_response()
            }
            Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    }
    axum::http::StatusCode::NOT_FOUND.into_response()
}

pub async fn get_user_target_by_username<'a>(username: &str, app_state: &'a AppState) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    if !username.is_empty() {
        return app_state.config.get_target_for_username(username).await;
    }
    None
}

pub async fn get_user_target_by_credentials<'a>(username: &str, password: &str, api_req: &'a UserApiRequest,
                                                app_state: &'a AppState) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    if !username.is_empty() && !password.is_empty() {
        app_state.config.get_target_for_user(username, password).await
    } else {
        let token = api_req.token.as_str().trim();
        if token.is_empty() {
            None
        } else {
            app_state.config.get_target_for_user_by_token(token).await
        }
    }
}

pub async fn get_user_target<'a>(api_req: &'a UserApiRequest, app_state: &'a AppState) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    let username = api_req.username.as_str().trim();
    let password = api_req.password.as_str().trim();
    get_user_target_by_credentials(username, password, api_req, app_state).await
}

pub struct StreamOptions {
    pub stream_retry: bool,
    pub stream_force_retry_secs: u32,
    pub buffer_enabled: bool,
    pub buffer_size: usize,
    pub pipe_provider_stream: bool,
}

fn get_stream_options(app_state: &AppState) -> StreamOptions {
    let (stream_retry, stream_force_retry_secs, buffer_enabled, buffer_size) = app_state
        .config
        .reverse_proxy
        .as_ref()
        .and_then(|reverse_proxy| reverse_proxy.stream.as_ref())
        .map_or((false, 0, false, 0), |stream| {
            let (buffer_enabled, buffer_size) = stream
                .buffer
                .as_ref()
                .map_or((false, 0), |buffer| (buffer.enabled, buffer.size));
            (stream.retry, stream.forced_retry_interval_secs, buffer_enabled, buffer_size)
        });
    let pipe_provider_stream = !stream_retry && !buffer_enabled;
    StreamOptions { stream_retry, stream_force_retry_secs, buffer_enabled, buffer_size, pipe_provider_stream }
}

// fn get_stream_content_length(provider_response: Option<&(Vec<(String, String)>, StatusCode)>) -> u64 {
//     let content_length = provider_response
//         .as_ref()
//         .and_then(|(headers, _)| headers.iter().find(|(h, _)| h.eq(axum::http::header::CONTENT_LENGTH.as_str())))
//         .and_then(|(_, val)| val.parse::<u64>().ok())
//         .unwrap_or(0);
//     content_length
// }

fn get_stream_alternative_url(stream_url: &str, input: &ConfigInput, alias_input: &ProviderConfig) -> String {
    let Some(input_user_info) = input.get_user_info() else { return stream_url.to_owned() };
    let Some(alt_input_user_info) = alias_input.get_user_info() else { return stream_url.to_owned() };

    let modified = stream_url.replace(&input_user_info.base_url, &alt_input_user_info.base_url);
    let modified = modified.replace(&input_user_info.username, &alt_input_user_info.username);
    let modified = modified.replace(&input_user_info.password, &alt_input_user_info.password);
    modified
}

type StreamUrl = String;
type ProviderName = String;

enum StreamingOption {
    CustomStream(ProviderStreamResponse),
    AvailableStream(Option<ProviderName>, StreamUrl),
    GracePeriodStream(Option<ProviderName>, StreamUrl),
}

pub struct StreamDetails {
    pub stream: Option<BoxedProviderStream>,
    stream_info: ProviderStreamInfo,
    pub input_name: Option<String>,
    pub grace_period_millis: u64,
    pub reconnect_flag: Option<Arc<AtomicOnceFlag>>,
}

impl StreamDetails {
    pub fn from_stream(stream: BoxedProviderStream) -> Self {
        Self {
            stream: Some(stream),
            stream_info: None,
            input_name: None,
            grace_period_millis: default_grace_period_millis(),
            reconnect_flag: None,
        }
    }
    #[inline]
    pub fn has_stream(&self) -> bool {
        self.stream.is_some()
    }

    #[inline]
    pub fn has_grace_period(&self) -> bool {
        self.grace_period_millis > 0
    }
}

/**
* If successfully a provider connection is used, do not forget to release if unsuccessfully
*/
fn get_streaming_options(app_state: &AppState, stream_url: &str, input_opt: Option<&ConfigInput>)
                         -> (StreamingOption, Option<HashMap<String, String>>) {
    if let Some(input) = input_opt {
        let allocation = app_state.active_provider.acquire_connection(&input.name);
        let stream_response_params = match allocation {
            ProviderAllocation::Exhausted => {
                let stream = create_provider_connections_exhausted_stream(&app_state.config, &[]);
                StreamingOption::CustomStream(stream)
            }
            ProviderAllocation::Available(provider)
            | ProviderAllocation::GracePeriod(provider) => {
                let (provider, url) = if provider.id != input.id {
                    (provider.name.to_string(), get_stream_alternative_url(stream_url, input, &provider))
                } else {
                    (input.name.to_string(), stream_url.to_string())
                };

                if matches!(allocation, ProviderAllocation::Available(_)) {
                    StreamingOption::AvailableStream(Some(provider), url)
                } else {
                    StreamingOption::GracePeriodStream(Some(provider), url)
                }
            }
        };
        (stream_response_params, Some(input.headers.clone()))
    } else {
        (StreamingOption::AvailableStream(None, stream_url.to_string()), None)
    }
}

async fn create_stream_response_details(app_state: &AppState, stream_options: &StreamOptions, stream_url: &str,
                                        req_headers: &HeaderMap, input_opt: Option<&ConfigInput>,
                                        item_type: PlaylistItemType, share_stream: bool) -> StreamDetails {
    let (stream_response_params, input_headers) = get_streaming_options(app_state, stream_url, input_opt);
    let config_grace_period_millis = app_state.config.reverse_proxy.as_ref().and_then(|r| r.stream.as_ref()).map(|s| s.grace_period_millis).unwrap_or_else(default_grace_period_millis);
    let grace_period_millis = if config_grace_period_millis > 0 && matches!(stream_response_params, StreamingOption::GracePeriodStream(_, _)) { config_grace_period_millis } else { 0 };
    match stream_response_params {
        StreamingOption::CustomStream(provider_stream) => {
            let (stream, stream_info) = provider_stream;
            StreamDetails {
                stream,
                stream_info,
                input_name: None,
                grace_period_millis,
                reconnect_flag: None,
            }
        }
        StreamingOption::AvailableStream(provider_name, request_url)
        | StreamingOption::GracePeriodStream(provider_name, request_url) => {
            let parsed_url = Url::parse(&request_url);
            let ((stream, stream_info), reconnect_flag) = if let Ok(url) = parsed_url {
                if stream_options.pipe_provider_stream {
                    (provider_stream::get_provider_pipe_stream(app_state, &url, req_headers, input_headers, item_type).await, None)
                } else {
                    let buffer_stream_options = BufferStreamOptions::new(item_type, share_stream, &stream_options);
                    let reconnect_flag = buffer_stream_options.get_reconnect_flag_clone();
                    (provider_stream::get_provider_reconnect_buffered_stream(app_state, &url, req_headers, input_headers, buffer_stream_options).await,
                     Some(reconnect_flag))
                }
            } else {
                ((None, None), None)
            };

            // if we have no stream we should release the provider
            if stream.is_none() {
                if let Some(alt_input_name) = &provider_name {
                    app_state.active_provider.release_connection(alt_input_name);
                }
            }

            if log_enabled!(log::Level::Debug) {
                if let Some((headers, status_code)) = stream_info.as_ref() {
                    debug!(
                        "Responding stream request {} with status {}, headers {:?}",
                        sanitize_sensitive_info(&request_url),
                        status_code,
                        headers
                    );
                }
            }

            StreamDetails {
                stream,
                stream_info,
                input_name: provider_name,
                grace_period_millis,
                reconnect_flag,
            }
        }
    }
}

pub async fn stream_response(app_state: &AppState,
                             stream_url: &str,
                             req_headers: &HeaderMap,
                             input: Option<&ConfigInput>,
                             item_type: PlaylistItemType,
                             target: &ConfigTarget,
                             user: &ProxyUserCredentials) -> impl axum::response::IntoResponse + Send {
    if log_enabled!(log::Level::Trace) { trace!("Try to open stream {}", sanitize_sensitive_info(stream_url)); }

    let share_stream = is_stream_share_enabled(item_type, target);
    if share_stream {
        if let Some(value) = shared_stream_response(app_state, stream_url, user).await {
            return value.into_response();
        }
    }

    let stream_options = get_stream_options(app_state);
    let stream_details =
        create_stream_response_details(app_state, &stream_options, &stream_url, req_headers, input, item_type, share_stream).await;

    if stream_details.has_stream() {
        // let content_length = get_stream_content_length(provider_response.as_ref());
        let provider_response = stream_details.stream_info.as_ref().map_or(None, |(h, sc)| Some((h.clone(), sc.clone())));
        let stream = ActiveClientStream::new(stream_details, app_state, &user).await;
        let stream_resp = if share_stream {
            // Shared Stream response
            let shared_headers = provider_response.as_ref().map_or_else(Vec::new, |(h, _)| h.clone());
            SharedStreamManager::subscribe(app_state, stream_url, stream, shared_headers, stream_options.buffer_size).await;
            if let Some(broadcast_stream) = SharedStreamManager::subscribe_shared_stream(app_state, stream_url).await {
                let (status_code, header_map) = get_stream_response_with_headers(provider_response);
                let mut response = axum::response::Response::builder()
                    .status(status_code);
                for (key, value) in &header_map {
                    response = response.header(key, value);
                }
                response.body(axum::body::Body::from_stream(broadcast_stream)).unwrap().into_response()
            } else {
                axum::http::StatusCode::BAD_REQUEST.into_response()
            }
        } else {
            let (status_code, header_map) = get_stream_response_with_headers(provider_response);
            let mut response = axum::response::Response::builder()
                .status(status_code);
            for (key, value) in &header_map {
                response = response.header(key, value);
            }

            let throttle_kbps = get_stream_throttle(app_state);

            let body_stream = if throttle_kbps > 0 && matches!(item_type, PlaylistItemType::Video | PlaylistItemType::Series  | PlaylistItemType::SeriesInfo) {
                axum::body::Body::from_stream(ThrottledStream::new(stream.boxed(), throttle_kbps as usize))
            } else {
                axum::body::Body::from_stream(stream)
            };
            response.body(body_stream).unwrap().into_response()
            // if content_length > 0 { response_builder.body(SizedStream::new(content_length, stream)) } else { response_builder.streaming(stream) }
        };

        return stream_resp.into_response();
    }

    error!("Cant open stream {}", sanitize_sensitive_info(stream_url));
    axum::http::StatusCode::BAD_REQUEST.into_response()
}

fn get_stream_throttle(app_state: &AppState) -> u64 {
    app_state.config
        .reverse_proxy
        .as_ref()
        .and_then(|reverse_proxy| reverse_proxy.stream.as_ref())
        .map(|stream| stream.throttle_kbps).unwrap_or_default()
}

async fn shared_stream_response(app_state: &AppState, stream_url: &str, user: &ProxyUserCredentials) -> Option<impl IntoResponse> {
    if let Some(stream) = SharedStreamManager::subscribe_shared_stream(app_state, stream_url).await {
        debug_if_enabled!("Using shared channel {}", sanitize_sensitive_info(stream_url));
        if let Some(headers) = app_state.shared_stream_manager.get_shared_state_headers(stream_url).await {
            let (status_code, header_map) = get_stream_response_with_headers(Some((headers.clone(), StatusCode::OK)));
            let stream_details = StreamDetails::from_stream(stream);
            let stream = ActiveClientStream::new(stream_details, app_state, &user).await.boxed();
            let mut response = axum::response::Response::builder()
                .status(status_code);
            for (key, value) in &header_map {
                response = response.header(key, value);
            }
            return Some(response.body(axum::body::Body::from_stream(stream)).unwrap());
        }
    }
    None
}

pub fn is_stream_share_enabled(item_type: PlaylistItemType, target: &ConfigTarget) -> bool {
    (item_type == PlaylistItemType::Live  /* || item_type == PlaylistItemType::LiveHls */) && target.options.as_ref().is_some_and(|opt| opt.share_live_streams)
}

pub type HeaderFilter = Option<Box<dyn Fn(&str) -> bool + Send>>;
pub fn get_headers_from_request(req_headers: &HeaderMap, filter: &HeaderFilter) -> HashMap<String, Vec<u8>> {
    req_headers
        .iter()
        .filter(|(k, _)| match &filter {
            None => true,
            Some(predicate) => predicate(k.as_str())
        })
        .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
        .collect()
}

fn get_add_cache_content(res_url: &str, cache: &Arc<Option<Mutex<LRUResourceCache>>>) -> Arc<dyn Fn(usize) + Send + Sync> {
    let resource_url = String::from(res_url);
    let cache = Arc::clone(cache);
    let add_cache_content: Arc<dyn Fn(usize) + Send + Sync> = Arc::new(move |size| {
        let res_url = resource_url.clone();
        let cache = Arc::clone(&cache);
        tokio::spawn(async move {
            if let Some(cache) = cache.as_ref() {
                let _ = cache.lock().await.add_content(&res_url, size);
            }
        });
    });
    add_cache_content
}

pub async fn resource_response(app_state: &AppState, resource_url: &str, req_headers: &HeaderMap, input: Option<&ConfigInput>) -> impl axum::response::IntoResponse + Send {
    if resource_url.is_empty() {
        return axum::http::StatusCode::NO_CONTENT.into_response();
    }
    let filter: HeaderFilter = Some(Box::new(|key| key != "if-none-match" && key != "if-modified-since"));
    let req_headers = get_headers_from_request(req_headers, &filter);
    if let Some(cache) = app_state.cache.as_ref() {
        let mut guard = cache.lock().await;
        if let Some(resource_path) = guard.get_content(resource_url) {
            trace_if_enabled!("Responding resource from cache {}", sanitize_sensitive_info(resource_url));
            return serve_file(&resource_path, mime::APPLICATION_OCTET_STREAM).await.into_response();
        }
    }
    trace_if_enabled!("Try to fetch resource {}", sanitize_sensitive_info(resource_url));
    if let Ok(url) = Url::parse(resource_url) {
        let client = request::get_client_request(&app_state.http_client, input.map(|i| &i.headers), &url, Some(&req_headers));
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let mut response_builder = axum::response::Response::builder()
                        .status(StatusCode::OK);
                    for (key, value) in response.headers() {
                        response_builder = response_builder.header(key, value);
                    }

                    let byte_stream = response.bytes_stream().map_err(|err| StreamError::reqwest(&err));
                    if let Some(cache) = app_state.cache.as_ref() {
                        let resource_path = cache.lock().await.store_path(resource_url);
                        if let Ok(file) = create_new_file_for_write(&resource_path) {
                            let writer = BufWriter::new(file);
                            let add_cache_content = get_add_cache_content(resource_url, &app_state.cache);
                            let stream = PersistPipeStream::new(byte_stream, writer, add_cache_content);
                            return response_builder.body(axum::body::Body::from_stream(stream)).unwrap().into_response();
                        }
                    }
                    return response_builder.body(axum::body::Body::from_stream(byte_stream)).unwrap().into_response();
                }
                debug_if_enabled!("Failed to open resource got status {} for {}", status, sanitize_sensitive_info(resource_url));
            }
            Err(err) => {
                error!("Received failure from server {}:  {}", sanitize_sensitive_info(resource_url), err);
            }
        }
    } else {
        error!("Url is malformed {}", sanitize_sensitive_info(resource_url));
    }
    axum::http::StatusCode::BAD_REQUEST.into_response()
}

pub fn separate_number_and_remainder(input: &str) -> (String, Option<String>) {
    input.rfind('.').map_or_else(|| (input.to_string(), None), |dot_index| {
        let number_part = input[..dot_index].to_string();
        let rest = input[dot_index..].to_string();
        (number_part, if rest.len() < 2 { None } else { Some(rest) })
    })
}

pub fn empty_json_list_response() -> impl axum::response::IntoResponse + Send {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime::APPLICATION_JSON.to_string())
        .body("[]".to_string())
        .unwrap()
        .into_response()
}

pub fn get_username_from_auth_header(
    token: &str,
    app_state: &Arc<AppState>,
) -> Option<String> {
    if let Some(web_auth_config) = &app_state.config.web_auth {
        let secret_key: &str = web_auth_config.secret.as_ref();
        if let Ok(token_data) = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret_key.as_bytes()),
            &Validation::new(Algorithm::HS256),
        ) {
            return Some(token_data.claims.username);
        }
    }
    None
}

pub fn redirect(url: &str) -> impl IntoResponse {
    axum::response::Response::builder()
        .status(StatusCode::FOUND)
        .header("Location", url)
        .body(axum::body::Body::empty())
        .unwrap()
}
