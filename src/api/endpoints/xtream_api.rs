// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use crate::api::api_utils;
use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, resource_response, separate_number_and_remainder, serve_file, stream_response};
use crate::api::api_utils::{redirect, try_option_bad_request, try_result_bad_request};
use crate::api::endpoints::hls_api::handle_hls_stream_request;
use crate::api::endpoints::xmltv_api::get_empty_epg_response;
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::streams::provider_stream::{create_custom_video_stream_response, CustomVideoStreamType};
use crate::api::model::xtream::XtreamAuthorizationResponse;
use crate::m3u_filter_error::info_err;
use crate::m3u_filter_error::create_m3u_filter_error_result;
use crate::m3u_filter_error::{str_to_io_error, M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials, UserConnectionPermission};
use crate::model::config::TargetType;
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{get_backdrop_path_value, FieldGetAccessor, PlaylistEntry, PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::{INFO_RESOURCE_PREFIX, INFO_RESOURCE_PREFIX_EPISODE, PROP_BACKDROP_PATH, SEASON_RESOURCE_PREFIX};
use crate::repository::playlist_repository::get_target_id_mapping;
use crate::repository::storage::{get_target_storage_path, hex_encode};
use crate::repository::xtream_repository::{TAG_EPISODES, TAG_INFO_DATA, TAG_SEASONS_DATA};
use crate::repository::{user_repository, xtream_repository};
use crate::utils::debug_if_enabled;
use crate::utils::hash_utils::generate_playlist_uuid;
use crate::utils::json_utils;
use crate::utils::json_utils::get_u32_from_serde_value;
use crate::utils::network::request::{extract_extension_from_url, replace_url_extension, sanitize_sensitive_info, DASH_EXT, HLS_EXT};
use crate::utils::network::xtream::{create_vod_info_from_item, ACTION_GET_LIVE_CATEGORIES, ACTION_GET_LIVE_STREAMS, ACTION_GET_SERIES, ACTION_GET_SERIES_CATEGORIES, ACTION_GET_SERIES_INFO, ACTION_GET_VOD_CATEGORIES, ACTION_GET_VOD_INFO, ACTION_GET_VOD_STREAMS};
use crate::utils::network::{request, xtream};
use crate::utils::trace_if_enabled;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use futures::Stream;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

const ACTION_GET_ACCOUNT_INFO: &str = "get_account_info";
const ACTION_GET_EPG: &str = "get_epg";
const ACTION_GET_SHORT_EPG: &str = "get_short_epg";
const ACTION_GET_CATCHUP_TABLE: &str = "get_simple_data_table";
const TAG_ID: &str = "id";
const TAG_CATEGORY_ID: &str = "category_id";
const TAG_STREAM_ID: &str = "stream_id";
const TAG_EPG_LISTINGS: &str = "epg_listings";

#[derive(Serialize, Deserialize, Debug)]
pub(in crate::api) enum XtreamApiStreamContext {
    LiveAlt,
    Live,
    Movie,
    Series,
    Timeshift,
}

impl XtreamApiStreamContext {
    const LIVE: &'static str = "live";
    const MOVIE: &'static str = "movie";
    const SERIES: &'static str = "series";
    const TIMESHIFT: &'static str = "timeshift";
}

impl Display for XtreamApiStreamContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Live | Self::LiveAlt => Self::LIVE,
            Self::Movie => Self::MOVIE,
            Self::Series => Self::SERIES,
            Self::Timeshift => Self::TIMESHIFT,
        })
    }
}

impl TryFrom<XtreamCluster> for XtreamApiStreamContext {
    type Error = String;
    fn try_from(cluster: XtreamCluster) -> Result<Self, Self::Error> {
        match cluster {
            XtreamCluster::Live => Ok(Self::Live),
            XtreamCluster::Video => Ok(Self::Movie),
            XtreamCluster::Series => Ok(Self::Series),
        }
    }
}

impl FromStr for XtreamApiStreamContext {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        match s.to_lowercase().as_str() {
            Self::LIVE => Ok(Self::Live),
            Self::MOVIE => Ok(Self::Movie),
            Self::SERIES => Ok(Self::Series),
            Self::TIMESHIFT => Ok(Self::Timeshift),
            _ => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown CounterModifier: {}", s)
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct XtreamApiStreamRequest<'a> {
    context: XtreamApiStreamContext,
    access_token: bool,
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
            access_token: false,
            username,
            password,
            stream_id,
            action_path,
        }
    }
    pub const fn from_access_token(context: XtreamApiStreamContext,
                                   password: &'a str,
                                   stream_id: &'a str,
                                   action_path: &'a str) -> Self {
        Self {
            context,
            access_token: false,
            username: "",
            password,
            stream_id,
            action_path,
        }
    }
}

pub fn serve_query(file_path: &Path, filter: &HashMap<&str, HashSet<String>>) -> impl IntoResponse + Send {
    let filtered = json_utils::json_filter_file(file_path, filter);
    axum::Json(filtered)
}

pub(in crate::api) fn get_xtream_player_api_stream_url(input: &ConfigInput, context: &XtreamApiStreamContext, action_path: &str, fallback_url: &str) -> Option<String> {
    if let Some(input_user_info) = input.get_user_info() {
        let ctx = match context {
            XtreamApiStreamContext::LiveAlt |
            XtreamApiStreamContext::Live => {
                let use_prefix = input.options.as_ref().is_none_or(|o| o.xtream_live_stream_use_prefix);
                String::from(if use_prefix { "live" } else { "" })
            }
            XtreamApiStreamContext::Movie
            | XtreamApiStreamContext::Series
            | XtreamApiStreamContext::Timeshift => context.to_string()
        };
        let ctx_path = if ctx.is_empty() { String::new() } else { format!("{ctx}/") };
        Some(format!("{}/{}{}/{}/{}",
                     &input_user_info.base_url,
                     ctx_path,
                     &input_user_info.username,
                     &input_user_info.password,
                     action_path
        ))
    } else if !fallback_url.is_empty() {
        Some(String::from(fallback_url))
    } else {
        None
    }
}

async fn get_user_info(user: &ProxyUserCredentials, app_state: &AppState) -> XtreamAuthorizationResponse {
    let server_info = app_state.config.get_user_server_info(user).await;
    let active_connections = app_state.get_active_connections_for_user(&user.username).await;
    XtreamAuthorizationResponse::new(&server_info, user, active_connections, app_state.config.user_access_control)
}

async fn xtream_player_api_stream(
    req_headers: &HeaderMap,
    api_req: &UserApiRequest,
    app_state: &Arc<AppState>,
    stream_req: XtreamApiStreamRequest<'_>,
) -> impl IntoResponse + Send {
    let (user, target) = try_option_bad_request!(get_user_target_by_credentials(stream_req.username, stream_req.password, api_req, app_state).await, false, format!("Could not find any user {}", stream_req.username));
    if user.permission_denied(app_state) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let connection_permission = user.connection_permission(&app_state).await;
    if connection_permission == UserConnectionPermission::Exhausted {
        return create_custom_video_stream_response(&app_state.config, &CustomVideoStreamType::UserConnectionsExhausted).into_response();
    }

    let target_name = &target.name;
    if !target.has_output(&TargetType::Xtream) {
        debug!("Target has no xtream output {target_name}");
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    let (action_stream_id, stream_ext) = separate_number_and_remainder(stream_req.stream_id);
    let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
    let (pli, mapping) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
    let input = try_option_bad_request!(app_state.config.get_input_by_name(pli.input_name.as_str()), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context));

    let is_hls_request = pli.item_type == PlaylistItemType::LiveHls || stream_ext.as_deref() == Some(HLS_EXT);

    if user.proxy == ProxyType::Redirect {
        if pli.xtream_cluster == XtreamCluster::Series {
            let ext = stream_ext.unwrap_or_else(String::new);
            let url = input.url.as_str();
            let username = input.username.as_ref().map_or("", |v| v);
            let password = input.password.as_ref().map_or("", |v| v);
            let stream_url = format!("{url}/series/{username}/{password}/{}{ext}", mapping.provider_id);
            return redirect(&stream_url).into_response();
        }

        if is_hls_request  || pli.item_type == PlaylistItemType::LiveDash {
            let redirect_url = if is_hls_request { &replace_url_extension(&pli.url, HLS_EXT)  } else { &replace_url_extension(&pli.url, DASH_EXT) };
            debug_if_enabled!("Redirecting stream request to {}", sanitize_sensitive_info(redirect_url));
            return redirect(redirect_url).into_response();
        }

        debug_if_enabled!("Redirecting stream request to {}", sanitize_sensitive_info(&pli.url));
        return redirect(&pli.url).into_response();
    }

    // Reverse proxy mode
    if is_hls_request {
        return handle_hls_stream_request(app_state, &user, &pli.url, pli.virtual_id, input).await.into_response();
    }

    let extension = stream_ext.unwrap_or_else(
        || extract_extension_from_url(&pli.url).map_or_else(String::new, std::string::ToString::to_string));

    let query_path = if stream_req.action_path.is_empty() {
        format!("{}{extension}", pli.provider_id)
    } else {
        format!("{}/{}{extension}", stream_req.action_path, pli.provider_id)
    };

    let stream_url = try_option_bad_request!(get_xtream_player_api_stream_url(input,
        &stream_req.context, &query_path, pli.url.as_str()),
        true, format!("Cant find stream url for target {target_name}, context {}, stream_id {virtual_id}",
        stream_req.context));

    trace_if_enabled!("Streaming stream request from {}", sanitize_sensitive_info(&stream_url));
    stream_response(app_state, &stream_url, req_headers, Some(input), pli.item_type, target, &user, connection_permission).await.into_response()
}

async fn xtream_player_api_stream_with_token(
    req_headers: &HeaderMap,
    app_state: &Arc<AppState>,
    target_id: u16,
    stream_req: XtreamApiStreamRequest<'_>,
) -> impl IntoResponse + Send {
    if let Some(target) = app_state.config.get_target_by_id(target_id) {
        let target_name = &target.name;
        if !target.has_output(&TargetType::Xtream) {
            debug!("Target has no xtream output {target_name}");
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
        let (action_stream_id, stream_ext) = separate_number_and_remainder(stream_req.stream_id);
        let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
        let (pli, _mapping) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
        let input = try_option_bad_request!(app_state.config.get_input_by_name(pli.input_name.as_str()), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context));

        let is_hls_request = pli.item_type == PlaylistItemType::LiveHls || stream_ext.as_deref() == Some(HLS_EXT);

        let server = app_state.config.web_ui.as_ref().and_then(|web_ui| web_ui.player_server.as_ref()).map_or("default", |server_name| server_name.as_str());

        let user = ProxyUserCredentials {
            username: "api_user".to_string(),
            password: "api_user".to_string(),
            token: None,
            proxy: ProxyType::Reverse,
            server: Some(server.to_string()),
            epg_timeshift: None,
            created_at: None,
            exp_date: None,
            max_connections: 0,
            status: None,
            ui_enabled: false,
        };

        // Reverse proxy mode
        if is_hls_request {
            return handle_hls_stream_request(app_state, &user, &pli.url, pli.virtual_id, input).await.into_response();
        }

        let extension = stream_ext.unwrap_or_else(
            || extract_extension_from_url(&pli.url).map_or_else(String::new, std::string::ToString::to_string));

        let query_path = if stream_req.action_path.is_empty() {
            format!("{}{extension}", pli.provider_id)
        } else {
            format!("{}/{}{extension}", stream_req.action_path, pli.provider_id)
        };

        let stream_url = try_option_bad_request!(get_xtream_player_api_stream_url(input,
        &stream_req.context, &query_path, pli.url.as_str()),
        true, format!("Cant find stream url for target {target_name}, context {}, stream_id {virtual_id}",
        stream_req.context));

        trace_if_enabled!("Streaming stream request from {}", sanitize_sensitive_info(&stream_url));
        stream_response(app_state, &stream_url, req_headers, Some(input), pli.item_type, target, &user, UserConnectionPermission::Allowed).await.into_response()
    } else {
        axum::http::StatusCode::BAD_REQUEST.into_response()
    }
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

fn get_doc_resource_field_value(field: &str, doc: Option<&Value>) -> Option<String> {
    if let Some(Value::Object(info_data)) = doc {
        if field.starts_with(PROP_BACKDROP_PATH) {
            return get_backdrop_path_value(field, info_data.get(PROP_BACKDROP_PATH));
        } else if let Some(Value::String(url)) = info_data.get(field) {
            return Some(url.to_string());
        }
    }
    None
}

fn xtream_get_info_resource_url(config: &Config, pli: &XtreamPlaylistItem, target: &ConfigTarget, resource: &str) -> Result<Option<String>, serde_json::Error> {
    let info_content = match pli.xtream_cluster {
        XtreamCluster::Video => {
            xtream_repository::xtream_load_vod_info(config, target.name.as_str(), pli.get_virtual_id())
        }
        XtreamCluster::Series => {
            xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.get_virtual_id())
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


fn xtream_get_season_resource_url(config: &Config, pli: &XtreamPlaylistItem, target: &ConfigTarget, resource: &str) -> Result<Option<String>, serde_json::Error> {
    let info_content = match pli.xtream_cluster {
        XtreamCluster::Series => {
            xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.get_virtual_id())
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
    req_headers: &HeaderMap,
    api_req: &UserApiRequest,
    app_state: &Arc<AppState>,
    resource_req: XtreamApiStreamRequest<'_>,
) -> impl IntoResponse {
    let (user, target) = try_option_bad_request!(get_user_target_by_credentials(resource_req.username, resource_req.password, api_req, app_state).await, false, format!("Could not find any user {}", resource_req.username));
    if user.permission_denied(app_state) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let target_name = &target.name;
    if !target.has_output(&TargetType::Xtream) {
        debug!("Target has no xtream output {target_name}");
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    let virtual_id: u32 = try_result_bad_request!(resource_req.stream_id.trim().parse());
    let resource = resource_req.action_path.trim();
    let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
    let stream_url = if resource.starts_with(INFO_RESOURCE_PREFIX) {
        try_result_bad_request!(xtream_get_info_resource_url(&app_state.config, &pli, target, resource))
    } else if resource.starts_with(SEASON_RESOURCE_PREFIX) {
        try_result_bad_request!(xtream_get_season_resource_url(&app_state.config, &pli, target, resource))
    } else {
        pli.get_field(resource)
    };

    match stream_url {
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
        Some(url) => {
            if user.proxy == ProxyType::Redirect {
                trace_if_enabled!("Redirecting resource request to {}", sanitize_sensitive_info(&url));
                redirect(url.as_str()).into_response()
            } else {
                trace_if_enabled!("Resource request to {}", sanitize_sensitive_info(&url));
                resource_response(app_state, url.as_str(), req_headers, None).await.into_response()
            }
        }
    }
}

macro_rules! create_xtream_player_api_resource {
    ($fn_name:ident, $context:expr) => {
        async fn $fn_name(
            axum::extract::Path((username, password, stream_id, resource)): axum::extract::Path<(String, String, String, String)>,
            axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
            axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
            req_headers: HeaderMap,
        ) ->  impl IntoResponse {
            xtream_player_api_resource(&req_headers, &api_req, &app_state, XtreamApiStreamRequest::from($context, &username, &password, &stream_id, &resource)).await.into_response()
        }
    }
}

macro_rules! create_xtream_player_api_stream {
    ($fn_name:ident, $context:expr) => {
        async fn $fn_name(
            axum::extract::Path((username, password, stream_id)): axum::extract::Path<(String, String, String)>,
            axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
            axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
            req_headers: HeaderMap,
        ) ->  impl IntoResponse + Send {
            xtream_player_api_stream(&req_headers, &api_req, &app_state, XtreamApiStreamRequest::from($context, &username, &password, &stream_id, "")).await.into_response()
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
    req_headers: HeaderMap,
    axum::extract::Query(api_query_req): axum::extract::Query<UserApiRequest>,
    axum::extract::Path((path_username, path_password, path_duration, path_start, path_stream_id)): axum::extract::Path<(String, String, String, String, String)>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Form(api_form_req): axum::extract::Form<UserApiRequest>,
) -> impl IntoResponse + Send {
    let username = get_non_empty(&path_username, &api_query_req.username, &api_form_req.username);
    let password = get_non_empty(&path_password, &api_query_req.password, &api_form_req.password);
    let stream_id = get_non_empty(&path_stream_id, &api_query_req.stream, &api_form_req.stream);
    let duration = get_non_empty(&path_duration, &api_query_req.duration, &api_form_req.duration);
    let start = get_non_empty(&path_start, &api_query_req.start, &api_form_req.start);
    let action_path = format!("{duration}/{start}");
    xtream_player_api_stream(&req_headers, &api_query_req, &app_state, XtreamApiStreamRequest::from(XtreamApiStreamContext::Timeshift, username, password, stream_id, &action_path)).await
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: XtreamCluster) -> impl IntoResponse + Send {
    let virtual_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response()
    };

    if let Ok((pli, virtual_record)) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(cluster)) {
        if pli.provider_id > 0 {
            let input_name = &pli.input_name;
            if let Some(input) = app_state.config.get_input_by_name(input_name.as_str()) {
                if let Some(info_url) = xtream::get_xtream_player_api_info_url(input, cluster, pli.provider_id) {
                    // Redirect is only possible for live streams, vod and series info needs to be modified
                    if user.proxy == ProxyType::Redirect && cluster == XtreamCluster::Live {
                        return redirect(&info_url).into_response();
                    } else if let Ok(content) = xtream::get_xtream_stream_info(Arc::clone(&app_state.http_client), &app_state.config, user, input, target, &pli, info_url.as_str(), cluster).await {
                        return axum::response::Response::builder()
                            .status(StatusCode::OK)
                            .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                            .body(axum::body::Body::from(content))
                            .unwrap()
                            .into_response()
                    }
                }
            }
        }

        return match cluster {
            XtreamCluster::Video => {
                let content = create_vod_info_from_item(user, &pli, virtual_record.last_updated);
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                    .body(axum::body::Body::from(content))
                    .unwrap()
                    .into_response()
            }
            XtreamCluster::Live | XtreamCluster::Series => axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                .body(axum::body::Body::from("{}".as_bytes()))
                .unwrap()
                .into_response(),
        };
    }
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
        .body(axum::body::Body::from("{}".as_bytes()))
        .unwrap()
        .into_response()
}

async fn xtream_get_short_epg(app_state: &AppState, user: &ProxyUserCredentials, target: &ConfigTarget, stream_id: &str, limit: &str) -> impl IntoResponse + Send {
    let target_name = &target.name;
    if target.has_output(&TargetType::Xtream) {
        let virtual_id: u32 = match FromStr::from_str(stream_id.trim()) {
            Ok(id) => id,
            Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response()
        };

        if let Ok((pli, _)) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None) {
            if pli.provider_id > 0 {
                let input_name = &pli.input_name;
                if let Some(input) = app_state.config.get_input_by_name(input_name.as_str()) {
                    if let Some(action_url) = xtream::get_xtream_player_api_action_url(input, ACTION_GET_SHORT_EPG) {
                        let mut info_url = format!("{action_url}&{TAG_STREAM_ID}={}", pli.provider_id);
                        if !(limit.is_empty() || limit.eq("0")) {
                            info_url = format!("{info_url}&limit={limit}");
                        }
                        if user.proxy == ProxyType::Redirect {
                            return redirect(&info_url).into_response();
                        }

                        // TODO serve epg from own db
                        return match request::download_text_content(Arc::clone(&app_state.http_client), input, info_url.as_str(), None).await {
                            Ok(content) => (axum::http::StatusCode::OK, axum::Json(content)).into_response(),
                            Err(err) => {
                                error!("Failed to download epg {}", sanitize_sensitive_info(err.to_string().as_str()));
                                get_empty_epg_response().into_response()
                            }
                        };
                    }
                }
            }
        }
    }
    warn!("Cant find short epg with id: {target_name}/{stream_id}");
    get_empty_epg_response().into_response()
}

async fn xtream_player_api_handle_content_action(config: &Config, target_name: &str, action: &str, category_id: Option<u32>, user: &ProxyUserCredentials) -> Option<impl IntoResponse> {
    if let Ok((path, content)) = match action {
        ACTION_GET_LIVE_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_LIVE),
        ACTION_GET_VOD_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_VOD),
        ACTION_GET_SERIES_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_SERIES),
        _ => Err(str_to_io_error(""))
    } {
        if let Some(file_path) = path {
            // load user bouquet
            let filter = match action {
                ACTION_GET_LIVE_CATEGORIES => user_repository::user_get_bouquet_filter(config, &user.username, category_id, TargetType::Xtream, XtreamCluster::Live).await,
                ACTION_GET_VOD_CATEGORIES => user_repository::user_get_bouquet_filter(config, &user.username, category_id, TargetType::Xtream, XtreamCluster::Video).await,
                ACTION_GET_SERIES_CATEGORIES => user_repository::user_get_bouquet_filter(config, &user.username, category_id, TargetType::Xtream, XtreamCluster::Series).await,
                _ => None
            };
            if let Some(flt) = filter {
                return Some(serve_query(&file_path, &HashMap::from([(TAG_CATEGORY_ID, flt)])).into_response());
            }
            return Some(serve_file(&file_path, mime::APPLICATION_JSON).await.into_response());
        } else if let Some(payload) = content {
            return Some(axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .body(payload).unwrap().into_response());
        }
        return Some(api_utils::empty_json_list_response().into_response());
    }
    None
}

async fn xtream_get_catchup_response(app_state: &AppState, target: &ConfigTarget, stream_id: &str, start: &str, end: &str) -> impl IntoResponse + Send {
    let virtual_id: u32 = try_result_bad_request!(FromStr::from_str(stream_id));
    let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(XtreamCluster::Live)));
    let input = try_option_bad_request!(app_state.config.get_input_by_name(pli.input_name.as_str()));
    let info_url = try_option_bad_request!(xtream::get_xtream_player_api_action_url(input, ACTION_GET_CATCHUP_TABLE).map(|action_url| format!("{action_url}&{TAG_STREAM_ID}={}&start={start}&end={end}", pli.provider_id)));
    let content = try_result_bad_request!(xtream::get_xtream_stream_info_content(Arc::clone(&app_state.http_client), info_url.as_str(), input).await);
    let mut doc: Map<String, Value> = try_result_bad_request!(serde_json::from_str(&content));
    let epg_listings = try_option_bad_request!(doc.get_mut(TAG_EPG_LISTINGS).and_then(Value::as_array_mut));
    let target_path = try_option_bad_request!(get_target_storage_path(&app_state.config, target.name.as_str()));
    let (mut target_id_mapping, file_lock) = get_target_id_mapping(&app_state.config, &target_path).await;
    for epg_list_item in epg_listings.iter_mut().filter_map(Value::as_object_mut) {
        // TODO epg_id
        if let Some(catchup_provider_id) = epg_list_item.get(TAG_ID).and_then(Value::as_str).and_then(|id| id.parse::<u32>().ok()) {
            let uuid = generate_playlist_uuid(&hex_encode(&pli.get_uuid()), &catchup_provider_id.to_string(), pli.item_type, &pli.url);
            let virtual_id = target_id_mapping.get_and_update_virtual_id(&uuid, catchup_provider_id, PlaylistItemType::Catchup, pli.provider_id);
            epg_list_item.insert(TAG_ID.to_string(), Value::String(virtual_id.to_string()));
        }
    }
    if let Err(err) = target_id_mapping.persist() {
        error!("Failed to write catchup id mapping {err}");
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    drop(file_lock);
    serde_json::to_string(&doc)
        .map_or_else(
            |_| axum::http::StatusCode::BAD_REQUEST.into_response(),
            |result| axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                .body(result).unwrap().into_response())
}

macro_rules! skip_json_response_if_flag_set {
    ($flag:expr, $stmt:expr) => {
        if $flag {
            return api_utils::empty_json_list_response().into_response();
        }
        return $stmt.into_response();
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
    api_req: UserApiRequest,
    app_state: &Arc<AppState>,
) -> impl IntoResponse + Send {
    let user_target = get_user_target(&api_req, app_state).await;
    if let Some((user, target)) = user_target {
        if !target.has_output(&TargetType::Xtream) {
            return axum::response::Json(get_user_info(&user, app_state).await).into_response();
        }

        let action = api_req.action.trim();
        if action.is_empty() {
            return axum::response::Json(get_user_info(&user, app_state).await).into_response();
        }

        if user.permission_denied(app_state) {
            return axum::http::StatusCode::FORBIDDEN.into_response();
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
            ACTION_GET_ACCOUNT_INFO => {
                return axum::response::Json(get_user_info(&user, app_state).await).into_response();
            }
            ACTION_GET_SERIES_INFO => {
                skip_json_response_if_flag_set!(skip_series, xtream_get_stream_info_response(app_state, &user, target, api_req.series_id.trim(), XtreamCluster::Series).await);
            }
            ACTION_GET_VOD_INFO => {
                skip_json_response_if_flag_set!(skip_vod,  xtream_get_stream_info_response(app_state, &user, target, api_req.vod_id.trim(), XtreamCluster::Video).await);
            }
            ACTION_GET_EPG | ACTION_GET_SHORT_EPG => {
                return xtream_get_short_epg(
                    app_state, &user, target, api_req.stream_id.trim(), api_req.limit.trim(),
                ).await.into_response();
            }
            ACTION_GET_CATCHUP_TABLE => {
                skip_json_response_if_flag_set!(skip_live, xtream_get_catchup_response(app_state, target, api_req.stream_id.trim(), api_req.start.trim(), api_req.end.trim()).await);
            }
            _ => {}
        }

        let category_id = api_req.category_id.trim().parse::<u32>().ok();
        // Handle general content actions
        if let Some(response) = xtream_player_api_handle_content_action(
            &app_state.config, &target.name, action, category_id, &user,
        ).await {
            return response.into_response();
        }

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
                        axum::response::Response::builder()
                            .status(axum::http::StatusCode::OK)
                            .header(axum::http::header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                            .body(axum::body::Body::from_stream(content_stream)).unwrap().into_response()
                    }
                    Err(err) => {
                        error!("Failed response for xtream target: {} action: {} error: {}", &target.name, action, err);
                        // Some players fail on NoContent, so we return an empty array
                        api_utils::empty_json_list_response().into_response()
                    }
                }
            }
            None => {
                // Some players fail on NoContent, so we return an empty array
                api_utils::empty_json_list_response().into_response()
            }
        }
    } else {
        match (user_target.is_none(), api_req.action.is_empty()) {
            (true, _) => debug!("Cant find user!"),
            (_, true) => debug!("Parameter action is empty!"),
            _ => debug!("Bad request!"),
        }
        axum::http::StatusCode::BAD_REQUEST.into_response()
    }
}

fn xtream_create_content_stream(xtream_iter: impl Iterator<Item=(String, bool)>) -> impl Stream<Item=Result<Bytes, String>> {
    stream::once(async { Ok::<Bytes, String>(Bytes::from("[")) }).chain(
        stream::iter(xtream_iter.map(move |(line, has_next)| {
            Ok::<Bytes, String>(Bytes::from(if has_next {
                format!("{line},")
            } else {
                line.to_string()
            }))
        })).chain(stream::once(async { Ok::<Bytes, String>(Bytes::from("]")) })))
}

async fn xtream_player_api_get(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
) -> impl IntoResponse + Send {
    xtream_player_api(api_req, &app_state).await
}


async fn xtream_player_api_post(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Form(api_req): axum::extract::Form<UserApiRequest>,
) -> impl IntoResponse + Send {
    xtream_player_api(api_req, &app_state).await
}

macro_rules! register_xtream_api {
    ($router:expr, [$($path:expr),*]) => {{
        $router
       $(
          .route($path, axum::routing::get(xtream_player_api_get))
          .route($path, axum::routing::post(xtream_player_api_post))
            // $router.service(web::resource($path).route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)))
        )*
    }};
}

macro_rules! register_xtream_api_stream {
     ($router:expr, [$(($path:expr, $fn_name:ident)),*]) => {{
         $router
       $(
          .route(format!("{}/{{username}}/{{password}}/{{stream_id}}", $path).as_str(), axum::routing::get($fn_name))
            // $cfg.service(web::resource(format!("{}/{{username}}/{{password}}/{{stream_id}}", $path)).route(web::get().to($fn_name)));
        )*
    }};
}

macro_rules! register_xtream_api_resource {
     ($router:expr, [$(($path:expr, $fn_name:ident)),*]) => {{
         $router
       $(
           .route(format!("/resource/{}/{{username}}/{{password}}/{{stream_id}}/{{resource}}", $path).as_str(), axum::routing::get($fn_name))
            // $cfg.service(web::resource(format!("/resource/{}/{{username}}/{{password}}/{{stream_id}}/{{resource}}", $path)).route(web::get().to($fn_name)));
        )*
    }};
}

macro_rules! register_xtream_api_timeshift {
     ($router:expr, [$($path:expr),*]) => {{
         $router
       $(
          .route($path, axum::routing::get(xtream_player_api_timeshift_stream))
          .route($path, axum::routing::post(xtream_player_api_timeshift_stream))
            //$cfg.service(web::resource($path).route(web::get().to(xtream_player_api_timeshift_stream)).route(web::post().to(xtream_player_api_timeshift_stream)));
        )*
    }};
}

async fn xtream_player_token_stream(
    axum::extract::Path((token, target_id, cluster, stream_id)): axum::extract::Path<(String, u16, String, String)>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    req_headers: HeaderMap,
) -> impl IntoResponse + Send {
    let ctxt = try_result_bad_request!(XtreamApiStreamContext::from_str(cluster.as_str()));
    xtream_player_api_stream_with_token(&req_headers, &app_state, target_id, XtreamApiStreamRequest::from_access_token(ctxt, &token, &stream_id, "")).await.into_response()
}

pub fn xtream_api_register() -> axum::Router<Arc<AppState>> {
    let router = axum::Router::new();
    let mut router = register_xtream_api!(router, ["/player_api.php", "/panel_api.php", "/xtream"]);
    router = router.route("/token/{token}/{target_id}/{cluster}/{stream_id}", axum::routing::get(xtream_player_token_stream));
    router = register_xtream_api_stream!(router, [
        ("", xtream_player_api_live_stream_alt),
        ("/live", xtream_player_api_live_stream),
        ("/movie", xtream_player_api_movie_stream),
        ("/series", xtream_player_api_series_stream)]);
    router = register_xtream_api_timeshift!(router, [
        "/timeshift/{username}/{password}/{duration}/{start}/{stream_id}",
        "/timeshift.php",
        "/streaming/timeshift.php"]);
    register_xtream_api_resource!(router, [
        ("live", xtream_player_api_live_resource),
        ("movie", xtream_player_api_movie_resource),
        ("series", xtream_player_api_series_resource)])
}