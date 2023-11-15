use std::sync::{Arc};
use actix_web::{HttpResponse, Scope, web};
use serde_json::{json};
use crate::api::api_model::{AppState, PlaylistRequest, ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::download::{get_m3u_playlist, get_xtream_playlist};
use crate::model::config::{ConfigInput, InputType, validate_targets};
use log::{error};
use crate::api::download_api::{download_file_info, queue_download_file};
use crate::config_reader::save_api_proxy;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ServerInfo, TargetUser};
use crate::processing::playlist_processor::exec_processing;

fn save_config_api_proxy(api_proxy: &mut ApiProxyConfig) -> Option<M3uFilterError> {
    match save_api_proxy(api_proxy) {
        Ok(_) => {}
        Err(err) => {
            error!("Failed to save api_proxy.yml {}", err.to_string());
            return Some(err);
        }
    }
    None
}

pub(crate) async fn save_config_api_proxy_user(
    mut req: web::Json<Vec<TargetUser>>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    req.0.iter_mut().flat_map(|t| &mut t.credentials).for_each(|c| c.trim());
    if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
        api_proxy.user = req.0;
        if let Some(err) = save_config_api_proxy(api_proxy) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
    }
    HttpResponse::Ok().finish()
}

pub(crate) async fn save_main_config(
    mut req: web::Json<ServerInfo>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    if req.0.is_valid() {
        // if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
        //     api_proxy.server = req.0;
        //     if let Some(err) = save_config_api_proxy(api_proxy) {
        //         return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        //     }
        // }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Invalid content"}))
    }
}


pub(crate) async fn save_config_api_proxy_config(
    mut req: web::Json<ServerInfo>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    if req.0.is_valid() {
        if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
            api_proxy.server = req.0;
            if let Some(err) = save_config_api_proxy(api_proxy) {
                return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
            }
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Invalid content"}))
    }
}

pub(crate) async fn playlist_update(
    req: web::Json<Vec<String>>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let targets = req.0;
    let user_targets = if targets.is_empty() { None } else { Some(targets) };
    let process_targets = validate_targets(&user_targets, &_app_state.config.sources);
    match process_targets {
        Ok(valid_targets) => {
            exec_processing(Arc::clone(&_app_state.config), Arc::new(valid_targets)).await;
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            error!("Failed playlist update {}", err.to_string());
            HttpResponse::BadRequest().json(json!({"error": err.to_string()}))
        }
    }
}

fn create_config_input_for_url(url: &str) -> ConfigInput {
    ConfigInput {
        id: 0,
        headers: Default::default(),
        input_type: InputType::M3u,
        url: String::from(url),
        epg_url: None,
        username: None,
        password: None,
        persist: None,
        prefix: None,
        suffix: None,
        name: None,
        enabled: true,
    }
}

pub(crate) async fn playlist(
    req: web::Json<PlaylistRequest>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match match &req.input_id {
        Some(input_id) => {
            _app_state.config.get_input_by_id(input_id)
        }
        None => {
            let url = req.url.as_deref().unwrap_or("");
            Some(create_config_input_for_url(url))
        }
    } {
        None => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
        Some(input) => {
            let (result, errors) =
                match input.input_type {
                    InputType::M3u => get_m3u_playlist(&_app_state.config, &input, &_app_state.config.working_dir).await,
                    InputType::Xtream => get_xtream_playlist(&input, &_app_state.config.working_dir).await,
                };
            if result.is_empty() {
                let error_strings: Vec<String> = errors.iter().map(|err| err.to_string()).collect();
                HttpResponse::BadRequest().json(json!({"error": error_strings.join(", ")}))
            } else {
                HttpResponse::Ok().json(result)
            }
        }
    }
}

pub(crate) async fn config(
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let sources: Vec<ServerSourceConfig> = _app_state.config.sources.iter()
        .map(|s| ServerSourceConfig {
            inputs: s.inputs.iter().map(|i| ServerInputConfig {
                id: i.id,
                input_type: i.input_type.clone(),
                url: i.url.clone(),
                username: i.username.clone(),
                password: i.password.clone(),
                persist: i.persist.clone(),
                name: i.name.clone(),
                enabled: i.enabled,
            }).collect(),
            targets: s.targets.iter().map(|t| ServerTargetConfig {
                id: t.id,
                enabled: t.enabled,
                name: t.name.clone(),
                options: t.options.clone(),
                sort: t.sort.clone(),
                filter: t.filter.clone(),
                output: t.output.clone(),
                rename: t.rename.clone(),
                mapping: t.mapping.clone(),
                processing_order: t.processing_order.clone(),
                watch: t.watch.clone(),
            }).collect(),
        }).collect();


    let result = ServerConfig {
        api: _app_state.config.api.clone(),
        threads: _app_state.config.threads,
        working_dir: _app_state.config.working_dir.to_owned(),
        schedule: _app_state.config.schedule.clone(),
        video: _app_state.config.video.clone(),
        sources,
        api_proxy: _app_state.config._api_proxy.read().unwrap().clone(),
    };
    HttpResponse::Ok().json(result)
}

pub(crate) fn v1_api_register() -> Scope {
    web::scope("/api/v1")
        .route("/config", web::get().to(config))
        .route("/config", web::post().to(save_main_config))
        .route("/config/user", web::post().to(save_config_api_proxy_user))
        .route("/config/apiproxy", web::post().to(save_config_api_proxy_config))
        .route("/playlist", web::post().to(playlist))
        .route("/playlist/update", web::post().to(playlist_update))
        .route("/file/download", web::post().to(queue_download_file))
        .route("/file/download/info", web::get().to(download_file_info))
}
