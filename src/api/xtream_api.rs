// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::FromStr;

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Local};
use log::{debug, error};
use serde_json::{Map, Value};
use url::Url;

use crate::api::api_model::{AppState, UserApiRequest, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::api::api_utils::{get_user_server_info, get_user_target, get_user_target_by_credentials, serve_file, stream_response};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::config::TargetType;
use crate::model::playlist::{XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::xtream_repository;
use crate::repository::xtream_repository::get_xtream_item_for_stream_id;
use crate::utils::{json_utils, request_utils};

pub(crate) async fn serve_query(file_path: &Path, filter: &HashMap<&str, &str>) -> HttpResponse {
    let filtered = json_utils::filter_json_file(file_path, filter);
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

fn get_xtream_player_api_info_url(input: &ConfigInput, cluster: &XtreamCluster, stream_id: u32) -> Option<String> {
    let (action, stream_id_field) = match cluster {
        XtreamCluster::Live => ("get_live_info", "live_id"),
        XtreamCluster::Video => ("get_vod_info", "vod_id"),
        XtreamCluster::Series => ("get_series_info", "series_id"),
    };
    get_xtream_player_api_action_url(input, action).map(|action_url| format!("{}&{}={}", action_url, stream_id_field, stream_id))
}

fn get_xtream_player_api_stream_url(input: &ConfigInput, context: &str, action_path: &str) -> Option<String> {
    let ctx_path = if context.is_empty() { "".to_string() } else { format!("{}/", context) };
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
            allowed_output_formats: Vec::from(["ts".to_string()]),
            auth: 1,
            created_at: (now - Duration::days(365)).timestamp(), // fake
            exp_date: (now + Duration::days(365)).timestamp(),// fake
            is_trial: "0".to_string(),
            max_connections: "1".to_string(),
            message: server_info.message.to_string(),
            password: user.password.to_string(),
            username: user.username.to_string(),
            status: "Active".to_string(),
        },
        server_info: XtreamServerInfo {
            url: server_info.host.to_owned(),
            port: server_info.http_port.to_owned(),
            https_port: server_info.https_port.to_owned(),
            server_protocol: server_info.protocol.clone(),
            rtmp_port: server_info.rtmp_port.to_owned(),
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
        XtreamApiStreamRequest {
            context,
            username,
            password,
            stream_id,
            action_path,
        }
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
            let req_stream_id: u32 = match FromStr::from_str(action_stream_id.trim()) {
                Ok(id) => id,
                Err(_) => return HttpResponse::BadRequest().finish()
            };

            match get_xtream_item_for_stream_id(req_stream_id, &app_state.config, target, None) {
                Ok(pli) => {
                    let input_id: u16 = pli.input_id;
                    if let Some(input) = app_state.config.get_input_by_id(&input_id) {
                        let mut query_path = if stream_req.action_path.is_empty() { "".to_string() } else { format!("{}/", stream_req.action_path) };
                        query_path = format!("{}{}{}", query_path, pli.id, stream_ext);
                        if let Some(stream_url) = get_xtream_player_api_stream_url(input, stream_req.context.to_string().as_str(), query_path.as_str()) {
                            if user.proxy == ProxyType::Redirect {
                                debug!("Redirecting stream request to {}", stream_url);
                                return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
                            }
                            return stream_response(&stream_url, req, Some(input)).await;
                        } else {
                            error!("Cant find stream url for target {}, context {}, stream_id {}", target_name, stream_req.context, req_stream_id);
                        }
                    } else {
                        error!("Cant find input for target {}, context {}, stream_id {}", target_name, stream_req.context, req_stream_id);
                    }
                }
                Err(_) => error!("Failed to read xtream item for stream id {}", req_stream_id),
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
    xtream_player_api_stream(&req, &api_req, &app_state,  XtreamApiStreamRequest::from(XtreamApiStreamContext::LiveAlt, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_series_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state,  XtreamApiStreamRequest::from(XtreamApiStreamContext::Series, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_movie_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &app_state,  XtreamApiStreamRequest::from(XtreamApiStreamContext::Movie, &username, &password, &stream_id, "")).await
}

async fn xtream_player_api_timeshift_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, duration, start, stream_id) = path.into_inner();
    let action_path = format!("{}/{}/", duration, start);
    xtream_player_api_stream(&req, &api_req, &app_state,  XtreamApiStreamRequest::from(XtreamApiStreamContext::Timeshift, &username, &password, &stream_id, &action_path)).await
}

async fn xtream_get_stream_info(input: &ConfigInput, target: &ConfigTarget, pli: &XtreamPlaylistItem,
                                info_url: &str, cluster: &XtreamCluster) -> Result<String, Error> {
    if let Ok(url) = Url::parse(info_url) {
        let client = request_utils::get_client_request(Some(input), url, None);
        if let Ok(response) = client.send().await {
            debug!("get stream info response status code {}", response.status());
            if response.status().is_success() {
                match response.text().await {
                    Ok(content) => {
                        return match cluster {
                            XtreamCluster::Live => Ok(content),
                            XtreamCluster::Video => {
                                if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content.as_str()) {
                                    if let Some(Value::Object(movie_data) ) = doc.get_mut("movie_data") {
                                        let stream_id = pli.id;
                                        let category_id = pli.category_id;
                                        movie_data.insert("stream_id".to_string(), Value::Number(serde_json::value::Number::from(stream_id)));
                                        movie_data.insert("category_id".to_string(), Value::Number(serde_json::value::Number::from(category_id)));
                                        let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
                                        if options.skip_video_direct_source {
                                            movie_data.insert("direct_source".to_string(), Value::String("".to_string()));
                                        } else {
                                            movie_data.insert("direct_source".to_string(), Value::String(pli.url.to_string()));
                                        }
                                        if let Ok(result) = serde_json::to_string(&doc) {
                                            return Ok(result);
                                        }
                                    }
                                }
                                return Ok(content);
                            }
                            XtreamCluster::Series => {
                                // With series we have a problem.
                                // it could be that the series are resolved and assigned to a new stream_id
                                // what now ?

                                return Ok(content);
                            }
                        };
                    }
                    Err(err) => { error!("Failed to download info {}", err.to_string()); }
                }
            }
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find stream with id: {}/{}/{}",
                                                      target.name.as_str(), &cluster, pli.stream_id)))
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: &XtreamCluster) -> HttpResponse {
    let req_stream_id: u32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if let Ok(pli) = get_xtream_item_for_stream_id(req_stream_id, &app_state.config, target, Some(cluster)) {
        let input_id = pli.input_id;
        if let Some(input) = app_state.config.get_input_by_id(&input_id) {
            let stream_id = pli.id;
            if let Some(info_url) = get_xtream_player_api_info_url(input, cluster, stream_id) {
                if user.proxy == ProxyType::Redirect {
                    return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                } else if let Ok(content) = xtream_get_stream_info(input, target, &pli, info_url.as_str(), cluster).await {
                    return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content);
                }
            }
        }
    }
    return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{info:[]}");
}

async fn xtream_get_short_epg(app_state: &AppState, user: &ProxyUserCredentials, target: &ConfigTarget, stream_id: &str, limit: &str) -> HttpResponse {
    let target_name = &target.name;
    if target.has_output(&TargetType::Xtream) {
        let req_stream_id: u32 = match FromStr::from_str(stream_id.trim()) {
            Ok(id) => id,
            Err(_) => return HttpResponse::BadRequest().finish()
        };

        if let Ok(pli) = get_xtream_item_for_stream_id(req_stream_id, &app_state.config, target, None) {
            let input_id: u16 = pli.input_id;
            if let Some(input) = app_state.config.get_input_by_id(&input_id) {
                if let Some(action_url) = get_xtream_player_api_action_url(input, "get_short_epg") {
                    let mut info_url = format!("{}&stream_id={}", action_url, pli.id);
                    if !(limit.is_empty() || limit.eq("0")) {
                        info_url = format!("{}&limit={}", info_url, limit);
                    }
                    if let Ok(url) = Url::parse(&info_url) {
                        if user.proxy == ProxyType::Redirect {
                            return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                        }

                        let client = request_utils::get_client_request(Some(input), url, None);
                        if let Ok(response) = client.send().await {
                            if response.status().is_success() {
                                return match response.text().await {
                                    Ok(content) => {
                                        HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content)
                                    }
                                    Err(err) => {
                                        error!("Failed to download epg {}", err.to_string());
                                        HttpResponse::NoContent().finish()
                                    }
                                };
                            }
                        }
                    }
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
                return Some(serve_query(&file_path, &HashMap::from([("category_id", category_id)])).await);
            } else {
                return Some(serve_file(&file_path, req, mime::APPLICATION_JSON).await);
            }
        } else if let Some(payload) = content {
            return Some(HttpResponse::Ok().body(payload));
        } else {
            return Some(HttpResponse::NoContent().finish());
        }
    }
    None
}

async fn xtream_player_api(
    req: &HttpRequest,
    api_req: UserApiRequest,
    _app_state: &web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, _app_state) {
        Some((user, target)) => {
            let action = api_req.action.trim();
            let target_name = &target.name;
            if target.has_output(&TargetType::Xtream) {
                if action.is_empty() {
                    return HttpResponse::Ok().json(get_user_info(&user, &_app_state.config));
                }

                match action {
                    "get_series_info" => {
                        xtream_get_stream_info_response(_app_state, &user, target,
                                                        api_req.series_id.trim(),
                                                        &XtreamCluster::Series).await
                    }
                    "get_vod_info" => {
                        xtream_get_stream_info_response(_app_state, &user, target,
                                                        api_req.vod_id.trim(),
                                                        &XtreamCluster::Video).await
                    }
                    "get_epg" |
                    "get_short_epg" => {
                        xtream_get_short_epg(_app_state, &user, target,
                                             api_req.stream_id.trim(),
                                             api_req.limit.trim()).await
                    }
                    _ => {
                        let category_id = api_req.category_id.as_str().trim();
                        match xtream_player_api_handle_content_action(&_app_state.config, target_name, action, category_id, req).await {
                            Some(response) => response,
                            _ => {
                                let cat_id = if category_id.is_empty() { 0 } else { category_id.parse::<u32>().unwrap_or(0) };
                                match match action {
                                    "get_live_streams" => xtream_repository::load_rewrite_xtream_playlist(&XtreamCluster::Live, &_app_state.config, target, cat_id),
                                    "get_vod_streams" => xtream_repository::load_rewrite_xtream_playlist(&XtreamCluster::Video, &_app_state.config, target, cat_id),
                                    "get_series" => xtream_repository::load_rewrite_xtream_playlist(&XtreamCluster::Series, &_app_state.config, target, cat_id),
                                    _ => Err(Error::new(ErrorKind::Unsupported, format!("Cant find action: {}/{}", target_name, action))),
                                } {
                                    Ok(payload) => HttpResponse::Ok().body(payload),
                                    Err(err) => {
                                        debug!("Could not create response for xtream target action: {} {} {}", target_name, action, err);
                                        HttpResponse::NoContent().finish()
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                HttpResponse::Ok().json(get_user_info(&user, &_app_state.config))
            }
        }
        _ => {
            if api_req.action.is_empty() {
                debug!("Paremeter action is empty!");
                HttpResponse::Unauthorized().finish()
            } else {
                debug!("cant find user!");
                HttpResponse::BadRequest().finish()
            }
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
        .service(web::resource("/timeshift/{username}/{password}/{duration}/{start}{stream_id}").route(web::get().to(xtream_player_api_timeshift_stream)));
    /* TODO
    cfg.service(web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(xtream_player_api_hlsr_stream)));
    cfg.service(web::resource("/hls/{token}/{chunk}").route(web::get().to(xtream_player_api_hls_stream)));
    cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
     */
}