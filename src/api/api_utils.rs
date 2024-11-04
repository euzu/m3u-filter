use std::collections::HashMap;
use std::path::{Path};
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::{HttpRequest, HttpResponse, web};
use log::{debug, error};
use url::Url;
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::api_proxy::{ApiProxyServerInfo, ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget, ConfigInput};
use crate::utils::request_utils;

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
                                                 app_state: &'a web::Data<AppState>) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
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

pub fn get_user_target<'a>(api_req: &'a UserApiRequest, app_state: &'a web::Data<AppState>) -> Option<(ProxyUserCredentials, &'a ConfigTarget)> {
    let username = api_req.username.as_str().trim();
    let password = api_req.password.as_str().trim();
    get_user_target_by_credentials(username, password, api_req, app_state)
}

pub fn get_user_server_info(cfg: &Config, user: &ProxyUserCredentials) -> ApiProxyServerInfo {
    let server_info_list = cfg.t_api_proxy.read().unwrap().as_ref().unwrap().server.clone();
    let server_info_name = user.server.as_ref().map_or("default", |server_name| server_name.as_str());
    server_info_list.iter().find(|c| c.name.eq(server_info_name)).map_or_else(|| server_info_list.first().unwrap().clone(), std::clone::Clone::clone)
}

pub async fn stream_response(stream_url: &str, req: &HttpRequest, input: Option<&ConfigInput>) -> HttpResponse {
    let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
    debug!("Try to open stream {}", stream_url);
    if let Ok(url) = Url::parse(stream_url) {
        let client = request_utils::get_client_request(input, &url, Some(&req_headers));
        match client.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let mut response_builder = HttpResponse::Ok();
                    response.headers().iter().for_each(|(k, v)| {
                        response_builder.insert_header((k.as_str(), v.as_ref()));
                    });
                    return response_builder.body(actix_web::body::BodyStream::new(response.bytes_stream()));
                }
                debug!("Failed to open stream got status {} for {}", response.status(), stream_url);
            }
            Err(err) => {
                error!("Received failure from server {}:  {}", stream_url, err);
            }
        }
    } else {
        error!("Url is malformed {}", &stream_url);
    }
    HttpResponse::BadRequest().finish()
}
