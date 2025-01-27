// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use crate::Arc;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;

use actix_web::{web, HttpRequest, HttpResponse};
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use futures::Stream;
use log::{debug, error, warn};
use serde_json::{Map, Value};

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, resource_response, serve_file, stream_response};
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::xtream::XtreamAuthorizationResponse;
use crate::m3u_filter_error::{str_to_io_error, M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::TargetType;
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{get_backdrop_path_value, FieldGetAccessor, PlaylistEntry, PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::{INFO_RESOURCE_PREFIX, INFO_RESOURCE_PREFIX_EPISODE, PROP_BACKDROP_PATH, SEASON_RESOURCE_PREFIX};
use crate::repository::storage::{get_target_storage_path, hash_string};
use crate::repository::target_id_mapping::TargetIdMapping;
use crate::repository::xtream_repository;
use crate::repository::xtream_repository::{TAG_EPISODES, TAG_INFO_DATA, TAG_SEASONS_DATA};
use crate::utils::json_utils::get_u32_from_serde_value;
use crate::utils::request_utils::{extract_extension_from_url, sanitize_sensitive_info};
use crate::utils::{download, json_utils, request_utils};
use crate::{debug_if_enabled, info_err};

const ACTION_GET_SERIES_INFO: &str = "get_series_info";
const ACTION_GET_VOD_INFO: &str = "get_vod_info";
// const ACTION_GET_LIVE_INFO: &str = "get_live_info";

const ACTION_GET_EPG: &str = "get_epg";
const ACTION_GET_SHORT_EPG: &str = "get_short_epg";
const ACTION_GET_CATCHUP_TABLE: &str = "get_simple_data_table";
const ACTION_GET_LIVE_CATEGORIES: &str = "get_live_categories";
const ACTION_GET_VOD_CATEGORIES: &str = "get_vod_categories";
const ACTION_GET_SERIES_CATEGORIES: &str = "get_series_categories";
const ACTION_GET_LIVE_STREAMS: &str = "get_live_streams";
const ACTION_GET_VOD_STREAMS: &str = "get_vod_streams";
const ACTION_GET_SERIES: &str = "get_series";

const TAG_ID: &str = "id";
const TAG_CATEGORY_ID: &str = "category_id";
const TAG_STREAM_ID: &str = "stream_id";
const TAG_EPG_LISTINGS: &str = "epg_listings";

macro_rules! try_option_bad_request {
    ($option:expr, $msg_is_error:expr, $msg:expr) => {
        match $option {
            Some(value) => value,
            None => {
                if $msg_is_error {error!("{}", $msg);} else {debug!("{}", $msg);}
                return HttpResponse::BadRequest().finish();
            }
        }
    };
    ($option:expr) => {
        match $option {
            Some(value) => value,
            None => return HttpResponse::BadRequest().finish(),
        }
    };
}
macro_rules! try_result_bad_request {
    ($option:expr, $msg_is_error:expr, $msg:expr) => {
        match $option {
            Ok(value) => value,
            Err(_) => {
                if $msg_is_error {error!("{}", $msg);} else {debug!("{}", $msg);}
                return HttpResponse::BadRequest().finish();
            }
        }
    };
    ($option:expr) => {
        match $option {
            Ok(value) => value,
            Err(_) => return HttpResponse::BadRequest().finish(),
        }
    };
}

enum XtreamApiStreamContext {
    LiveAlt,
    Live,
    Movie,
    Series,
    Timeshift,
}

impl XtreamApiStreamContext {
    const LIVE_ALT: &'static str = "";
    const LIVE: &'static str = "live";
    const MOVIE: &'static str = "movie";
    const SERIES: &'static str = "series";
    const TIMESHIFT: &'static str = "timeshift";
}

impl Display for XtreamApiStreamContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::LiveAlt => Self::LIVE_ALT,
            Self::Live => Self::LIVE,
            Self::Movie => Self::MOVIE,
            Self::Series => Self::SERIES,
            Self::Timeshift => Self::TIMESHIFT,
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
    pub const fn from(context: XtreamApiStreamContext,
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

pub fn serve_query(file_path: &Path, filter: &HashMap<&str, &str>) -> HttpResponse {
    let filtered = json_utils::json_filter_file(file_path, filter);
    HttpResponse::Ok().json(filtered)
}

fn get_xtream_player_api_stream_url(input: &ConfigInput, context: &str, action_path: &str, fallback_url: &str) -> Option<String> {
    let ctx_path = if context.is_empty() { String::new() } else { format!("{context}/") };
    if let Some(user_info) = input.get_user_info() {
        Some(format!("{}/{}{}/{}/{}",
                     &user_info.base_url,
                     ctx_path,
                     &user_info.username,
                     &user_info.password,
                     action_path
        ))
    } else if !fallback_url.is_empty() {
        Some(String::from(fallback_url))
    } else {
        None
    }
}

fn get_user_info(user: &ProxyUserCredentials, cfg: &Config) -> XtreamAuthorizationResponse {
    let server_info = cfg.get_user_server_info(user);
    XtreamAuthorizationResponse::new(&server_info, user)
}

fn xtream_api_request_separate_number_and_remainder(input: &str) -> (String, Option<String>) {
    input.rfind('.').map_or_else(|| (input.to_string(), None), |dot_index| {
        let number_part = input[..dot_index].to_string();
        let rest = input[dot_index..].to_string();
        (number_part, if rest.len() < 2 { None } else { Some(rest) })
    })
}

async fn xtream_player_api_stream(
    req: &HttpRequest,
    api_req: &web::Query<UserApiRequest>,
    app_state: &web::Data<AppState>,
    stream_req: XtreamApiStreamRequest<'_>,
) -> HttpResponse {
    let (user, target) = try_option_bad_request!(get_user_target_by_credentials(stream_req.username, stream_req.password, api_req, app_state), false, format!("Could not find any user {}", stream_req.username));
    let target_name = &target.name;
    if !target.has_output(&TargetType::Xtream) {
        debug!("Target has no xtream output {}", target_name);
        return HttpResponse::BadRequest().finish();
    }
    let (action_stream_id, stream_ext) = xtream_api_request_separate_number_and_remainder(stream_req.stream_id);
    let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
    let (pli, mapping) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None).await, true, format!("Failed to read xtream item for stream id {}", virtual_id));
    let input = try_option_bad_request!(app_state.config.get_input_by_name(pli.input_name.as_str()), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context));

    if pli.item_type == PlaylistItemType::LiveHls {
        debug_if_enabled!("Redirecting stream request to {}", sanitize_sensitive_info(&pli.url));
        return HttpResponse::Found().insert_header(("Location", pli.url.to_string())).finish();
    }

    if user.proxy == ProxyType::Redirect {
        if pli.xtream_cluster == XtreamCluster::Series {
            let ext = stream_ext.unwrap_or_else(String::new);
            let url = input.url.as_str();
            let username = input.username.as_ref().map_or("", |v| v);
            let password = input.password.as_ref().map_or("", |v| v);
            let stream_url = format!("{url}/series/{username}/{password}/{}{ext}", mapping.provider_id);
            return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
        }
        debug_if_enabled!("Redirecting stream request to {}", sanitize_sensitive_info(&pli.url));
        HttpResponse::Found().insert_header(("Location", pli.url.as_str())).finish();
    }

    let extension = stream_ext.unwrap_or_else(
        || extract_extension_from_url(&pli.url).map_or_else(String::new, std::string::ToString::to_string));

    let query_path = if stream_req.action_path.is_empty() {
        format!("{}{extension}", pli.provider_id)
    } else {
        format!("{}/{}{extension}", stream_req.action_path, pli.provider_id)
    };

    let stream_url = try_option_bad_request!(get_xtream_player_api_stream_url(input,
        stream_req.context.to_string().as_str(), &query_path, pli.url.as_str()),
        true, format!("Cant find stream url for target {target_name}, context {}, stream_id {virtual_id}",
        stream_req.context));
    debug_if_enabled!("Streaming stream request from {}", sanitize_sensitive_info(&stream_url));
    stream_response(app_state, &stream_url, req, Some(input), pli.item_type, target).await
}

fn get_doc_id_and_field_name(input: &str) -> Option<(u32, &str)> {
    if let Some(pos) = input.find('_') {
        let (number_part, rest) = input.split_at(pos);
        let field = &rest[1..]; // cut _
        if let Ok(number) = number_part.parse::<u32>() {
            return Some((number, field));
        }
    }
    None
}

fn get_doc_resource_field_value(field: &str, doc: Option<&Value>) -> Option<Rc<String>> {
    if let Some(Value::Object(info_data)) = doc {
        if field.starts_with(PROP_BACKDROP_PATH) {
            return get_backdrop_path_value(field, info_data.get(PROP_BACKDROP_PATH));
        } else if let Some(Value::String(url)) = info_data.get(field) {
            return Some(Rc::new(url.to_string()));
        }
    }
    None
}

async fn xtream_get_info_resource_url(config: &Config, pli: &XtreamPlaylistItem, target: &ConfigTarget, resource: &str) -> Result<Option<Rc<String>>, serde_json::Error> {
    let info_content = match pli.xtream_cluster {
        XtreamCluster::Video => {
            xtream_repository::xtream_load_vod_info(config, target.name.as_str(), pli.get_virtual_id()).await
        }
        XtreamCluster::Series => {
            xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.get_virtual_id()).await
        }
        XtreamCluster::Live => None,
    };
    if let Some(content) = info_content {
        let doc: Map<String, Value> = serde_json::from_str(&content)?;
        let (field, possible_episode_id) = if let Some(field_name_with_episode_id) = resource.strip_prefix(INFO_RESOURCE_PREFIX_EPISODE) {
            if let Some((episode_id, field_name)) = get_doc_id_and_field_name(field_name_with_episode_id) {
                (field_name, Some(episode_id))
            } else {
                return Ok(None);
            }
        } else {
            (&resource[INFO_RESOURCE_PREFIX.len()..], None)
        };
        let info_doc = match pli.xtream_cluster {
            XtreamCluster::Video | XtreamCluster::Series => {
                if let Some(episode_id) = possible_episode_id {
                    get_episode_info_doc(&doc, episode_id)
                } else {
                    doc.get(TAG_INFO_DATA)
                }
            }
            XtreamCluster::Live => None,
        };

        if let Some(value) = get_doc_resource_field_value(field, info_doc) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn get_episode_info_doc(doc: &Map<String, Value>, episode_id: u32) -> Option<&Value> {
    let episodes = doc.get(TAG_EPISODES)?.as_object()?;
    for season_episodes in episodes.values() {
        if let Value::Array(episode_list) = season_episodes {
            for episode in episode_list {
                if let Value::Object(episode_doc) = episode {
                    if let Some(episode_id_value) = episode_doc.get(TAG_ID) {
                        if let Some(doc_episode_id) = get_u32_from_serde_value(episode_id_value) {
                            if doc_episode_id == episode_id {
                                return episode_doc.get(TAG_INFO_DATA);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn get_season_info_doc(doc: &Vec<Value>, season_id: u32) -> Option<&Value> {
    for season in doc {
        if let Value::Object(season_doc) = season {
            if let Some(season_id_value) = season_doc.get(TAG_ID) {
                if let Some(doc_season_id) = get_u32_from_serde_value(season_id_value) {
                    if doc_season_id == season_id {
                        return Some(season);
                    }
                }
            }
        }
    }
    None
}


async fn xtream_get_season_resource_url(config: &Config, pli: &XtreamPlaylistItem, target: &ConfigTarget, resource: &str) -> Result<Option<Rc<String>>, serde_json::Error> {
    let info_content = match pli.xtream_cluster {
        XtreamCluster::Series => {
            xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.get_virtual_id()).await
        }
        XtreamCluster::Video | XtreamCluster::Live => None,
    };
    if let Some(content) = info_content {
        let doc: Map<String, Value> = serde_json::from_str(&content)?;

        if let Some(field_name_with_season_id) = resource.strip_prefix(SEASON_RESOURCE_PREFIX) {
            if let Some((season_id, field)) = get_doc_id_and_field_name(field_name_with_season_id) {
                let seasons_doc = match pli.xtream_cluster {
                    XtreamCluster::Series => doc.get(TAG_SEASONS_DATA),
                    XtreamCluster::Video | XtreamCluster::Live => None,
                };

                if let Some(Value::Array(seasons)) = seasons_doc {
                    if let Some(value) = get_doc_resource_field_value(field, get_season_info_doc(seasons, season_id)) {
                        return Ok(Some(value));
                    }
                }
            }
        }
    }
    Ok(None)
}

async fn xtream_player_api_resource(
    req: &HttpRequest,
    api_req: &web::Query<UserApiRequest>,
    app_state: &web::Data<AppState>,
    resource_req: XtreamApiStreamRequest<'_>,
) -> HttpResponse {
    let (user, target) = try_option_bad_request!(get_user_target_by_credentials(resource_req.username, resource_req.password, api_req, app_state), false, format!("Could not find any user {}", resource_req.username));
    let target_name = &target.name;
    if !target.has_output(&TargetType::Xtream) {
        debug!("Target has no xtream output {}", target_name);
        return HttpResponse::BadRequest().finish();
    }
    let virtual_id: u32 = try_result_bad_request!(resource_req.stream_id.trim().parse());
    let resource = resource_req.action_path.trim();
    let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None).await, true, format!("Failed to read xtream item for stream id {}", virtual_id));
    let stream_url = if resource.starts_with(INFO_RESOURCE_PREFIX) {
        try_result_bad_request!(xtream_get_info_resource_url(&app_state.config, &pli, target, resource).await)
    } else if resource.starts_with(SEASON_RESOURCE_PREFIX) {
        try_result_bad_request!(xtream_get_season_resource_url(&app_state.config, &pli, target, resource).await)
    } else {
        pli.get_field(resource)
    };

    match stream_url {
        None => HttpResponse::NotFound().finish(),
        Some(url) => {
            if user.proxy == ProxyType::Redirect {
                debug!("Redirecting resource request to {}", sanitize_sensitive_info(&url));
                HttpResponse::Found().insert_header(("Location", url.as_str())).finish()
            } else {
                debug_if_enabled!("Resource request to {}", sanitize_sensitive_info(&url));
                resource_response(app_state, url.as_str(), req, None).await
            }
        }
    }
}

macro_rules! create_xtream_player_api_stream {
    ($fn_name:ident, $context:expr) => {
        async fn $fn_name(
            req: HttpRequest,
            api_req: web::Query<UserApiRequest>,
            path: web::Path<(String, String, String)>,
            app_state: web::Data<AppState>,
        ) -> HttpResponse {
            let (username, password, stream_id) = path.into_inner();
            xtream_player_api_stream(&req, &api_req, &app_state, XtreamApiStreamRequest::from($context, &username, &password, &stream_id, "")).await
        }
    }
}

macro_rules! create_xtream_player_api_resource {
    ($fn_name:ident, $context:expr) => {
        async fn $fn_name(
            req: HttpRequest,
            api_req: web::Query<UserApiRequest>,
            path: web::Path<(String, String, String, String)>,
            app_state: web::Data<AppState>,
        ) -> HttpResponse {
            let (username, password, stream_id, resource) = path.into_inner();
            xtream_player_api_resource(&req, &api_req, &app_state, XtreamApiStreamRequest::from($context, &username, &password, &stream_id, &resource)).await
        }
    }
}

create_xtream_player_api_stream!(xtream_player_api_live_stream, XtreamApiStreamContext::Live);
create_xtream_player_api_stream!(xtream_player_api_live_stream_alt, XtreamApiStreamContext::LiveAlt);
create_xtream_player_api_stream!(xtream_player_api_series_stream, XtreamApiStreamContext::Series);
create_xtream_player_api_stream!(xtream_player_api_movie_stream, XtreamApiStreamContext::Movie);

create_xtream_player_api_resource!(xtream_player_api_live_resource, XtreamApiStreamContext::Live);
create_xtream_player_api_resource!(xtream_player_api_series_resource, XtreamApiStreamContext::Series);
create_xtream_player_api_resource!(xtream_player_api_movie_resource, XtreamApiStreamContext::Movie);

fn get_non_empty<'a>(first: &'a str, second: &'a str, third: &'a str) -> &'a str {
    if !first.is_empty() {
        first
    } else if !second.is_empty() {
        second
    } else {
        third
    }
}

async fn xtream_player_api_timeshift_stream(
    req: HttpRequest,
    api_query_req: web::Query<UserApiRequest>,
    api_form_req: web::Form<UserApiRequest>,
    path: web::Path<(String, String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (path_username, path_password, path_duration, path_start, path_stream_id) = path.into_inner();
    let username = get_non_empty(&path_username, &api_query_req.username, &api_form_req.username);
    let password = get_non_empty(&path_password, &api_query_req.password, &api_form_req.password);
    let stream_id = get_non_empty(&path_stream_id, &api_query_req.stream, &api_form_req.stream);
    let duration = get_non_empty(&path_duration, &api_query_req.duration, &api_form_req.duration);
    let start = get_non_empty(&path_start, &api_query_req.start, &api_form_req.start);
    let action_path = format!("{duration}/{start}");
    xtream_player_api_stream(&req, &api_query_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Timeshift, username, password, stream_id, &action_path)).await
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: XtreamCluster) -> HttpResponse {
    let virtual_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if let Ok((pli, _)) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(cluster)).await {
        let input_name = Rc::clone(&pli.input_name);
        if let Some(input) = app_state.config.get_input_by_name(input_name.as_str()) {
            if let Some(info_url) = download::get_xtream_player_api_info_url(input, cluster, pli.provider_id) {
                // Redirect is only possible for live streams, vod and series info needs to be modified
                if user.proxy == ProxyType::Redirect && cluster == XtreamCluster::Live {
                    return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                } else if let Ok(content) = download::get_xtream_stream_info(Arc::clone(&app_state.http_client), &app_state.config, user, input, target, &pli, info_url.as_str(), cluster).await {
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
        let virtual_id: u32 = match FromStr::from_str(stream_id.trim()) {
            Ok(id) => id,
            Err(_) => return HttpResponse::BadRequest().finish()
        };

        if let Ok((pli, _)) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None).await {
            let input_name = Rc::clone(&pli.input_name);
            if let Some(input) = app_state.config.get_input_by_name(input_name.as_str()) {
                if let Some(action_url) = download::get_xtream_player_api_action_url(input, ACTION_GET_SHORT_EPG) {
                    let mut info_url = format!("{action_url}&{TAG_STREAM_ID}={}", pli.provider_id);
                    if !(limit.is_empty() || limit.eq("0")) {
                        info_url = format!("{info_url}&limit={limit}");
                    }
                    if user.proxy == ProxyType::Redirect {
                        return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                    }

                    return match request_utils::download_text_content(Arc::clone(&app_state.http_client), input, info_url.as_str(), None).await {
                        Ok(content) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content),
                        Err(err) => {
                            error!("Failed to download epg {}", sanitize_sensitive_info(err.to_string().as_str()));
                            HttpResponse::NoContent().finish()
                        }
                    };
                }
            }
        }
    }
    warn!("Cant find short epg with id: {target_name}/{stream_id}");
    HttpResponse::NoContent().finish()
}

async fn xtream_player_api_handle_content_action(config: &Config, target_name: &str, action: &str, category_id: &str, req: &HttpRequest) -> Option<HttpResponse> {
    if let Ok((path, content)) = match action {
        ACTION_GET_LIVE_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_LIVE),
        ACTION_GET_VOD_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_VOD),
        ACTION_GET_SERIES_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_SERIES),
        _ => Err(str_to_io_error(""))
    } {
        if let Some(file_path) = path {
            let category_id = category_id.trim();
            if !category_id.is_empty() {
                return Some(serve_query(&file_path, &HashMap::from([(TAG_CATEGORY_ID, category_id)])));
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
    let virtual_id: u32 = try_result_bad_request!(FromStr::from_str(stream_id));
    let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(XtreamCluster::Live)).await);
    let input = try_option_bad_request!(app_state.config.get_input_by_name(pli.input_name.as_str()));
    let info_url = try_option_bad_request!(download::get_xtream_player_api_action_url(input, ACTION_GET_CATCHUP_TABLE).map(|action_url| format!("{action_url}&{TAG_STREAM_ID}={}&start={start}&end={end}", pli.provider_id)));
    let content = try_result_bad_request!(download::get_xtream_stream_info_content(Arc::clone(&app_state.http_client), info_url.as_str(), input).await);
    let mut doc: Map<String, Value> = try_result_bad_request!(serde_json::from_str(&content));
    let epg_listings = try_option_bad_request!(doc.get_mut(TAG_EPG_LISTINGS).and_then(Value::as_array_mut));
    let target_path = try_option_bad_request!(get_target_storage_path(&app_state.config, target.name.as_str()));
    let mut target_id_mapping = TargetIdMapping::new(&target_path);

    for epg_list_item in epg_listings.iter_mut().filter_map(Value::as_object_mut) {
        // TODO epg_id
        if let Some(catchup_provider_id) = epg_list_item.get(TAG_ID).and_then(Value::as_str).and_then(|id| id.parse::<u32>().ok()) {
            let uuid = hash_string(&format!("{}/{}", pli.url, catchup_provider_id));
            let virtual_id = target_id_mapping.insert_entry(uuid, catchup_provider_id, PlaylistItemType::Catchup, pli.provider_id);
            epg_list_item.insert(TAG_ID.to_string(), Value::String(virtual_id.to_string()));
        }
    }
    if let Err(err) = target_id_mapping.persist() {
        error!("Failed to write catchup id mapping {err}");
        return HttpResponse::BadRequest().finish();
    }

    serde_json::to_string(&doc).map_or_else(|_| HttpResponse::BadRequest().finish(), |result| HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(result))
}

macro_rules! skip_response_if_flag_set {
    ($flag:expr, $stmt:expr) => {
        if $flag {
            return HttpResponse::NoContent().finish();
        }
        return $stmt;
    };
}

macro_rules! skip_flag_optional {
    ($flag:expr, $stmt:expr) => {
        if $flag {
            None
        } else {
            Some($stmt)
        }
    };
}

async fn xtream_player_api(
    req: &HttpRequest,
    api_req: UserApiRequest,
    app_state: &web::Data<AppState>,
) -> HttpResponse {
    let user_target = get_user_target(&api_req, app_state);
    if let Some((user, target)) = user_target {
        if !target.has_output(&TargetType::Xtream) {
            return HttpResponse::Ok().json(get_user_info(&user, &app_state.config));
        }

        let action = api_req.action.trim();
        if action.is_empty() {
            return HttpResponse::Ok().json(get_user_info(&user, &app_state.config));
        }

        // Process specific playlist actions
        let (skip_live, skip_vod, skip_series) = if let Some(inputs) = app_state.config.get_inputs_for_target(&target.name) {
            inputs.iter().fold((true, true, true), |acc, i| {
                let (l, v, s) = acc;
                i.options.as_ref().map_or((false, false, false), |o| (l && o.xtream_skip_live, v && o.xtream_skip_vod, s && o.xtream_skip_series))
            })
        } else {
            (false, false, false)
        };

        match action {
            ACTION_GET_SERIES_INFO => {
                skip_response_if_flag_set!(skip_series, xtream_get_stream_info_response(app_state, &user, target, api_req.series_id.trim(), XtreamCluster::Series).await);
            }
            ACTION_GET_VOD_INFO => {
                skip_response_if_flag_set!(skip_vod,  xtream_get_stream_info_response(app_state, &user, target, api_req.vod_id.trim(), XtreamCluster::Video).await);
            }
            ACTION_GET_EPG | ACTION_GET_SHORT_EPG => {
                return xtream_get_short_epg(
                    app_state, &user, target, api_req.stream_id.trim(), api_req.limit.trim(),
                ).await;
            }
            ACTION_GET_CATCHUP_TABLE => {
                skip_response_if_flag_set!(skip_live, xtream_get_catchup_response(app_state, target, api_req.stream_id.trim(), api_req.start.trim(), api_req.end.trim()).await);
            }
            _ => {}
        }

        // Handle general content actions
        if let Some(response) = xtream_player_api_handle_content_action(
            &app_state.config, &target.name, action, api_req.category_id.trim(), req,
        ).await {
            return response;
        }

        let category_id = api_req.category_id.trim().parse::<u32>().unwrap_or(0);
        let result = match action {
            ACTION_GET_LIVE_STREAMS =>
                skip_flag_optional!(skip_live, xtream_repository::xtream_load_rewrite_playlist(XtreamCluster::Live, &app_state.config, target, category_id, &user).await),
            ACTION_GET_VOD_STREAMS =>
                skip_flag_optional!(skip_vod, xtream_repository::xtream_load_rewrite_playlist(XtreamCluster::Video, &app_state.config, target, category_id, &user).await),
            ACTION_GET_SERIES =>
                skip_flag_optional!(skip_series, xtream_repository::xtream_load_rewrite_playlist(XtreamCluster::Series, &app_state.config, target, category_id, &user).await),
            _ => Some(Err(info_err!(format!("Cant find action: {action} for target: {}", &target.name))
            )),
        };

        match result {
            Some(result_iter) => {
                match result_iter {
                    Ok(xtream_iter) => {
                        // Convert the iterator into a stream of `Bytes`
                        let content_stream = xtream_create_content_stream(xtream_iter);
                        HttpResponse::Ok()
                            .content_type(mime::APPLICATION_JSON)
                            .streaming(content_stream)
                    }
                    Err(err) => {
                        error!("Failed response for xtream target: {} action: {} error: {}", &target.name, action, err);
                        HttpResponse::NoContent().finish()
                    }
                }
            }
            None => {
                HttpResponse::NoContent().finish()
            }
        }
    } else {
        if user_target.is_none() {
            debug!("Cant find user!");
        } else if api_req.action.is_empty() {
            debug!("Paremeter action is empty!");
        } else {
            debug!("Bad request!" );
        }
        HttpResponse::BadRequest().finish()
    }
}

fn xtream_create_content_stream(xtream_iter: impl Iterator<Item=String>) -> impl Stream<Item=Result<Bytes, String>> {
    let mut first_item = true;
    stream::once(async { Ok::<Bytes, String>(Bytes::from("[")) }).chain(
        stream::iter(xtream_iter.map(move |line| {
            let line = if first_item {
                first_item = false;
                line
            } else {
                format!(",{line}")
            };
            Ok::<Bytes, String>(Bytes::from(line))
        })).chain(stream::once(async { Ok::<Bytes, String>(Bytes::from("]")) })))
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

macro_rules! register_xtream_api {
    ($cfg:expr, [$($path:expr),*]) => {{
       $(
            $cfg.service(web::resource($path).route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)));
        )*
    }};
}

macro_rules! register_xtream_api_stream {
     ($cfg:expr, [$(($path:expr, $fn_name:ident)),*]) => {{
       $(
            $cfg.service(web::resource(format!("{}/{{username}}/{{password}}/{{stream_id}}", $path)).route(web::get().to($fn_name)));
        )*
    }};
}

macro_rules! register_xtream_api_resource {
     ($cfg:expr, [$(($path:expr, $fn_name:ident)),*]) => {{
       $(
            $cfg.service(web::resource(format!("/resource/{}/{{username}}/{{password}}/{{stream_id}}/{{resource}}", $path)).route(web::get().to($fn_name)));
        )*
    }};
}

macro_rules! register_xtream_api_timeshift {
     ($cfg:expr, [$($path:expr),*]) => {{
       $(
            $cfg.service(web::resource($path).route(web::get().to(xtream_player_api_timeshift_stream)).route(web::post().to(xtream_player_api_timeshift_stream)));
        )*
    }};
}

pub fn xtream_api_register(cfg: &mut web::ServiceConfig) {
    register_xtream_api!(cfg, ["/player_api.php", "/panel_api.php", "/xtream"]);
    register_xtream_api_stream!(cfg, [
        ("", xtream_player_api_live_stream_alt),
        ("/live", xtream_player_api_live_stream),
        ("/movie", xtream_player_api_movie_stream),
        ("/series", xtream_player_api_series_stream)]);
    register_xtream_api_timeshift!(cfg, [
        "/timeshift/{username}/{password}/{duration}/{start}/{stream_id}",
        "/timeshift.php",
        "/streaming/timeshift.php"]);
    register_xtream_api_resource!(cfg, [
        ("live", xtream_player_api_live_resource),
        ("movie", xtream_player_api_movie_resource),
        ("series", xtream_player_api_series_resource)]);
    /* TODO
    cfg.service(web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(xtream_player_api_hlsr_stream)));
    cfg.service(web::resource("/hls/{token}/{chunk}").route(web::get().to(xtream_player_api_hls_stream)));
    cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
     */
}