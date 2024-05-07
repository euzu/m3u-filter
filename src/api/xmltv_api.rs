use std::path::PathBuf;

use actix_web::{HttpRequest, HttpResponse, web};
use log::{debug, info};
use url::{ParseError, Url};

use crate::api::api_model::{AppState, UserApiRequest};
use crate::api::api_utils::{get_user_target, serve_file};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::m3u_get_epg_file_path;
use crate::repository::xtream_repository::{xtream_get_epg_file_path, xtream_get_storage_path};
use crate::utils::{file_utils, request_utils};

fn get_epg_path_for_target_of_type(target_name: &str, file_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(epg_path) = file_path {
        if file_utils::path_exists(&epg_path) {
            return Some(epg_path);
        } else {
            info!("Cant find epg file for {target_name} target: {}", epg_path.to_str().unwrap_or("?"))
        }
    }
    None
}

fn get_epg_path_for_target(config: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    for output in &target.output {
        match output.target {
            TargetType::M3u => {
                return get_epg_path_for_target_of_type(&target.name, m3u_get_epg_file_path(config, &target.get_m3u_filename()));
            }
            TargetType::Xtream => {
                if let Some(storage_path) = xtream_get_storage_path(config, &target.name) {
                    return get_epg_path_for_target_of_type(&target.name, Some(xtream_get_epg_file_path(&storage_path)));
                }
            }
            TargetType::Strm => {}
        }
    }
    None
}

async fn xmltv_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some((user, target)) = get_user_target(&api_req, &app_state) {
        match get_epg_path_for_target(&app_state.config, target) {
            None => {
                if let Some(value) = get_xmltv_raw_epg(&app_state.config, &user, &target.name).await {
                    return value;
                }
            }
            Some(epg_path) => return serve_file(&epg_path, &req, mime::TEXT_XML).await
        }
    }
    HttpResponse::Ok().content_type(mime::TEXT_XML).body(
        r#"<?xml version="1.0" encoding="utf-8" ?><!DOCTYPE tv SYSTEM "xmltv.dtd"><tv generator-info-name="Xtream Codes" generator-info-url=""></tv>"#)
}

fn get_xmltv_epg_url(input: &ConfigInput) -> Result<Url, ParseError> {
    let epg_url = input.epg_url.as_ref().map_or("".to_string(), |s| s.to_owned());
    if epg_url.is_empty() {
        if let Some(user_info) = input.get_user_info() {
            let url = user_info.base_url.as_str();
            let username = user_info.username.as_str();
            let password = user_info.password.as_str();
            Url::parse(format!("{url}/xmltv.php?username={username}&password={password}").as_str())
        } else {
            Err(ParseError::EmptyHost)
        }
    } else {
        Url::parse(epg_url.as_str())
    }
}

async fn get_xmltv_raw_epg(config: &Config, user: &ProxyUserCredentials, target_name: &str) -> Option<HttpResponse> {
    // If no epg_url is provided for input, we did not process the xmltv for our channels.
    // We are now delivering the original untouched xmltv.
    // If you want to use xmltv then provide the url in the config to filter unnecessary content.
    // If you have multiple xtream sources, no response because of mapped ids
    // if you want epg for multi xtream input, then provide  epg_url.
    if let Some(inputs) = config.get_inputs_for_target(target_name) {
        if inputs.len() == 1 {
            if let Some(&input) = inputs.first() {
                if let Ok(url) = get_xmltv_epg_url(input) {
                    if user.proxy == ProxyType::Redirect {
                        debug!("Redirecting epg request to {}", url.as_str());
                        return Some(HttpResponse::Found().insert_header(("Location", url.as_str())).finish());
                    }
                    let client = request_utils::get_client_request(Some(input), url, None);
                    if let Ok(response) = client.send().await {
                        if response.status().is_success() {
                            if let Ok(content) = response.text().await {
                                return Some(HttpResponse::Ok().content_type(mime::TEXT_XML).body(content));
                            }
                        }
                    }
                } else {
                    debug!("Could not generate epg url for {target_name}")
                }
            }
        } else {
            debug!("No epg_url is provided for target {target_name}, multi input requires epg_url")
        }
    }
    None
}

pub(crate) fn xmltv_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/xmltv.php").route(web::get().to(xmltv_api)))
        .service(web::resource("/epg").route(web::get().to(xmltv_api)));
}