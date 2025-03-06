use std::collections::HashSet;
use std::sync::Arc;

use actix_web::body::BodyStream;
use actix_web::middleware::Condition;
use actix_web::{web, HttpResponse};
use actix_web_httpauth::middleware::HttpAuthentication;
use bytes::Bytes;
use futures::{stream, StreamExt};
use log::error;
use serde_json::json;

use crate::api::endpoints::user_api::user_api_register;
use crate::api::endpoints::{download_api, user_api};
use crate::api::model::app_state::AppState;
use crate::api::model::config::{ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::api::model::request::{PlaylistRequest, PlaylistRequestType};
use crate::auth::authenticator::validator_admin;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, TargetUser};
use crate::model::config::{validate_targets, Config, ConfigDto, ConfigInput, ConfigInputOptions, ConfigSource, ConfigTarget, InputType, TargetType};
use crate::model::playlist::{XtreamCluster};
use crate::processing::processor::playlist;
use crate::repository::user_repository::store_api_user;
use crate::repository::xtream_repository;
use crate::repository::xtream_repository::playlist_iter_to_stream;
use crate::utils::file::config_reader;
use crate::utils::network::request::sanitize_sensitive_info;
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
    app_state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let mut users = req.0;
    let mut usernames = HashSet::new();
    let mut tokens = HashSet::new();
    for target_user in &mut users {
        for credential in &mut target_user.credentials {
            credential.trim();
            if let Err(err) = credential.validate() {
                return HttpResponse::BadRequest().json(json!({"error": err.to_string()}));
            }
            if usernames.contains(&credential.username) {
                return HttpResponse::BadRequest().json(json!({"error": format!("Duplicate username {}", &credential.username)}));
            }
            usernames.insert(&credential.username);
            if let Some(token) = &credential.token {
                if tokens.contains(token) {
                    return HttpResponse::BadRequest().json(json!({"error": format!("Duplicate token {token}")}));
                }
                tokens.insert(token);
            }
        }
    }

    if let Some(api_proxy) = app_state.config.t_api_proxy.write().as_mut() {
        api_proxy.user = users;
        api_proxy.user.iter_mut().flat_map(|t| &mut t.credentials).for_each(|c| c.prepare(true));
        if api_proxy.use_user_db {
            if let Err(err) = store_api_user(&app_state.config, &api_proxy.user) {
                return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
            }
        } else {
            let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
            if let Some(err) = intern_save_config_api_proxy(backup_dir, api_proxy, app_state.config.t_api_proxy_file_path.as_str()) {
                return HttpResponse::InternalServerError().json(json!({"error": err.to_string()}));
            }
        }
    }
    HttpResponse::Ok().finish()
}

async fn save_config_main(
    req: web::Json<ConfigDto>,
    app_state: web::Data<Arc<AppState>>,
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
    app_state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let mut req_api_proxy = req.0;
    for server_info in &mut req_api_proxy {
        if !server_info.is_valid() {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid content"}));
        }
    }
    if let Some(api_proxy) = app_state.config.t_api_proxy.write().as_mut() {
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
    app_state: web::Data<Arc<AppState>>,
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

fn create_config_input_for_m3u(url: &str) -> ConfigInput {
    ConfigInput {
        id: 0,
        name: String::from("m3u_req"),
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

fn create_config_input_for_xtream(username: &str, password: &str, host: &str) -> ConfigInput {
    ConfigInput {
        id: 0,
        name: String::from("xc_req"),
        input_type: InputType::Xtream,
        url: String::from(host),
        username: Some(String::from(username)),
        password: Some(String::from(password)),
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



async fn get_playlist_for_target(cfg_target: Option<&ConfigTarget>, cfg: &Arc<Config>) -> HttpResponse {
    if let Some(target) = cfg_target {
        let target_name = &target.name;
        if target.has_output(&TargetType::Xtream) {
            let live_categories = user_api::get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_LIVE)).await;
            let vod_categories = user_api::get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_VOD)).await;
            let series_categories = user_api::get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_SERIES)).await;

            let live_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Live);
            let vod_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Video);
            let series_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Series);

            let live_stream = playlist_iter_to_stream(live_channels);
            let vod_stream = playlist_iter_to_stream(vod_channels);
            let series_stream = playlist_iter_to_stream(series_channels);

            let json_stream =
                stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r#"{"categories": {"live": "#.to_string())),
                    Ok::<Bytes, String>(Bytes::from(live_categories.unwrap_or("null".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#", "vod": "#.to_string())),
                    Ok::<Bytes, String>(Bytes::from(vod_categories.unwrap_or("null".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#", "series": "#.to_string())),
                    Ok::<Bytes, String>(Bytes::from(series_categories.unwrap_or("null".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#"},"channels": {"live": ["#.to_string())),
                ]).chain(live_stream).chain(stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r#"], "vod": ["#.to_string())),
                ])).chain(vod_stream).chain(stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r#"], "series": ["#.to_string())),
                ])).chain(series_stream).chain(stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r"]}}".to_string())),
                ]));
            return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(BodyStream::new(json_stream));
        } else if target.has_output(&TargetType::M3u) {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"}));
        }
    }
    HttpResponse::BadRequest().json(json!({"error": "Invalid Arguments"}))
}

async fn playlist(
    req: web::Json<PlaylistRequest>,
    app_state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    match req.rtype {
        PlaylistRequestType::Input => {
            if let Some(source_id) = req.source_id {
                get_playlist(Arc::clone(&app_state.http_client), app_state.config.get_input_by_id(source_id), &app_state.config).await
            } else {
                HttpResponse::BadRequest().json(json!({"error": "Invalid input"}))
            }
        }
        PlaylistRequestType::Target => {
            if let Some(source_id) = req.source_id {
                get_playlist_for_target(app_state.config.get_target_by_id(source_id), &app_state.config).await
            } else {
                HttpResponse::BadRequest().json(json!({"error": "Invalid target"}))
            }
        }
        PlaylistRequestType::Xtream => {
            if let (Some(url), Some(username), Some(password)) = (req.url.as_ref(), req.username.as_ref(), req.password.as_ref()) {
                let input = create_config_input_for_xtream(username, password, url);
                get_playlist(Arc::clone(&app_state.http_client), Some(&input), &app_state.config).await
            } else {
                HttpResponse::BadRequest().json(json!({"error": "Invalid url"}))
            }
        }
        PlaylistRequestType::M3U => {
            if let Some(url) = req.url.as_ref() {
                let input = create_config_input_for_m3u(url);
                get_playlist(Arc::clone(&app_state.http_client), Some(&input), &app_state.config).await
            } else {
                HttpResponse::BadRequest().json(json!({"error": "Invalid url"}))
            }
        }
    }
}

async fn config(
    app_state: web::Data<Arc<AppState>>,
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
        user_config_dir: config.user_config_dir.clone(),
        log: config.log.clone(),
        update_on_boot: config.update_on_boot,
        web_ui_enabled: config.web_ui_enabled,
        web_auth: config.web_auth.clone(),
        schedules: config.schedules.clone(),
        reverse_proxy: config.reverse_proxy.clone(),
        messaging: config.messaging.clone(),
        video: config.video.clone(),
        sources: config.sources.iter().map(map_source).collect(),
        api_proxy: config_reader::read_api_proxy(config, app_state.config.t_api_proxy_file_path.as_str(), false),
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
        result.api_proxy.clone_from(&*app_state.config.t_api_proxy.read());
    }

    HttpResponse::Ok().json(result)
}

pub fn v1_api_register(web_auth_enabled: bool) -> impl Fn(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        user_api_register(cfg);
        cfg.service(web::scope("/api/v1")
            .wrap(Condition::new(web_auth_enabled, HttpAuthentication::with_fn(validator_admin)))
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
