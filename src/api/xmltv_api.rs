use std::path::{PathBuf};
use actix_web::{HttpRequest, HttpResponse, Resource, web};
use log::{debug, info};
use url::Url;

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::api_proxy::ProxyType;
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::get_m3u_epg_file_path;
use crate::repository::xtream_repository::{get_xtream_epg_file_path, get_xtream_storage_path};
use crate::utils::{file_utils, request_utils};

fn get_epg_path_for_target(config: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    for output in &target.output {
        match output.target {
            TargetType::M3u => {
                if let Some(epg_path) = get_m3u_epg_file_path(config, &target.get_m3u_filename()) {
                    if file_utils::path_exists(&epg_path) {
                        return Some(epg_path);
                    } else {
                        info!("Cant find epg file for m3u target: {}", epg_path.to_str().unwrap_or("?"))
                    }
                }
            }
            TargetType::Xtream => {
                if let Some(storage_path) = get_xtream_storage_path(config, &target.name) {
                    let epg_path = get_xtream_epg_file_path(&storage_path);
                    if file_utils::path_exists(&epg_path) {
                        return Some(epg_path);
                    } else {
                        info!("Cant find epg file for xtream target: {}", epg_path.to_str().unwrap_or("?"))
                    }
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
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some((user, target)) = get_user_target(&api_req, &_app_state) {
        match get_epg_path_for_target(&_app_state.config, target) {
            None => {
                // If no epg_url is provided for input, we did not process the xmltv for our channels.
                // We are now delivering the original untouched xmltv.
                // If you want to use xmltv then provide the url in the config to filter unnecessary content.
                // If you have multiple xtream sources, no response because of mapped ids
                // if you want epg for multi xtream input, then provide  epg_url.
                let target_name = &target.name;
                if let Some(inputs) = _app_state.config.get_input_for_target(target_name, &InputType::Xtream) {
                    if inputs.len() == 1 {
                        if let Some(&input) = inputs.first() {
                            let epg_url = input.epg_url.as_ref().map_or("".to_string(), |s| s.to_owned());
                            let api_url = if epg_url.is_empty() {
                                format!("{}/xmltv.php?username={}&password={}",
                                        input.url.as_str(),
                                        input.username.as_ref().unwrap_or(&"".to_string()).as_str(),
                                        input.password.as_ref().unwrap_or(&"".to_string()).as_str(),
                                )
                            } else { epg_url.to_string() };
                            if let Ok(url) = Url::parse(&api_url) {
                                if user.proxy == ProxyType::Redirect {
                                    debug!("Redirecting epg request to {}", api_url);
                                    return HttpResponse::Found().insert_header(("Location", api_url)).finish();
                                }
                                let client = request_utils::get_client_request(input, url, None);
                                if let Ok(response) = client.send().await {
                                    if response.status().is_success() {
                                        if let Ok(content) = response.text().await {
                                            return HttpResponse::Ok().content_type(mime::TEXT_XML).body(content);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(epg_path) => return serve_file(&epg_path, &req).await
        }
    }
    HttpResponse::Ok().content_type(mime::TEXT_XML).body(
        r#"<?xml version="1.0" encoding="utf-8" ?><!DOCTYPE tv SYSTEM "xmltv.dtd"><tv generator-info-name="Xtream Codes" generator-info-url=""></tv>"#)
}

pub(crate) fn xmltv_api_register() -> Vec<Resource> {
    vec![
        web::resource("/xmltv.php").route(web::get().to(xmltv_api)),
        web::resource("/epg").route(web::get().to(xmltv_api)),
    ]
}