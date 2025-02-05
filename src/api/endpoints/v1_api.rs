use std::sync::Arc;

use actix_web::middleware::Condition;
use actix_web::{web, HttpResponse};
use actix_web_httpauth::middleware::HttpAuthentication;
use log::error;
use serde_json::json;

use crate::api::endpoints::download_api;
use crate::api::model::app_state::AppState;
use crate::api::model::config::{ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::api::model::request::PlaylistRequest;
use crate::auth::authenticator::validator;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials, TargetUser};
use crate::model::config::{validate_targets, Config, ConfigDto, ConfigInput, ConfigInputOptions, ConfigSource, ConfigTarget, InputType};
use crate::processing::processor::playlist;
use crate::utils::network::request::sanitize_sensitive_info;
use crate::utils::config_reader;
use crate::utils::network::{m3u, xtream};

fn intern_save_config_api_proxy(backup_dir: &str, api_proxy: &ApiProxyConfig, file_path: &str) -> Option<M3uFilterError> {
    match config_reader::save_api_proxy(file_path, backup_dir, api_proxy) {
        Ok(()) => {}
        Err(err) => {
            error!("Failed to save api_proxy.yml {}", err.to_string());
            return Some(err);
        }
    }
    None
}

fn intern_save_config_main(file_path: &str, backup_dir: &str, cfg: &ConfigDto) -> Option<M3uFilterError> {
    match config_reader::save_main_config(file_path, backup_dir, cfg) {
        Ok(()) => {}
        Err(err) => {
            error!("Failed to save config.yml {}", err.to_string());
            return Some(err);
        }
    }
    None
}

async fn save_config_api_proxy_user(
    req: web::Json<Vec<TargetUser>>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let mut users = req.0;
    users.iter_mut().flat_map(|t| &mut t.credentials).for_each(ProxyUserCredentials::trim);
    if let Some(api_proxy) = app_state.config.t_api_proxy.write().await.as_mut() {
        let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
        api_proxy.user = users;
        if let Some(err) = intern_save_config_api_proxy(backup_dir, api_proxy, app_state.config.t_api_proxy_file_path.as_str()) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
        api_proxy.user.iter_mut().flat_map(|t| &mut t.credentials).for_each(|c| c.prepare(true));
    }
    HttpResponse::Ok().finish()
}

async fn save_config_main(
    req: web::Json<ConfigDto>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let cfg = req.0;
    if cfg.is_valid() {
        let file_path = app_state.config.t_config_file_path.as_str();
        let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = intern_save_config_main(file_path, backup_dir, &cfg) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().json(json!({"error": "Invalid content"}))
    }
}

async fn save_config_api_proxy_config(
    req: web::Json<Vec<ApiProxyServerInfo>>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let mut req_api_proxy = req.0;
    for server_info in &mut req_api_proxy {
        if !server_info.is_valid() {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid content"}));
        }
    }
    if let Some(api_proxy) = app_state.config.t_api_proxy.write().await.as_mut() {
        api_proxy.server = req_api_proxy;
        let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = intern_save_config_api_proxy(backup_dir, api_proxy, app_state.config.t_api_proxy_file_path.as_str()) {
            return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
        }
    }
    HttpResponse::Ok().finish()
}

async fn playlist_update(
    req: web::Json<Vec<String>>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let targets = req.0;
    let user_targets = if targets.is_empty() { None } else { Some(targets) };
    let process_targets = validate_targets(user_targets.as_ref(), &app_state.config.sources);
    match process_targets {
        Ok(valid_targets) => {
            actix_rt::spawn(playlist::exec_processing(Arc::clone(&app_state.http_client), Arc::clone(&app_state.config), Arc::new(valid_targets)));
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            error!("Failed playlist update {}", sanitize_sensitive_info(err.to_string().as_str()));
            HttpResponse::BadRequest().json(json!({"error": err.to_string()}))
        }
    }
}

fn create_config_input_for_url(name: &str, url: &str) -> ConfigInput {
    ConfigInput {
        id: 0,
        name: String::from(name),
        input_type: InputType::M3u,
        url: String::from(url),
        enabled: true,
        options: Some(ConfigInputOptions {
            xtream_skip_live: false,
            xtream_skip_vod: false,
            xtream_skip_series: false,
            xtream_live_stream_without_extension: false,
            xtream_live_stream_use_prefix: true,
        }),
        ..Default::default()
    }
}

async fn get_playlist(client: Arc<reqwest::Client>, cfg_input: Option<&ConfigInput>, cfg: &Config) -> HttpResponse {
    match cfg_input {
        Some(input) => {
            let (result, errors) =
                match input.input_type {
                    InputType::M3u => m3u::get_m3u_playlist(client, cfg, input, &cfg.working_dir).await,
                    InputType::Xtream => xtream::get_xtream_playlist(client, input, &cfg.working_dir).await,
                };
            if result.is_empty() {
                let error_strings: Vec<String> = errors.iter().map(std::string::ToString::to_string).collect();
                HttpResponse::BadRequest().json(json!({"error": error_strings.join(", ")}))
            } else {
                HttpResponse::Ok().json(result)
            }
        }
        None => HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"})),
    }
}

async fn playlist(
    req: web::Json<PlaylistRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    if let Some(input_name) = req.input_name.as_ref() {
        get_playlist(Arc::clone(&app_state.http_client), app_state.config.get_input_by_name(input_name), &app_state.config).await
    } else {
        let url = req.url.as_deref().unwrap_or("");
        let name = req.input_name.as_deref().unwrap_or("");
        let input = create_config_input_for_url(name, url);
        get_playlist(Arc::clone(&app_state.http_client), Some(&input), &app_state.config).await
    }
}

async fn config(
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let map_input = |i: &ConfigInput| ServerInputConfig {
        id: i.id,
        name: i.name.clone(),
        input_type: i.input_type.clone(),
        url: i.url.clone(),
        username: i.username.clone(),
        password: i.password.clone(),
        persist: i.persist.clone(),
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
        working_dir: config.working_dir.clone(),
        backup_dir: config.backup_dir.clone(),
        log: config.log.clone(),
        update_on_boot: config.update_on_boot,
        web_ui_enabled: config.web_ui_enabled,
        web_auth: config.web_auth.clone(),
        schedules: config.schedules.clone(),
        reverse_proxy: config.reverse_proxy.clone(),
        messaging: config.messaging.clone(),
        video: config.video.clone(),
        sources: config.sources.iter().map(map_source).collect(),
        api_proxy: config_reader::read_api_proxy(app_state.config.t_api_proxy_file_path.as_str(), false),
    };

    let mut result = match config_reader::read_config(app_state.config.t_config_path.as_str(),
                                                      app_state.config.t_config_file_path.as_str(),
                                                      app_state.config.t_sources_file_path.as_str()) {
        Ok(mut cfg) => {
            let _ = cfg.prepare(true);
            map_config(&cfg)
        }
        Err(_) => map_config(&app_state.config)
    };

    // if we didn't read it from file then we should use it from app_state
    if result.api_proxy.is_none() {
        result.api_proxy.clone_from(&*app_state.config.t_api_proxy.read().await);
    }

    HttpResponse::Ok().json(result)
}

pub fn v1_api_register(web_auth_enabled: bool) -> impl Fn(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(web::scope("/api/v1")
            .wrap(Condition::new(web_auth_enabled, HttpAuthentication::with_fn(validator)))
            .route("/config", web::get().to(config))
            .route("/config/main", web::post().to(save_config_main))
            .route("/config/user", web::post().to(save_config_api_proxy_user))
            .route("/config/apiproxy", web::post().to(save_config_api_proxy_config))
            .route("/playlist", web::post().to(playlist))
            .route("/playlist/update", web::post().to(playlist_update))
            .route("/file/download", web::post().to(download_api::queue_download_file))
            .route("/file/download/info", web::get().to(download_api::download_file_info)));
    }
}
