// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::FromStr;

use actix_web::{web, HttpRequest, HttpResponse};
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use futures::Stream;
use log::{debug, error, log_enabled, Level};
use serde_json::{Map, Value};

use crate::api::api_utils::{get_user_server_info, get_user_target, get_user_target_by_credentials, serve_file, stream_response};
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::api::model::xtream::XtreamAuthorizationResponse;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::TargetType;
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::storage::{get_target_storage_path, hash_string};
use crate::repository::target_id_mapping::TargetIdMapping;
use crate::repository::xtream_repository;
use crate::utils::request_utils::mask_sensitive_info;
use crate::utils::{json_utils, request_utils};

const ACTION_GET_SERIES_INFO: &str = "get_series_info";
const ACTION_GET_VOD_INFO: &str = "get_vod_info";
const ACTION_GET_LIVE_INFO: &str = "get_live_info";
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
const TAG_DIRECT_SOURCE: &str = "direct_source";
const TAG_STREAM_ID: &str = "stream_id";
const TAG_MOVIE_DATA: &str = "movie_data";
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
        XtreamCluster::Live => (ACTION_GET_LIVE_INFO, "live_id"),
        XtreamCluster::Video => (ACTION_GET_VOD_INFO, "vod_id"),
        XtreamCluster::Series => (ACTION_GET_SERIES_INFO, "series_id"),
    };
    get_xtream_player_api_action_url(input, action).map(|action_url| format!("{action_url}&{stream_id_field}={stream_id}"))
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
    let server_info = get_user_server_info(cfg, user);
    XtreamAuthorizationResponse::new(&server_info, user)
}

fn xtream_api_request_separate_number_and_rest(input: &str) -> (String, String) {
    input.find('.').map_or_else(|| (input.to_string(), String::new()), |dot_index| {
        let number_part = input[..dot_index].to_string();
        let rest = input[dot_index..].to_string();
        (number_part, rest)
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
    let (action_stream_id, stream_ext) = xtream_api_request_separate_number_and_rest(stream_req.stream_id);
    let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
    let pli = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None).await, true, format!("Failed to read xtream item for stream id {}", virtual_id));
    let input = try_option_bad_request!(app_state.config.get_input_by_id(pli.input_id), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context));

    if pli.item_type == PlaylistItemType::LiveHls {
        let stream_url = pli.url.to_string();
        if log_enabled!(Level::Debug) {
            debug!("Redirecting stream request to {}", mask_sensitive_info(&stream_url));
        }
        return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
    }

    let query_path = if stream_req.action_path.is_empty() {
        format!("{}{stream_ext}", pli.provider_id)
    } else {
        format!("{}/{}{stream_ext}", stream_req.action_path, pli.provider_id)
    };

    if user.proxy == ProxyType::Redirect {
        if log_enabled!(Level::Debug) {
            debug!("Redirecting stream request to {}", mask_sensitive_info(&pli.url));
        }
        return HttpResponse::Found().insert_header(("Location", mask_sensitive_info(pli.url.as_str()))).finish();
    }

    let stream_url = try_option_bad_request!(get_xtream_player_api_stream_url(input, stream_req.context.to_string().as_str(), &query_path, pli.url.as_str()), true, format!("Cant find stream url for target {target_name}, context {}, stream_id {virtual_id}", stream_req.context));
    if log_enabled!(Level::Debug) {
        debug!("Streaming stream request from {}", mask_sensitive_info(&stream_url));
    }
    stream_response(app_state, &stream_url, req, Some(input)).await
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
        if let Some(Value::Object(movie_data)) = doc.get_mut(TAG_MOVIE_DATA) {
            let stream_id = pli.virtual_id;
            let category_id = pli.category_id;
            movie_data.insert(TAG_STREAM_ID.to_string(), Value::Number(serde_json::value::Number::from(stream_id)));
            movie_data.insert(TAG_CATEGORY_ID.to_string(), Value::Number(serde_json::value::Number::from(category_id)));
            let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
            if options.skip_video_direct_source {
                movie_data.insert(TAG_DIRECT_SOURCE.to_string(), Value::String(String::new()));
            } else {
                movie_data.insert(TAG_DIRECT_SOURCE.to_string(), Value::String(pli.url.to_string()));
            }
            if let Ok(result) = serde_json::to_string(&doc) {
                return Ok(result);
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to get vod info for id {}", pli.virtual_id)))
}

async fn xtream_get_stream_info_content(info_url: &str, input: &ConfigInput) -> Result<String, Error> {
    request_utils::download_text_content(input, info_url, None).await
}

async fn xtream_get_stream_info(config: &Config, input: &ConfigInput, target: &ConfigTarget,
                                pli: &XtreamPlaylistItem, info_url: &str, cluster: XtreamCluster) -> Result<String, Error> {
    if cluster == XtreamCluster::Series {
        if let Some(content) = xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.virtual_id).await {
            return Ok(content);
        }
    }

    if let Ok(content) = xtream_get_stream_info_content(info_url, input).await {
        return match cluster {
            XtreamCluster::Live => Ok(content),
            XtreamCluster::Video => get_xtream_vod_info(target, pli, &content),
            XtreamCluster::Series => xtream_repository::write_and_get_xtream_series_info(config, target, pli, &content).await,
        };
    }

    Err(Error::new(ErrorKind::Other, format!("Cant find stream with id: {}/{}/{}",
                                             target.name.replace(' ', "_").as_str(), &cluster, pli.virtual_id)))
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: XtreamCluster) -> HttpResponse {
    let virtual_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(cluster)).await {
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
        let virtual_id: u32 = match FromStr::from_str(stream_id.trim()) {
            Ok(id) => id,
            Err(_) => return HttpResponse::BadRequest().finish()
        };

        if let Ok(pli) = xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None).await {
            let input_id: u16 = pli.input_id;
            if let Some(input) = app_state.config.get_input_by_id(input_id) {
                if let Some(action_url) = get_xtream_player_api_action_url(input, ACTION_GET_SHORT_EPG) {
                    let mut info_url = format!("{action_url}&{TAG_STREAM_ID}={}", pli.provider_id);
                    if !(limit.is_empty() || limit.eq("0")) {
                        info_url = format!("{info_url}&limit={limit}");
                    }
                    if user.proxy == ProxyType::Redirect {
                        return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                    }

                    return match request_utils::download_text_content(input, info_url.as_str(), None).await {
                        Ok(content) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content),
                        Err(err) => {
                            error!("Failed to download epg {}", mask_sensitive_info(err.to_string().as_str()));
                            HttpResponse::NoContent().finish()
                        }
                    };
                }
            }
        }
    }
    error!("Cant find short epg with id: {target_name}/{stream_id}");
    HttpResponse::NoContent().finish()
}

async fn xtream_player_api_handle_content_action(config: &Config, target_name: &str, action: &str, category_id: &str, req: &HttpRequest) -> Option<HttpResponse> {
    if let Ok((path, content)) = match action {
        ACTION_GET_LIVE_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_LIVE),
        ACTION_GET_VOD_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_VOD),
        ACTION_GET_SERIES_CATEGORIES => xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_SERIES),
        _ => Err(Error::new(ErrorKind::Other, ""))
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
    let pli = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, Some(XtreamCluster::Live)).await);
    let input = try_option_bad_request!(app_state.config.get_input_by_id(pli.input_id));
    let info_url = try_option_bad_request!(get_xtream_player_api_action_url(input, ACTION_GET_CATCHUP_TABLE).map(|action_url| format!("{action_url}&{TAG_STREAM_ID}={}&start={start}&end={end}", pli.provider_id)));
    let content = try_result_bad_request!(xtream_get_stream_info_content(info_url.as_str(), input).await);
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

        match action {
            ACTION_GET_SERIES_INFO => {
                return xtream_get_stream_info_response(
                    app_state, &user, target, api_req.series_id.trim(), XtreamCluster::Series,
                ).await;
            }
            ACTION_GET_VOD_INFO => {
                return xtream_get_stream_info_response(
                    app_state, &user, target, api_req.vod_id.trim(), XtreamCluster::Video,
                ).await;
            }
            ACTION_GET_EPG | ACTION_GET_SHORT_EPG => {
                return xtream_get_short_epg(
                    app_state, &user, target, api_req.stream_id.trim(), api_req.limit.trim(),
                ).await;
            }
            ACTION_GET_CATCHUP_TABLE => {
                return xtream_get_catchup_response(
                    app_state, target, api_req.stream_id.trim(), api_req.start.trim(), api_req.end.trim(),
                ).await;
            }
            _ => {}
        }

        // Handle general content actions
        if let Some(response) = xtream_player_api_handle_content_action(
            &app_state.config, &target.name, action, api_req.category_id.trim(), req,
        ).await {
            return response;
        }

        // Process specific playlist actions
        let category_id = api_req.category_id.trim().parse::<u32>().unwrap_or(0);
        let result = match action {
            ACTION_GET_LIVE_STREAMS => xtream_repository::xtream_load_rewrite_playlist(
                XtreamCluster::Live, &app_state.config, target, category_id,
            ).await,
            ACTION_GET_VOD_STREAMS => xtream_repository::xtream_load_rewrite_playlist(
                XtreamCluster::Video, &app_state.config, target, category_id,
            ).await,
            ACTION_GET_SERIES => xtream_repository::xtream_load_rewrite_playlist(
                XtreamCluster::Series, &app_state.config, target, category_id,
            ).await,
            _ => Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Cant find action: {action} for target: {}", &target.name),
            )),
        };

        match result {
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
    stream::iter(xtream_iter.map(move |line| {
        let line = if first_item {
            first_item = false;
            format!("[{line}")
        } else {
            format!(",{line}")
        };
        Ok::<Bytes, String>(Bytes::from(line))
    }))
        .chain(stream::once(async { Ok::<Bytes, String>(Bytes::from("]")) }))
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

pub fn xtream_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/player_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)))
        .service(web::resource("/panel_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)))
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