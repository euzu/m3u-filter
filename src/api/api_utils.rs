use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::shared_stream::{NotifyStream, SharedStream};
use crate::model::api_proxy::{ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::utils::request_utils;
use crate::utils::request_utils::mask_sensitive_info;
use actix_web::http::header::{HeaderValue, CACHE_CONTROL};
use actix_web::{HttpRequest, HttpResponse};
use async_std::prelude::Stream;
use async_std::stream::StreamExt;
use bytes::Bytes;
use log::{debug, error, log_enabled, Level};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use url::Url;

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

async fn create_notify_stream(app_state: &AppState, stream_url: &str) -> Option<NotifyStream<BroadcastStream<Bytes>>> {
    let notify_stream_url = stream_url.to_string();
    let shared_streams_map = app_state.shared_streams.clone();
    let shared_streams = shared_streams_map.lock().await;
    if let Some(shared_stream) = shared_streams.get(&notify_stream_url) {
        let rx = shared_stream.data_stream.subscribe();
        drop(shared_streams);
        let (stream, notify) = NotifyStream::new(tokio_stream::wrappers::BroadcastStream::new(rx));
        actix_rt::spawn(async move {
            let _ = notify.await;
            let mut shared_streams = shared_streams_map.lock().await;
            if let Some(shared_stream) = shared_streams.get(&notify_stream_url) {
                let cur_count = shared_stream.client_count.fetch_sub(1u32, std::sync::atomic::Ordering::SeqCst);
                if cur_count == 1 {
                    shared_streams.remove(&notify_stream_url);
                }
            }
        });
        return Some(stream);
    }
    drop(shared_streams);
    None
}

async fn create_shared_stream<S, E>(app_state: &AppState, bytes_stream: S, stream_url: &str)
where
    S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
{
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let sender = Arc::new(tx);
    let _ = app_state.shared_streams.lock().await.insert(
        stream_url.to_string(),
        SharedStream {
            data_stream: sender.clone(),
            client_count: AtomicU32::new(1),
        },
    );
    let mut source_stream = Box::pin(bytes_stream);
    actix_rt::spawn(async move {
        while let Some(Ok(item)) = source_stream.next().await {
            if sender.send(item).is_err() {
                // ignore
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
        let shared_streams = app_state.shared_streams.lock().await;
        if let Some(shared_stream) = shared_streams.get(stream_url) {
            shared_stream.client_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    if let Ok(url) = Url::parse(stream_url) {
        let client = request_utils::get_client_request(input, &url, Some(&req_headers));
        match client.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let mut response_builder = HttpResponse::Ok();
                    response.headers().iter().for_each(|(k, v)| {
                        response_builder.insert_header((k.as_str(), v.as_ref()));
                    });
                    if share_stream {
                        create_shared_stream(app_state, response.bytes_stream(), stream_url).await;
                        if let Some(stream) = create_notify_stream(app_state, stream_url).await {
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
