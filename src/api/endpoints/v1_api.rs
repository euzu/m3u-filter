use crate::api::endpoints::download_api;
use crate::api::endpoints::user_api::user_api_register;
use crate::api::model::app_state::AppState;
use crate::api::model::config::{ServerConfig, ServerInputConfig, ServerSourceConfig, ServerTargetConfig};
use crate::api::model::request::{PlaylistRequest, PlaylistRequestType};
use crate::auth::access_token::create_access_token;
use crate::auth::authenticator::validator_admin;
use crate::m3u_filter_error::M3uFilterError;
use crate::model::api_proxy::{ApiProxyConfig, ApiProxyServerInfo, ProxyUserCredentials, TargetUser};
use crate::model::config::{validate_targets, Config, ConfigDto, ConfigInput, ConfigInputOptions, ConfigSource, ConfigTarget, InputType, TargetType};
use crate::model::playlist::{M3uPlaylistItem, PlaylistGroup, PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::processing::processor::playlist;
use crate::repository::user_repository::store_api_user;
use crate::repository::{m3u_repository, xtream_repository};
use crate::utils::file::config_reader;
use crate::utils::file::file_lock_manager::FileReadGuard;
use crate::utils::network::request::sanitize_sensitive_info;
use crate::utils::network::{m3u, xtream};
use axum::response::IntoResponse;
use log::error;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn intern_save_config_api_proxy(backup_dir: &str, api_proxy: &ApiProxyConfig, file_path: &str) -> Option<M3uFilterError> {
    match config_reader::save_api_proxy(file_path, backup_dir, api_proxy) {
        Ok(()) => {}
        Err(err) => {
            error!("Failed to save api_proxy.yml {err}");
            return Some(err);
        }
    }
    None
}

fn intern_save_config_main(file_path: &str, backup_dir: &str, cfg: &ConfigDto) -> Option<M3uFilterError> {
    match config_reader::save_main_config(file_path, backup_dir, cfg) {
        Ok(()) => {}
        Err(err) => {
            error!("Failed to save config.yml {err}");
            return Some(err);
        }
    }
    None
}

async fn save_config_api_proxy_user(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(mut users): axum::extract::Json<Vec<TargetUser>>,
) -> impl axum::response::IntoResponse + Send {
    let mut usernames = HashSet::new();
    let mut tokens = HashSet::new();
    for target_user in &mut users {
        for credential in &mut target_user.credentials {
            credential.trim();
            if let Err(err) = credential.validate() {
                return (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": err.to_string()}))).into_response();
            }
            if usernames.contains(&credential.username) {
                return (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": format!("Duplicate username {}", &credential.username)}))).into_response();
            }
            usernames.insert(&credential.username);
            if let Some(token) = &credential.token {
                if tokens.contains(token) {
                    return (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": format!("Duplicate token {token}")}))).into_response();
                }
                tokens.insert(token);
            }
        }
    }

    let mut lock = app_state.config.t_api_proxy.write().await;
    if let Some(api_proxy) = lock.as_mut() {
        api_proxy.user = users;
        api_proxy.user.iter_mut().flat_map(|t| &mut t.credentials).for_each(ProxyUserCredentials::prepare);
        if api_proxy.use_user_db {
            if let Err(err) = store_api_user(&app_state.config, &api_proxy.user) {
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({"error": err.to_string()}))).into_response();
            }
        } else {
            let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
            if let Some(err) = intern_save_config_api_proxy(backup_dir, api_proxy, app_state.config.t_api_proxy_file_path.as_str()) {
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({"error": err.to_string()}))).into_response();
            }
        }
    }
    axum::http::StatusCode::OK.into_response()
}

async fn save_config_main(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(cfg): axum::extract::Json<ConfigDto>,
) -> impl axum::response::IntoResponse + Send {
    if cfg.is_valid() {
        let file_path = app_state.config.t_config_file_path.as_str();
        let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = intern_save_config_main(file_path, backup_dir, &cfg) {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({"error": err.to_string()}))).into_response();
        }
        axum::http::StatusCode::OK.into_response()
    } else {
        (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid content"}))).into_response()
    }
}

async fn save_config_api_proxy_config(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(mut req_api_proxy): axum::extract::Json<Vec<ApiProxyServerInfo>>,
) -> impl axum::response::IntoResponse + Send {
    for server_info in &mut req_api_proxy {
        if !server_info.is_valid() {
            return (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid content"}))).into_response();
        }
    }
    let mut lock = app_state.config.t_api_proxy.write().await;
    if let Some(api_proxy) = lock.as_mut() {
        api_proxy.server = req_api_proxy;
        let backup_dir = app_state.config.backup_dir.as_ref().unwrap().as_str();
        if let Some(err) = intern_save_config_api_proxy(backup_dir, api_proxy, app_state.config.t_api_proxy_file_path.as_str()) {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({"error": err.to_string()}))).into_response();
        }
    }
    axum::http::StatusCode::OK.into_response()
}

async fn playlist_update(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(targets): axum::extract::Json<Vec<String>>,
) -> impl axum::response::IntoResponse + Send {
    let user_targets = if targets.is_empty() { None } else { Some(targets) };
    let process_targets = validate_targets(user_targets.as_ref(), &app_state.config.sources);
    match process_targets {
        Ok(valid_targets) => {
            tokio::spawn(playlist::exec_processing(Arc::clone(&app_state.http_client), Arc::clone(&app_state.config), Arc::new(valid_targets)));
            axum::http::StatusCode::OK.into_response()
        }
        Err(err) => {
            error!("Failed playlist update {}", sanitize_sensitive_info(err.to_string().as_str()));
            (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": err.to_string()}))).into_response()
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

#[derive(serde::Serialize, serde::Deserialize)]
struct PlaylistResponseGroup {
    id: u32,
    title: String,
    channels: serde_json::Value,
    xtream_cluster: XtreamCluster,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PlaylistResponse {
    live: Option<Vec<PlaylistResponseGroup>>,
    vod: Option<Vec<PlaylistResponseGroup>>,
    series: Option<Vec<PlaylistResponseGroup>>,
}

fn group_playlist_items<T>(
    cluster: XtreamCluster,
    iter: impl Iterator<Item=T>,
    get_group: fn(&T) -> String,
) -> Vec<PlaylistResponseGroup>
where
    T: Serialize,
{
    let mut groups: HashMap<String, Vec<T>> = HashMap::new();

    for item in iter {
        let group_key = get_group(&item);
        groups.entry(group_key)
            .or_default()
            .push(item);
    }

    groups
        .into_iter()
        .enumerate()
        .map(|(index, (key, value))| PlaylistResponseGroup {
            #[allow(clippy::cast_possible_truncation)]
            id: index as u32,
            title: key.to_string(),
            channels: serde_json::to_value(value).unwrap_or(Value::Null),
            xtream_cluster: cluster,
        })
        .collect()
}

fn group_playlist_items_by_cluster(params: Option<(FileReadGuard,
                                                   impl Iterator<Item=(M3uPlaylistItem, bool)>)>) ->
                                   (Vec<M3uPlaylistItem>, Vec<M3uPlaylistItem>, Vec<M3uPlaylistItem>) {
    if params.is_none() {
        return (vec![], vec![], vec![]);
    }
    let mut live = Vec::new();
    let mut video = Vec::new();
    let mut series = Vec::new();
    let (guard, iter) = params.unwrap();
    for (item, _) in iter {
        match item.item_type {
            PlaylistItemType::Live
            | PlaylistItemType::Catchup
            | PlaylistItemType::LiveUnknown
            | PlaylistItemType::LiveHls
            | PlaylistItemType::LiveDash => {
                live.push(item);
            }
            PlaylistItemType::Video => {
                video.push(item);
            }
            PlaylistItemType::Series
            | PlaylistItemType::SeriesInfo => {
                series.push(item);
            }
        }
    }

    drop(guard);

    (live, video, series)
}

fn group_playlist_groups_by_cluster(playlist: Vec<PlaylistGroup>, input_type: InputType) -> (Vec<PlaylistResponseGroup>, Vec<PlaylistResponseGroup>, Vec<PlaylistResponseGroup>) {
    let mut live = Vec::new();
    let mut video = Vec::new();
    let mut series = Vec::new();
    for group in playlist {
        let channels = group.channels.iter().map(|item| if input_type == InputType::M3u { serde_json::to_value(item.to_m3u()).unwrap() } else { serde_json::to_value(item.to_xtream()).unwrap() }).collect();
        let grp = PlaylistResponseGroup {
            id: group.id,
            title: group.title,
            channels,
            xtream_cluster: group.xtream_cluster,
        };
        match group.xtream_cluster {
            XtreamCluster::Live => live.push(grp),
            XtreamCluster::Video => video.push(grp),
            XtreamCluster::Series => series.push(grp),
        }
    }
    (live, video, series)
}

async fn get_playlist(client: Arc<reqwest::Client>, cfg_input: Option<&ConfigInput>, cfg: &Config) -> impl IntoResponse + Send {
    match cfg_input {
        Some(input) => {
            let (result, errors) =
                match input.input_type {
                    InputType::M3u => m3u::get_m3u_playlist(client, cfg, input, &cfg.working_dir).await,
                    InputType::Xtream => xtream::get_xtream_playlist(client, input, &cfg.working_dir).await,
                    InputType::M3uBatch | InputType::XtreamBatch => (vec![], vec![])
                };
            if result.is_empty() {
                let error_strings: Vec<String> = errors.iter().map(std::string::ToString::to_string).collect();
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": error_strings.join(", ")}))).into_response()
            } else {
                let (live, vod, series) = group_playlist_groups_by_cluster(result, input.input_type);
                let response = PlaylistResponse {
                    live: Some(live),
                    vod: Some(vod),
                    series: Some(series),
                };
                (axum::http::StatusCode::OK, axum::Json(response)).into_response()
            }
        }
        None => (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid Arguments"}))).into_response(),
    }
}

// async fn get_categories_content(action: Result<(Option<PathBuf>, Option<String>), std::io::Error>) -> Option<String> {
//     if let Ok((Some(file_path), _content)) = action {
//         if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
//             // TODO deserialize like sax parser
//             if let Ok(categories) = serde_json::from_str::<Vec<PlaylistXtreamCategory>>(&content) {
//                 return serde_json::to_string(&categories).ok();
//             }
//         }
//     }
//     None
// }

async fn get_playlist_for_target(cfg_target: Option<&ConfigTarget>, cfg: &Arc<Config>) -> impl axum::response::IntoResponse + Send {
    if let Some(target) = cfg_target {
        if target.has_output(&TargetType::Xtream) {
            // let live_categories = get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_LIVE)).await;
            // let vod_categories = get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_VOD)).await;
            // let series_categories = get_categories_content(xtream_repository::xtream_get_collection_path(cfg, target_name, xtream_repository::COL_CAT_SERIES)).await;

            let live_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Live).await
                .map(|(_guard, iter)| group_playlist_items(XtreamCluster::Live, iter.map(|(v, _)| v), |item| item.group.clone()));
            let vod_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Video).await
                .map(|(_guard, iter)| group_playlist_items(XtreamCluster::Video, iter.map(|(v, _)| v), |item| item.group.clone()));
            let series_channels = xtream_repository::iter_raw_xtream_playlist(cfg, target, XtreamCluster::Series).await
                .map(|(_guard, iter)| group_playlist_items(XtreamCluster::Series, iter.map(|(v, _)| v), |item| item.group.clone()));

            let response = PlaylistResponse {
                live: live_channels,
                vod: vod_channels,
                series: series_channels,
            };

            return (axum::http::StatusCode::OK, axum::Json(response)).into_response();
        } else if target.has_output(&TargetType::M3u) {
            let all_channels = m3u_repository::iter_raw_m3u_playlist(cfg, target).await;
            let (live_channels, vod_channels, series_channels) = group_playlist_items_by_cluster(all_channels);
            let response = PlaylistResponse {
                live: Some(group_playlist_items(XtreamCluster::Live, live_channels.into_iter(), |item| item.group.clone())),
                vod: Some(group_playlist_items(XtreamCluster::Video, vod_channels.into_iter(), |item| item.group.clone())),
                series: Some(group_playlist_items(XtreamCluster::Series, series_channels.into_iter(), |item| item.group.clone())),
            };

            return (axum::http::StatusCode::OK, axum::Json(response)).into_response();
        }
    }
    (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid Arguments"}))).into_response()
}

async fn playlist_content(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(playlist_req): axum::extract::Json<PlaylistRequest>,
) -> impl axum::response::IntoResponse + Send {
    match playlist_req.rtype {
        PlaylistRequestType::Input => {
            if let Some(source_id) = playlist_req.source_id {
                get_playlist(Arc::clone(&app_state.http_client), app_state.config.get_input_by_id(source_id), &app_state.config).await.into_response()
            } else {
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid input"}))).into_response()
            }
        }
        PlaylistRequestType::Target => {
            if let Some(source_id) = playlist_req.source_id {
                get_playlist_for_target(app_state.config.get_target_by_id(source_id), &app_state.config).await.into_response()
            } else {
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid target"}))).into_response()
            }
        }
        PlaylistRequestType::Xtream => {
            if let (Some(url), Some(username), Some(password)) = (playlist_req.url.as_ref(), playlist_req.username.as_ref(), playlist_req.password.as_ref()) {
                let input = create_config_input_for_xtream(username, password, url);
                get_playlist(Arc::clone(&app_state.http_client), Some(&input), &app_state.config).await.into_response()
            } else {
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid url"}))).into_response()
            }
        }
        PlaylistRequestType::M3U => {
            if let Some(url) = playlist_req.url.as_ref() {
                let input = create_config_input_for_m3u(url);
                get_playlist(Arc::clone(&app_state.http_client), Some(&input), &app_state.config).await.into_response()
            } else {
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid url"}))).into_response()
            }
        }
    }
}

async fn playlist_reverse(
    axum::extract::Path(target_id): axum::extract::Path<u32>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(playlist_item): axum::extract::Json<XtreamPlaylistItem>,
) -> impl axum::response::IntoResponse + Send {
    let access_token = create_access_token(&app_state.config.t_access_token_secret, 5);
    let server_name = app_state.config.web_ui.as_ref().and_then(|web_ui| web_ui.player_server.as_ref()).map_or("default", |server_name| server_name.as_str());
    let server_info = app_state.config.get_server_info(server_name).await;
    let base_url = server_info.get_base_url();
    format!("{base_url}/token/{access_token}/{target_id}/{}/{}", playlist_item.xtream_cluster.as_stream_type(), playlist_item.virtual_id).into_response()
}

async fn config(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    let map_input = |i: &ConfigInput| ServerInputConfig {
        id: i.id,
        name: i.name.clone(),
        input_type: i.input_type,
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
        web_ui: config.web_ui.clone(),
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
            let _ = cfg.prepare();
            map_config(&cfg)
        }
        Err(_) => map_config(&app_state.config)
    };

    // if we didn't read it from file then we should use it from app_state
    if result.api_proxy.is_none() {
        result.api_proxy.clone_from(&*app_state.config.t_api_proxy.read().await);
    }

    axum::response::Json(result).into_response()
}

pub fn v1_api_register(web_auth_enabled: bool, app_state: Arc<AppState>, web_ui_path: &str) -> axum::Router<Arc<AppState>> {
    let mut router = axum::Router::new();
    router = router.route("/config", axum::routing::get(config))
        .route("/config/main", axum::routing::post(save_config_main))
        .route("/config/user", axum::routing::post(save_config_api_proxy_user))
        .route("/config/apiproxy", axum::routing::post(save_config_api_proxy_config))
        .route("/playlist/reverse/{target_id}", axum::routing::post(playlist_reverse))
        .route("/playlist/update", axum::routing::post(playlist_update))
        .route("/playlist", axum::routing::post(playlist_content))
        .route("/file/download", axum::routing::post(download_api::queue_download_file))
        .route("/file/download/info", axum::routing::get(download_api::download_file_info));
    if web_auth_enabled {
        router = router.route_layer(axum::middleware::from_fn_with_state(Arc::clone(&app_state), validator_admin));
    }

    let mut base_router = axum::Router::new();
    if app_state.config.web_ui.as_ref().is_none_or(|c| c.user_ui_enabled) {
        base_router = base_router.merge(user_api_register(app_state));
    }
    base_router.nest(&format!("{web_ui_path}/api/v1"), router)
}
