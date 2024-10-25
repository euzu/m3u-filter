// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::FromStr;

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Local};
use log::{debug, error};
use serde_json::{Map, Value};

use crate::api::api_model::{AppState, UserApiRequest, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::api::api_utils::{get_user_server_info, get_user_target, get_user_target_by_credentials, serve_file, stream_response};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::config::TargetType;
use crate::model::playlist::{PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::storage::{get_target_id_mapping_file, get_target_storage_path, hash_string};
use crate::repository::target_id_mapping_record::TargetIdMapping;
use crate::repository::xtream_repository;
use crate::utils::{json_utils, request_utils};


enum XtreamApiStreamContext {
    LiveAlt,
    Live,
    Movie,
    Series,
    Timeshift,
}

impl Display for XtreamApiStreamContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            XtreamApiStreamContext::LiveAlt => "",
            XtreamApiStreamContext::Live => "live",
            XtreamApiStreamContext::Movie => "movie",
            XtreamApiStreamContext::Series => "series",
            XtreamApiStreamContext::Timeshift => "timeshift",
        })
    }
}

struct XtreamApiStreamRequest<'a> {
    context: XtreamApiStreamContext,
    username: &'a str,
    password: &'a str,
    stream_id: &'a str,
    action_path: &'a str,
}

impl<'a> XtreamApiStreamRequest<'a> {
    pub fn from(context: XtreamApiStreamContext,
                username: &'a str,
                password: &'a str,
                stream_id: &'a str,
                action_path: &'a str) -> Self {
        Self {
            context,
            username,
            password,
            stream_id,
            action_path,
        }
    }
}

pub(crate) fn serve_query(file_path: &Path, filter: &HashMap<&str, &str>) -> HttpResponse {
    let filtered = json_utils::json_filter_file(file_path, filter);
    HttpResponse::Ok().json(filtered)
}

fn get_xtream_player_api_action_url(input: &ConfigInput, action: &str) -> Option<String> {
    if let Some(user_info) = input.get_user_info() {
        Some(format!("{}/player_api.php?username={}&password={}&action={}",
                     &user_info.base_url,
                     &user_info.username,
                     &user_info.password,
                     action
        ))
    } else {
        None
    }
}

fn get_xtream_player_api_info_url(input: &ConfigInput, cluster: XtreamCluster, stream_id: u32) -> Option<String> {
    let (action, stream_id_field) = match cluster {
        XtreamCluster::Live => ("get_live_info", "live_id"),
        XtreamCluster::Video => ("get_vod_info", "vod_id"),
        XtreamCluster::Series => ("get_series_info", "series_id"),
    };
    get_xtream_player_api_action_url(input, action).map(|action_url| format!("{action_url}&{stream_id_field}={stream_id}"))
}

fn get_xtream_player_api_stream_url(input: &ConfigInput, context: &str, action_path: &str) -> Option<String> {
    let ctx_path = if context.is_empty() { String::new() } else { format!("{context}/") };
    if let Some(user_info) = input.get_user_info() {
        Some(format!("{}/{}{}/{}/{}",
                     &user_info.base_url,
                     ctx_path,
                     &user_info.username,
                     &user_info.password,
                     action_path
        ))
    } else {
        None
    }
}


fn get_user_info(user: &ProxyUserCredentials, cfg: &Config) -> XtreamAuthorizationResponse {
    let server_info = get_user_server_info(cfg, user);

    let now = Local::now();
    XtreamAuthorizationResponse {
        user_info: XtreamUserInfo {
            active_cons: "0".to_string(),
            allowed_output_formats: Vec::from(["ts".to_string(), "m3u8".to_string(), "rtmp".to_string()]),
            auth: 1,
            created_at: (now - Duration::days(365)).timestamp(), // fake
            exp_date: (now + Duration::days(365)).timestamp(), // fake
            is_trial: "0".to_string(),
            max_connections: "1".to_string(),
            message: server_info.message.to_string(),
            password: user.password.to_string(),
            username: user.username.to_string(),
            status: "Active".to_string(),
        },
        server_info: XtreamServerInfo {
            url: server_info.host.clone(),
            port: server_info.http_port.clone(),
            https_port: server_info.https_port.clone(),
            server_protocol: server_info.protocol.clone(),
            rtmp_port: server_info.rtmp_port.clone(),
            timezone: server_info.timezone.to_string(),
            timestamp_now: now.timestamp(),
            time_now: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    }
}

fn xtream_api_request_separate_number_and_rest(input: &str) -> (String, String) {
    if let Some(dot_index) = input.find('.') {
        let number_part = input[..dot_index].to_string();
        let rest = input[dot_index..].to_string();
        (number_part, rest)
    } else {
        (input.to_string(), String::new())
    }
}

async fn xtream_player_api_stream(
    req: &HttpRequest,
    api_req: &web::Query<UserApiRequest>,
    app_state: &web::Data<AppState>,
    stream_req: XtreamApiStreamRequest<'_>,
) -> HttpResponse {
    if let Some((user, target)) = get_user_target_by_credentials(stream_req.username, stream_req.password, api_req, app_state) {
        let target_name = &target.name;
        if target.has_output(&TargetType::Xtream) {
            let (action_stream_id, stream_ext) = xtream_api_request_separate_number_and_rest(stream_req.stream_id);
            let virtual_id: u32 = match FromStr::from_str(action_stream_id.trim()) {
                Ok(id) => id,
                Err(_) => return HttpResponse::BadRequest().finish()
            };

            if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None) {
                let input_id: u16 = pli.input_id;
                if let Some(input) = app_state.config.get_input_by_id(input_id) {
                    let mut query_path = if stream_req.action_path.is_empty() { String::new() } else { format!("{}/", stream_req.action_path) };
                    query_path = format!("{query_path}{}{stream_ext}", pli.provider_id);
                    if let Some(stream_url) = get_xtream_player_api_stream_url(input, stream_req.context.to_string().as_str(), query_path.as_str()) {
                        if user.proxy == ProxyType::Redirect {
                            debug!("Redirecting stream request to {stream_url}");
                            return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
                        }
                        return stream_response(&stream_url, req, Some(input)).await;
                    }
                    error!("Cant find stream url for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context);
                } else {
                    error!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context);
                }
            } else {
                error!("Failed to read xtream item for stream id {}", virtual_id);
            }
        } else {
            debug!("Target has no xtream output {}", target_name);
        }
    } else {
        debug!("Could not find any user {}", stream_req.username);
    }
    HttpResponse::BadRequest().finish()
}

async fn xtream_player_api_live_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Live, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_live_stream_alt(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::LiveAlt, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_series_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Series, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_movie_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Movie, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_timeshift_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, duration, start, stream_id) = path.into_inner();
    let action_path = format!("{duration}/{start}");
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Timeshift, &username, &password, &stream_id, &action_path)).await
}


async fn xtream_player_api_streaming_timeshift(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let username = &api_req.username;
    let password = &api_req.password;
    let stream_id = &api_req.stream;
    let duration = &api_req.duration;
    let start = &api_req.start;
    let action_path = format!("{duration}/{start}");
    xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Timeshift, username, password, stream_id, &action_path)).await
}

fn get_xtream_vod_info(target: &ConfigTarget, pli: &XtreamPlaylistItem, content: &str) -> Result<String, Error> {
    if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content) {
        if let Some(Value::Object(movie_data)) = doc.get_mut("movie_data") {
            let stream_id = pli.virtual_id;
            let category_id = pli.category_id;
            movie_data.insert("stream_id".to_string(), Value::Number(serde_json::value::Number::from(stream_id)));
            movie_data.insert("category_id".to_string(), Value::Number(serde_json::value::Number::from(category_id)));
            let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
            if options.skip_video_direct_source {
                movie_data.insert("direct_source".to_string(), Value::String(String::new()));
            } else {
                movie_data.insert("direct_source".to_string(), Value::String(pli.url.to_string()));
            }
            if let Ok(result) = serde_json::to_string(&doc) {
                return Ok(result);
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to get vod info for id {}", pli.virtual_id)))
}

fn get_and_write_xtream_series_info(config: &Config, target: &ConfigTarget, pli_series_info: &XtreamPlaylistItem, content: &str) -> Result<String, Error> {
    if let Ok(mut doc) = serde_json::from_str::<Value>(content) {
        if let Some(target_path) = get_target_storage_path(config, target.name.as_str()) {
            let mut target_id_mapping = TargetIdMapping::from_path(&get_target_id_mapping_file(&target_path));

            if let Some(episodes) = doc.get_mut("episodes") {
                if let Some(episodes_map) = episodes.as_object_mut() {
                    let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
                    for (_season, episode_list) in episodes_map {
                        // Iterate over items in the episode
                        if let Some(entries) = episode_list.as_array_mut() {
                            for entry in entries {
                                if let Some(episode) = entry.as_object_mut() {
                                    if let Some(episode_id) = episode.get("id") {
                                        if let Ok(provider_id) = episode_id.as_str().unwrap().parse::<u32>() {
                                            let uuid_str = format!("{}/{}", pli_series_info.url, provider_id);
                                            let uuid = hash_string(uuid_str.as_str());
                                            let virtual_id = target_id_mapping.insert_entry(provider_id, uuid, &PlaylistItemType::Series, pli_series_info.virtual_id);
                                            episode.insert("id".to_string(), Value::String(virtual_id.to_string()));
                                        }
                                    }
                                    if options.skip_series_direct_source {
                                        episode.insert("direct_source".to_string(), Value::String(String::new()));
                                    }
                                }
                            }
                        }
                    }
                }
                if let Ok(result) = serde_json::to_string(&doc) {
                    let _ = xtream_repository::xtream_write_series_info(config, target.name.as_str(), pli_series_info.virtual_id, &result);
                    return Ok(result);
                }
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to get series info for id {}", pli_series_info.virtual_id)))
}

async fn xtream_get_stream_info_content(info_url: &str, input: &ConfigInput) -> Result<String, Error> {
    request_utils::download_text_content(input, info_url, None).await
}

async fn xtream_get_stream_info(config: &Config, input: &ConfigInput, target: &ConfigTarget,
                                pli: &XtreamPlaylistItem, info_url: &str, cluster: XtreamCluster) -> Result<String, Error> {
    if cluster == XtreamCluster::Series {
        if let Ok(content) = xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.virtual_id) {
            return Ok(content);
        }
    }

    if let Ok(content) = xtream_get_stream_info_content(info_url, input).await {
        return match cluster {
            XtreamCluster::Live => Ok(content),
            XtreamCluster::Video => get_xtream_vod_info(target, pli, &content),
            XtreamCluster::Series => get_and_write_xtream_series_info(config, target, pli, &content),
        };
    }

    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find stream with id: {}/{}/{}",
                                                      target.name.replace(' ', "_").as_str(), &cluster, pli.virtual_id)))
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: XtreamCluster) -> HttpResponse {
    let virtual_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(cluster)) {
        let input_id = pli.input_id;
        if let Some(input) = app_state.config.get_input_by_id(input_id) {
            if let Some(info_url) = get_xtream_player_api_info_url(input, cluster, pli.provider_id) {
                // Redirect is only possible for live streams, vod and series info needs to be modified
                if user.proxy == ProxyType::Redirect && cluster == XtreamCluster::Live {
                    return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                } else if let Ok(content) = xtream_get_stream_info(&app_state.config, input, target, &pli, info_url.as_str(), cluster).await {
                    return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content);
                }
            }
        }
    }
    match cluster {
        XtreamCluster::Live => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{}"),
        XtreamCluster::Video => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{info:[]}"),
        XtreamCluster::Series => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("[]"),
    }
}

async fn xtream_get_short_epg(app_state: &AppState, user: &ProxyUserCredentials, target: &ConfigTarget, stream_id: &str, limit: &str) -> HttpResponse {
    let target_name = &target.name;
    if target.has_output(&TargetType::Xtream) {
        let req_stream_id: u32 = match FromStr::from_str(stream_id.trim()) {
            Ok(id) => id,
            Err(_) => return HttpResponse::BadRequest().finish()
        };

        if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(req_stream_id, &app_state.config, target, None) {
            let input_id: u16 = pli.input_id;
            if let Some(input) = app_state.config.get_input_by_id(input_id) {
                if let Some(action_url) = get_xtream_player_api_action_url(input, "get_short_epg") {
                    let mut info_url = format!("{}&stream_id={}", action_url, pli.provider_id);
                    if !(limit.is_empty() || limit.eq("0")) {
                        info_url = format!("{info_url}&limit={limit}");
                    }
                    if user.proxy == ProxyType::Redirect {
                        return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                    }

                    return match request_utils::download_text_content(input, info_url.as_str(), None).await {
                        Ok(content) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content),
                        Err(err) => {
                            error!("Failed to download epg {}", err.to_string());
                            HttpResponse::NoContent().finish()
                        }
                    };
                }
            }
        }
    }
    error!("Cant find short epg with id: {}/{}", target_name, stream_id);
    HttpResponse::NoContent().finish()
}

async fn xtream_player_api_handle_content_action(config: &Config, target_name: &str, action: &str, category_id: &str, req: &HttpRequest) -> Option<HttpResponse> {
    if let Ok((path, content)) = match action {
        "get_live_categories" => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_LIVE),
        "get_vod_categories" => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_VOD),
        "get_series_categories" => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_SERIES),
        _ => Err(std::io::Error::new(ErrorKind::Other, ""))
    } {
        if let Some(file_path) = path {
            let category_id = category_id.trim();
            if !category_id.is_empty() {
                return Some(serve_query(&file_path, &HashMap::from([("category_id", category_id)])));
            }
            return Some(serve_file(&file_path, req, mime::APPLICATION_JSON).await);
        } else if let Some(payload) = content {
            return Some(HttpResponse::Ok().body(payload));
        }
        return Some(HttpResponse::NoContent().finish());
    }
    None
}

async fn xtream_get_catchup_response(app_state: &AppState, target: &ConfigTarget, stream_id: &str, start: &str, end: &str) -> HttpResponse {
    let req_stream_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(req_stream_id, &app_state.config, target, Some(XtreamCluster::Live)) {
        let input_id = pli.input_id;
        if let Some(input) = app_state.config.get_input_by_id(input_id) {
            if let Some(info_url) = get_xtream_player_api_action_url(input, "get_simple_data_table")
                .map(|action_url| format!("{action_url}&stream_id={}&start={start}&end={end}", pli.provider_id)) {
                if let Ok(content) = xtream_get_stream_info_content(info_url.as_str(), input).await {
                    if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content.as_str()) {
                        if let Some(epg_listings) = doc.get_mut("epg_listings") {
                            if let Some(epg_listing_list) = epg_listings.as_array_mut() {
                                let mapping = xtream_repository::xtream_load_catchup_id_mapping(&app_state.config, target.name.as_str());
                                let mut max_id = u32::try_from(mapping.len()).unwrap();
                                let mut new_id_mappings = Vec::new();
                                for epg_list_value in epg_listing_list {
                                    if let Some(epg_list_item) = epg_list_value.as_object_mut() {
                                        // TODO epg_id
                                        if let Some(Some(provider_id)) = epg_list_item.get("id").map(|v| v.as_str()) {
                                            if let Ok(provider_stream_id) = &FromStr::from_str(provider_id) {
                                                let stream_id = match mapping.get(provider_stream_id) {
                                                    None => {
                                                        max_id += 1;
                                                        new_id_mappings.push((*provider_stream_id, max_id));
                                                        max_id
                                                    }
                                                    Some(mapped_id) => *mapped_id
                                                };
                                                epg_list_item.insert("id".to_string(), Value::String(stream_id.to_string()));
                                            }
                                        }
                                    }
                                }
                                if !new_id_mappings.is_empty() {
                                    if let Err(err) = xtream_repository::xtream_write_catchup_id_mapping(&app_state.config, target.name.as_str(), &new_id_mappings) {
                                        error!("Failed to write catchup id mapping {err}");
                                        return HttpResponse::BadRequest().finish();
                                    }
                                }
                            }
                        }

                        if let Ok(result) = serde_json::to_string(&doc) {
                            return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(result);
                        }
                    }
                }
            }
        }
    }
    HttpResponse::BadRequest().finish()
}

async fn xtream_player_api(
    req: &HttpRequest,
    api_req: UserApiRequest,
    app_state: &web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, app_state) {
        Some((user, target)) => {
            let action = api_req.action.trim();
            let target_name = &target.name;
            if target.has_output(&TargetType::Xtream) {
                if action.is_empty() {
                    return HttpResponse::Ok().json(get_user_info(&user, &app_state.config));
                }

                match action {
                    "get_series_info" => {
                        xtream_get_stream_info_response(app_state, &user, target,
                                                        api_req.series_id.trim(),
                                                        XtreamCluster::Series).await
                    }
                    "get_vod_info" => {
                        xtream_get_stream_info_response(app_state, &user, target,
                                                        api_req.vod_id.trim(),
                                                        XtreamCluster::Video).await
                    }
                    "get_epg" |
                    "get_short_epg" => {
                        xtream_get_short_epg(app_state, &user, target,
                                             api_req.stream_id.trim(),
                                             api_req.limit.trim()).await
                    }
                    "get_simple_data_table" => {
                        xtream_get_catchup_response(app_state, target,
                                                    api_req.stream_id.trim(),
                                                    api_req.start.trim(),
                                                    api_req.end.trim()).await
                    }
                    _ => {
                        let category_id = api_req.category_id.as_str().trim();
                        if let Some(response) = xtream_player_api_handle_content_action(&app_state.config, target_name, action, category_id, req).await {
                            response
                        } else {
                            let cat_id = if category_id.is_empty() { 0 } else { category_id.parse::<u32>().unwrap_or(0) };
                            match match action {
                                "get_live_streams" => xtream_repository::xtream_load_rewrite_playlist(&XtreamCluster::Live, &app_state.config, target, cat_id),
                                "get_vod_streams" => xtream_repository::xtream_load_rewrite_playlist(&XtreamCluster::Video, &app_state.config, target, cat_id),
                                "get_series" => xtream_repository::xtream_load_rewrite_playlist(&XtreamCluster::Series, &app_state.config, target, cat_id),
                                _ => Err(Error::new(ErrorKind::Unsupported, format!("Cant find action: {action} for target: {target_name}"))),
                            } {
                                Ok(payload) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(payload),
                                Err(err) => {
                                    error!("Could not create response for xtream target: {} action: {} err: {}", target_name, action, err);
                                    HttpResponse::NoContent().finish()
                                }
                            }
                        }
                    }
                }
            } else {
                HttpResponse::Ok().json(get_user_info(&user, &app_state.config))
            }
        }
        _ => {
            debug!("{}", if api_req.action.is_empty() { "Paremeter action is empty!" } else { "cant find user!" });
            HttpResponse::BadRequest().finish()
        }
    }
}


async fn xtream_player_api_get(req: HttpRequest,
                               api_req: web::Query<UserApiRequest>,
                               app_state: web::Data<AppState>,
) -> HttpResponse {
    xtream_player_api(&req, api_req.into_inner(), &app_state).await
}

async fn xtream_player_api_post(req: HttpRequest,
                                api_req: web::Form<UserApiRequest>,
                                app_state: web::Data<AppState>,
) -> HttpResponse {
    xtream_player_api(&req, api_req.into_inner(), &app_state).await
}

pub(crate) fn xtream_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/player_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)))
        .service(web::resource("/panel_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)))
        .service(web::resource("/xtream").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)))
        .service(web::resource("/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream_alt)))
        .service(web::resource("/live/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream)))
        .service(web::resource("/movie/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_movie_stream)))
        .service(web::resource("/series/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_series_stream)))
        .service(web::resource("/timeshift/{username}/{password}/{duration}/{start}/{stream_id}").route(web::get().to(xtream_player_api_timeshift_stream)))
        .service(web::resource("/timeshift.php").route(web::get().to(xtream_player_api_streaming_timeshift)))
        .service(web::resource("/streaming/timeshift.php").route(web::get().to(xtream_player_api_streaming_timeshift)));
    /* TODO
    cfg.service(web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(xtream_player_api_hlsr_stream)));
    cfg.service(web::resource("/hls/{token}/{chunk}").route(web::get().to(xtream_player_api_hls_stream)));
    cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
     */
}