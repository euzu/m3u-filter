// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use std::collections::HashMap;
use std::io::{Error};
use std::path::Path;
use std::str::FromStr;
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Local};
use log::{debug, error};
use url::Url;

use crate::api::api_utils::{get_user_server_info, get_user_target, get_user_target_by_credentials, serve_file, stream_response};
use crate::api::api_model::{AppState, UserApiRequest, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::config::{TargetType};
use crate::model::playlist::XtreamCluster;
use crate::repository::xtream_repository;
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

fn get_xtream_player_api_info_url(input: &ConfigInput, cluster: &XtreamCluster, stream_id: i32) -> Option<String> {
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

fn separate_number_and_rest(input: &str) -> (String, String) {
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
    _app_state: &web::Data<AppState>,
    context: &str,
    username: &str,
    password: &str,
    action_path: &str,
) -> HttpResponse {
    if let Some((user, target)) = get_user_target_by_credentials(username, password, api_req, _app_state) {
        let target_name = &target.name;
        if target.has_output(&TargetType::Xtream) {
            let mut stream_id = action_path.to_owned();
            let mut input: Option<&ConfigInput> = None;
            if target.is_multi_input() {
                let (action_stream_id, action_ext) = separate_number_and_rest(action_path);
                if let Ok(num) = action_stream_id.trim().parse() {
                    let (xtream_id, cfg_input) = get_xtream_mapped_id_and_input_for_stream_id(_app_state, target_name, num);
                    if cfg_input.is_some() {
                        input = cfg_input;
                        stream_id = format!("{}{}", xtream_id, action_ext);
                    }
                }
            } else if let Some(inputs) = _app_state.config.get_inputs_for_target(target_name) {
                input = inputs.first().copied();
            }

            if let Some(target_input) = input {
                if let Some(stream_url) = get_xtream_player_api_stream_url(target_input, context, stream_id.as_str()) {
                    if user.proxy == ProxyType::Redirect {
                        debug!("Redirecting stream request to {}", stream_url);
                        return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
                    }
                    return stream_response(&stream_url, req, Some(target_input)).await
                } else {
                    debug!("Cant figure out stream url for target {}, context {}, action {}",
                        target_name, context, action_path);
                }
            } else {
                debug!("Cant find input definition for target {}", target_name);
            }
        } else {
            debug!("Target has no xtream output {}", target_name);
        }
    } else {
        debug!("Could not find any user {}", username);
    }
    HttpResponse::BadRequest().finish()
}

async fn xtream_player_api_live_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &_app_state, "live", &username, &password, &stream_id).await
}

async fn xtream_player_api_live_stream_alt(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &_app_state, "", &username, &password, &stream_id).await
}

async fn xtream_player_api_series_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &_app_state, "series", &username, &password, &stream_id).await
}

async fn xtream_player_api_movie_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    xtream_player_api_stream(&req, &api_req, &_app_state, "movie", &username, &password, &stream_id).await
}

async fn xtream_player_api_timeshift_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String)>,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, duration, start, stream_id) = path.into_inner();
    let action_path = format!("{}/{}/{}", duration, start, stream_id);
    xtream_player_api_stream(&req, &api_req, &_app_state, "timeshift", &username, &password, &action_path).await
}

fn get_xtream_mapped_id_and_input_for_stream_id<'a>(app_state: &'a AppState, target_name: &str, stream_id: i32) -> (i32, Option<&'a ConfigInput>) {
    if let Some(inputs) = app_state.config.get_inputs_for_target(target_name) {
        if let Ok(Some(mapping)) = xtream_repository::read_xtream_mapping(stream_id as u32, app_state.config.as_ref(), target_name) {
            if let Some(cfg_input) = inputs.iter().find(|&&inp| inp.id == mapping.input_id).cloned() {
                return (mapping.stream_id as i32, Some(cfg_input));
            }
        }
    }
    (stream_id, None)
}

async fn xtream_get_stream_info(app_state: &AppState, target_name: &str, stream_id: i32,
                                cluster: &XtreamCluster) -> Result<String, Error> {
    let (xtream_id, input) = get_xtream_mapped_id_and_input_for_stream_id(app_state, target_name, stream_id);
    if let Some(target_input) = input {
        if let Ok(content) = xtream_repository::xtream_get_stored_stream_info(app_state, target_name, stream_id, cluster, target_input).await {
            return Ok(content);
        }

        if let Some(info_url) = get_xtream_player_api_info_url(target_input, cluster, xtream_id) {
            if let Ok(url) = Url::parse(&info_url) {
                let client = request_utils::get_client_request(Some(target_input), url, None);
                if let Ok(response) = client.send().await {
                    debug!("{}", response.status());
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(content) => {
                                // TODO we are not replacing direct_source, we should add an option to do this.
                                xtream_repository::xtream_persist_stream_info(app_state, target_name, stream_id, cluster,
                                                                              target_input, content.as_str()).await;
                                return Ok(content);
                            }
                            Err(err) => { error!("Failed to download info {}", err.to_string()); }
                        }
                    }
                }
            }
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find stream with id: {}/{}/{}", target_name, &cluster, stream_id)))
}

async fn xtream_get_stream_info_response(app_state: &AppState, user: &ProxyUserCredentials,
                                         target: &ConfigTarget, stream_id: &str,
                                         cluster: &XtreamCluster) -> HttpResponse {
    let req_stream_id: i32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    if user.proxy == ProxyType::Redirect && !target.is_multi_input() {
        if let Some(inputs) = app_state.config.get_inputs_for_target(&target.name) {
            if let Some(&input) = inputs.first() {
                if let Some(info_url) = get_xtream_player_api_info_url(input, cluster, req_stream_id) {
                    return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                }
            }
        }
        return HttpResponse::BadRequest().finish();
    }

    match xtream_get_stream_info(app_state, &target.name, req_stream_id, cluster).await {
        Ok(content) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content),
        Err(_) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{info:[]}"),
    }
}

async fn xtream_get_short_epg(app_state: &AppState, user: &ProxyUserCredentials, target_name: &str, stream_id: &str, limit: &str) -> HttpResponse {
    let xtream_stream_id: i32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    let (xtream_id, input) = get_xtream_mapped_id_and_input_for_stream_id(app_state, target_name, xtream_stream_id);
    if let Some(target_input) = input {
        if let Some(action_url) = get_xtream_player_api_action_url(target_input, "get_short_epg") {
            let mut info_url = format!("{}&stream_id={}", action_url, xtream_id);
            if !(limit.is_empty() || limit.eq("0")) {
                info_url = format!("{}&limit={}", info_url, limit);
            }
            if let Ok(url) = Url::parse(&info_url) {
                if user.proxy == ProxyType::Redirect {
                    return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                }

                let client = request_utils::get_client_request(Some(target_input), url, None);
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
    error!("Cant find short epg with id: {}/{}", target_name, stream_id);
    HttpResponse::NoContent().finish()
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
                        xtream_get_short_epg(_app_state, &user, target_name,
                                             api_req.stream_id.trim(),
                                             api_req.limit.trim()).await
                    }
                    _ => {
                        match match action {
                            "get_live_categories" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_CAT_LIVE),
                            "get_vod_categories" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_CAT_VOD),
                            "get_series_categories" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_CAT_SERIES),
                            "get_live_streams" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_LIVE),
                            "get_vod_streams" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_VOD),
                            "get_series" => xtream_repository::xtream_get_collection_path(&_app_state.config, target_name, xtream_repository::COL_SERIES),
                            _ => Err(Error::new(std::io::ErrorKind::Unsupported, format!("Cant find action: {}/{}", target_name, action))),
                        } {
                            Ok((path, content)) => {
                                if let Some(file_path) = path {
                                    let category_id = api_req.category_id.trim();
                                    if !category_id.is_empty() {
                                        serve_query(&file_path, &HashMap::from([("category_id", category_id)])).await
                                    } else {
                                        serve_file(&file_path, req, mime::APPLICATION_JSON).await
                                    }
                                } else if let Some(payload) = content {
                                    HttpResponse::Ok().body(payload)
                                } else {
                                    HttpResponse::NoContent().finish()
                                }
                            }
                            Err(err) => {
                                debug!("Could not open file for xtream target: {} {}", target_name, err);
                                HttpResponse::NoContent().finish()
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
                               _app_state: web::Data<AppState>,
) -> HttpResponse {
    xtream_player_api(&req, api_req.into_inner(), &_app_state).await
}

async fn xtream_player_api_post(req: HttpRequest,
                                api_req: web::Form<UserApiRequest>,
                                _app_state: web::Data<AppState>,
) -> HttpResponse {
    xtream_player_api(&req, api_req.into_inner(), &_app_state).await
}

pub(crate) fn xtream_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/player_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)));
    cfg.service(web::resource("/panel_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)));
    cfg.service(web::resource("/xtream").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)));
    cfg.service(web::resource("/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream_alt)));
    cfg.service(web::resource("/live/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream)));
    cfg.service(web::resource("/movie/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_movie_stream)));
    cfg.service(web::resource("/series/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_series_stream)));
    cfg.service(web::resource("/timeshift/{username}/{password}/{duration}/{start}{stream_id}").route(web::get().to(xtream_player_api_timeshift_stream)));
    /* TODO
    cfg.service(web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(xtream_player_api_hlsr_stream)));
    cfg.service(web::resource("/hls/{token}/{chunk}").route(web::get().to(xtream_player_api_hls_stream)));
    cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
     */
}