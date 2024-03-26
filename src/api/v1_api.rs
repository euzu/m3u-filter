use std::sync::{Arc};
use actix_web::{HttpResponse, Scope, web};
use serde_json::{json};
use crate::api::api_model::{AppState, PlaylistRequest, ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::model::config::{Config, ConfigDto, ConfigInput, ConfigInputOptions, ConfigSource, ConfigTarget, InputType, validate_targets};
use log::{error};
use crate::api::download_api::{download_file_info, queue_download_file};
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, TargetUser};
use crate::processing::playlist_processor::exec_processing;
use crate::utils::{config_reader, download};

fn _save_config_api_proxy(backup_dir: &str, api_proxy: &mut ApiProxyConfig) -> Option<M3uFilterError> {
    match config_reader::save_api_proxy(api_proxy._file_path.as_str(), backup_dir, api_proxy) {
        Ok(_) => {}
        Err(err) => {
            error!("Failed to save api_proxy.yml {}", err.to_string());
            return Some(err);
        }
    }
    None
}

fn _save_config_main(file_path: &str, backup_dir: &str, cfg: &ConfigDto) -> Option<M3uFilterError> {
    match config_reader::save_main_config(file_path, backup_dir, cfg) {
        Ok(_) => {}
        Err(err) => {
            error!("Failed to save config.yml {}", err.to_string());
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
        let backup_dir = _app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = _save_config_api_proxy(backup_dir, api_proxy) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
    }
    HttpResponse::Ok().finish()
}

pub(crate) async fn save_config_main(
    req: web::Json<ConfigDto>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    let cfg = req.0;
    if cfg.is_valid() {
        let file_path = _app_state.config._config_file_path.as_str();
        let backup_dir = _app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = _save_config_main(file_path, backup_dir, &cfg) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Invalid content"}))
    }
}


pub(crate) async fn save_config_api_proxy_config(
    req: web::Json<Vec<ApiProxyServerInfo>>,
    mut _app_state: web::Data<AppState>,
) -> HttpResponse {
    let mut req_api_proxy = req.0;
    for server_info in &mut req_api_proxy {
        if !server_info.is_valid() {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid content"}));
        }
    }
    if let Some(api_proxy) = _app_state.config._api_proxy.write().unwrap().as_mut() {
        api_proxy.server = req_api_proxy;
        let backup_dir = _app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = _save_config_api_proxy(backup_dir, api_proxy) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
    }
    HttpResponse::Ok().finish()
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
            actix_rt::spawn(exec_processing(Arc::clone(&_app_state.config), Arc::new(valid_targets)));
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
        options: Some(ConfigInputOptions {
            xtream_info_cache: false,
        }),
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
                    InputType::M3u => download::get_m3u_playlist(&_app_state.config, &input, &_app_state.config.working_dir).await,
                    InputType::Xtream => download::get_xtream_playlist(&input, &_app_state.config.working_dir).await,
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
    let map_input = |i: &ConfigInput| ServerInputConfig {
        id: i.id,
        input_type: i.input_type.clone(),
        url: i.url.clone(),
        username: i.username.clone(),
        password: i.password.clone(),
        persist: i.persist.clone(),
        name: i.name.clone(),
        enabled: i.enabled,
    };

    let map_target = |t: &ConfigTarget| ServerTargetConfig {
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
    };

    let map_source = |s: &ConfigSource| ServerSourceConfig {
        inputs: s.inputs.iter().map(map_input).collect(),
        targets: s.targets.iter().map(map_target).collect(),
    };

    let map_config = |config: &Config| ServerConfig {
        api: config.api.clone(),
        threads: config.threads,
        working_dir: config.working_dir.to_owned(),
        backup_dir: config.backup_dir.to_owned(),
        schedule: config.schedule.clone(),
        messaging: config.messaging.clone(),
        video: config.video.clone(),
        sources: config.sources.iter().map(map_source).collect(),
        api_proxy: config._api_proxy.read().unwrap().clone(),
    };

    let mut result = match config_reader::read_config(_app_state.config._config_path.as_str(),
                                       _app_state.config._config_file_path.as_str(),
                                       _app_state.config._sources_file_path.as_str()) {
        Ok(mut cfg) => {
            let _ = cfg.prepare();
            map_config(&cfg)
        }
        Err(_) => map_config(&_app_state.config)
    };

    // if we didn't read it from file then we should use it from app_state
    if result.api_proxy.is_none() {
        result.api_proxy = _app_state.config._api_proxy.read().unwrap().clone();
    }

    HttpResponse::Ok().json(result)
}

pub(crate) fn v1_api_register() -> Scope {
    web::scope("/api/v1")
        .route("/config", web::get().to(config))
        .route("/config/main", web::post().to(save_config_main))
        .route("/config/user", web::post().to(save_config_api_proxy_user))
        .route("/config/apiproxy", web::post().to(save_config_api_proxy_config))
        .route("/playlist", web::post().to(playlist))
        .route("/playlist/update", web::post().to(playlist_update))
        .route("/file/download", web::post().to(queue_download_file))
        .route("/file/download/info", web::get().to(download_file_info))
}
