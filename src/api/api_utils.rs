use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::shared_stream::SharedStream;
use crate::model::api_proxy::{ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::utils::request_utils;
use crate::utils::request_utils::mask_sensitive_info;
use actix_web::http::header::{HeaderValue, CACHE_CONTROL};
use actix_web::http::header::{DATE};
use actix_web::{HttpRequest, HttpResponse};
use async_std::prelude::Stream;
use async_std::stream::StreamExt;
use bytes::Bytes;
use chrono::Utc;
use log::{debug, error, log_enabled, Level};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use url::Url;
use crate::model::playlist::PlaylistItemType;

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

pub fn get_user_server_info(cfg: &Config, user: &ProxyUserCredentials) -> ApiProxyServerInfo {
    let server_info_list = cfg.t_api_proxy.read().unwrap().as_ref().unwrap().server.clone();
    let server_info_name = user.server.as_ref().map_or("default", |server_name| server_name.as_str());
    server_info_list.iter().find(|c| c.name.eq(server_info_name)).map_or_else(|| server_info_list.first().unwrap().clone(), std::clone::Clone::clone)
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

/// Creates a shared stream and stores it in the shared state.
async fn create_shared_stream<S, E>(
    app_state: &AppState,
    stream_url: &str,
    header: HashMap<String, Vec<u8>>,
    bytes_stream: S,
) where
    S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
{
    // Create a broadcast channel for the shared stream
    let (tx, _) = broadcast::channel(100);
    let sender = Arc::new(tx);

    // Insert the shared stream into the shared state
    app_state
        .shared_streams
        .lock()
        .await
        .insert(
            stream_url.to_string(),
            SharedStream {
                data_stream: sender.clone(),
                header,
            },
        );

    let shared_streams_map = Arc::clone(&app_state.shared_streams);
    let mut source_stream = Box::pin(bytes_stream);
    let streaming_url = stream_url.to_string();
    // Spawn a task to forward items from the source stream to the broadcast channel
    actix_rt::spawn(async move {
        while let Some(item) = source_stream.next().await {
            if let Ok(data) = item {
                if sender.receiver_count() > 0 {
                    // if let Err(err) = sender.send(data) {
                    //     debug!("{err}")
                    // }
                    if sender.send(data).is_err() {
                        // ignore
                    }
                    actix_web::rt::time::sleep(Duration::from_millis(20)).await;
                } else {
                    if log_enabled!(Level::Debug) {
                        debug!("No active subscribers. Closing stream {}", mask_sensitive_info(&streaming_url));
                    }
                    // Cleanup for removing unused shared streams
                    let mut shared_streams = shared_streams_map.lock().await;
                    shared_streams.remove(&streaming_url);
                    return;
                }
            }
        }
    });
}

pub async fn stream_response(app_state: &AppState, stream_url: &str, req: &HttpRequest, input: Option<&ConfigInput>, share_stream: bool) -> HttpResponse {
    let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
    if log_enabled!(Level::Debug) {
        debug!("Try to open stream {}", mask_sensitive_info(stream_url));
    }
    if share_stream {
        if let Some(value) = shared_stream_response(app_state, stream_url).await {
            return value;
        }
    }

    if let Ok(url) = Url::parse(stream_url) {
        let client = request_utils::get_client_request(input, &url, Some(&req_headers));
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let mut response_builder = HttpResponse::Ok();
                    let mut header = HashMap::new();
                    response.headers().iter().for_each(|(k, v)| {
                        let key = k.as_str().to_string();
                        // ignore date it is dynamic
                        if !"date".eq(key.to_lowercase().as_str()) {
                            header.insert(k.as_str().to_string(), v.as_bytes().to_vec());
                        }
                        response_builder.insert_header((k.as_str(), v.as_ref()));
                    });
                    if share_stream {
                        create_shared_stream(app_state, stream_url, header, response.bytes_stream()).await;
                        if let Some(stream) = create_notify_stream(app_state, stream_url).await {
                            debug!("Creating shared channel  {stream_url}");
                            return response_builder.body(actix_web::body::BodyStream::new(stream));
                        }
                    } else {
                        return response_builder.body(actix_web::body::BodyStream::new(response.bytes_stream()));
                    }
                }
                if log_enabled!(Level::Debug) {
                    debug!("Failed to open stream got status {} for {}", status, mask_sensitive_info(stream_url));
                }
            }
            Err(err) => {
                error!("Received failure from server {}:  {}", mask_sensitive_info(stream_url), err);
            }
        }
    } else {
        error!("Url is malformed {}", mask_sensitive_info(stream_url));
    }
    HttpResponse::BadRequest().finish()
}

async fn shared_stream_response(app_state: &AppState, stream_url: &str) -> Option<HttpResponse> {
    if let Some(stream) = create_notify_stream(app_state, stream_url).await {
        if log_enabled!(Level::Debug) {
            debug!("Using shared channel {}", mask_sensitive_info(stream_url));
        }
        // return HttpResponse::Ok().body(actix_web::body::BodyStream::new(stream));
        if let Some(shared_stream) = app_state.shared_streams.lock().await.get(stream_url) {
            let mut response_builder = HttpResponse::Ok();
            for (key, value) in &shared_stream.header {
                response_builder.insert_header((key.as_str(), &value[..]));
            }
            let current_date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            response_builder.insert_header((DATE, current_date.as_bytes()));
            response_builder.insert_header((CACHE_CONTROL, "no-cache".as_bytes()));
            // response_builder.insert_header((ACCEPT_RANGES, "bytes".as_bytes()));
            return Some(response_builder.body(actix_web::body::BodyStream::new(stream)));
        }
    }
    None
}

pub fn is_stream_share_enabled(item_type: PlaylistItemType, target: &ConfigTarget) -> bool {
    item_type == PlaylistItemType::Live && target.options.as_ref().map_or(false, |opt| opt.share_live_streams)
}