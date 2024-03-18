// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use std::collections::HashMap;
use std::io::{Error};
use std::str::FromStr;
use actix_web::{HttpRequest, HttpResponse, web, Resource};
use chrono::{Duration, Local};
use log::{debug, error};
use url::{Url};

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, serve_file};
use crate::api::api_model::{AppState, UserApiRequest, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::model::api_proxy::{ProxyType, UserCredentials};
use crate::model::config::{Config, ConfigInput, InputType};
use crate::model::model_config::{TargetType};
use crate::model::model_playlist::XtreamCluster;
use crate::repository::xtream_repository::{COL_CAT_LIVE, COL_CAT_SERIES, COL_CAT_VOD, COL_LIVE, COL_SERIES, COL_VOD, read_xtream_mapping, xtream_get_all, xtream_get_stored_stream_info, xtream_persist_stream_info};
use crate::utils::{get_client_request};

fn get_xtream_player_api_action_url(input: &ConfigInput, action: &str) -> Option<String> {
    match input.input_type {
        InputType::M3u => None,
        InputType::Xtream => Some(
            format!("{}/player_api.php?username={}&password={}&action={}",
                    input.url.as_str(),
                    input.username.as_ref().unwrap_or(&"".to_string()).as_str(),
                    input.password.as_ref().unwrap_or(&"".to_string()).as_str(),
                    action
            ))
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
    match input.input_type {
        InputType::M3u => None,
        InputType::Xtream => Some(format!("{}/{}{}/{}/{}",
                                          input.url.as_str(),
                                          ctx_path,
                                          input.username.as_ref().unwrap_or(&"".to_string()).as_str(),
                                          input.password.as_ref().unwrap_or(&"".to_string()).as_str(),
                                          action_path
        ))
    }
}


fn get_user_info(user: &UserCredentials, cfg: &Config) -> XtreamAuthorizationResponse {
    let server_info_list = cfg._api_proxy.read().unwrap().as_ref().unwrap().server.clone();
    let server_info_name = match &user.server {
        Some(server_name) => server_name.as_str(),
        None => "default"
    };
    let server_info = match server_info_list.iter().find(|c| c.name.eq(server_info_name)) {
        Some(info) => info,
        None => server_info_list.first().unwrap(),
    };

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
            if let Ok(num) = action_path.trim().parse() {
                let (xtream_id, cfg_input) = get_xtream_input_for_stream_id(_app_state, target_name, num);
                if cfg_input.is_some() {
                    input = cfg_input;
                    stream_id = xtream_id.to_string();
                }
            }

            if input.is_none() {
                if let Some(m3u_inputs) = _app_state.config.get_input_for_target(target_name, &InputType::M3u) {
                    input = m3u_inputs.first().cloned();
                }
            }

            if let Some(target_input) = input {
                if let Some(stream_url) = get_xtream_player_api_stream_url(target_input, context, stream_id.as_str()) {
                    if user.proxy == ProxyType::Redirect {
                        debug!("Redirecting stream request to {}", stream_url);
                        return HttpResponse::Found().insert_header(("Location", stream_url)).finish();
                    }

                    let req_headers: HashMap<&str, &[u8]> = req.headers().iter().map(|(k, v)| (k.as_str(), v.as_bytes())).collect();
                    debug!("Try to open stream {}", &stream_url);
                    if let Ok(url) = Url::parse(&stream_url) {
                        let client = get_client_request(target_input, url, Some(&req_headers));
                        match client.send().await {
                            Ok(response) => {
                                if response.status().is_success() {
                                    let mut response_builder = HttpResponse::Ok();
                                    response.headers().iter().for_each(|(k, v)| {
                                        response_builder.insert_header((k, v));
                                    });
                                    return response_builder.body(actix_web::body::BodyStream::new(response.bytes_stream()));
                                } else {
                                    debug!("Failed to open stream got status {} for {}", response.status(), &stream_url)
                                }
                            }
                            Err(err) => {
                                error!("Received failure from server {}:  {}", &stream_url, err)
                            }
                        }
                    } else {
                        error!("Url is malformed {}", &stream_url)
                    }
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


fn get_xtream_input_for_stream_id<'a>(app_state: &'a AppState, target_name: &str, stream_id: i32) -> (i32, Option<&'a ConfigInput>) {
    if let Some(inputs) = app_state.config.get_input_for_target(target_name, &InputType::Xtream) {
        if let Ok(Some(mapping)) = read_xtream_mapping(stream_id as u32, app_state.config.as_ref(), target_name) {
            if let Some(cfg_input) = inputs.iter().find(|&&inp| inp.id == mapping.input_id).cloned() {
                return (mapping.stream_id as i32, Some(cfg_input));
            }
        }
        return (stream_id, inputs.first().cloned());
    }
    (stream_id, None)
}

// TODO use u32 or i32 ??
async fn xtream_get_stream_info(app_state: &AppState, target_name: &str, stream_id: i32,
                                cluster: &XtreamCluster) -> Result<String, Error> {
    let (xtream_id, input) = get_xtream_input_for_stream_id(app_state, target_name, stream_id);
    if let Some(target_input) = input {
        if let Ok(content) = xtream_get_stored_stream_info(app_state, target_name, xtream_id, cluster, target_input).await {
            return Ok(content);
        }

        if let Some(info_url) = get_xtream_player_api_info_url(target_input, cluster, xtream_id) {
            if let Ok(url) = Url::parse(&info_url) {
                let client = get_client_request(target_input, url, None);
                if let Ok(response) = client.send().await {
                    debug!("{}", response.status());
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(content) => {
                                // TODO we are not replacing direct_source, we should add an option to do this.
                                xtream_persist_stream_info(app_state, target_name, xtream_id, cluster,
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

async fn xtream_get_stream_info_response(app_state: &AppState, user: &UserCredentials,
                                         target_name: &str, stream_id: &str,
                                         cluster: &XtreamCluster) -> HttpResponse {
    match FromStr::from_str(stream_id) {
        Ok(xtream_stream_id) => {
            if user.proxy == ProxyType::Redirect {
                let (xtream_id, input) = get_xtream_input_for_stream_id(app_state, target_name, xtream_stream_id);
                if let Some(target_input) = input {
                    if let Some(info_url) = get_xtream_player_api_info_url(target_input, cluster, xtream_id) {
                        return HttpResponse::Found().insert_header(("Location", info_url)).finish();
                    }
                }
            }

            match xtream_get_stream_info(app_state, target_name, xtream_stream_id, cluster).await {
                Ok(content) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content),
                Err(_) => HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{info:[]}"),
            }
        }
        Err(_) => HttpResponse::BadRequest().finish()
    }
}

async fn xtream_get_short_epg(app_state: &AppState, user: &UserCredentials, target_name: &str, stream_id: &str, limit: &str) -> HttpResponse {
    let xtream_stream_id: i32 = match FromStr::from_str(stream_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish()
    };

    let (xtream_id, input) = get_xtream_input_for_stream_id(app_state, target_name, xtream_stream_id);
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

                let client = get_client_request(target_input, url, None);
                if let Ok(response) = client.send().await {
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(content) => {
                                return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(content);
                            }
                            Err(err) => {
                                error!("Failed to download epg {}", err.to_string());
                                return HttpResponse::NoContent().finish();
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
                        xtream_get_stream_info_response(_app_state, &user, target_name,
                                                        api_req.series_id.trim(),
                                                        &XtreamCluster::Series).await
                    }
                    "get_vod_info" => {
                        xtream_get_stream_info_response(_app_state, &user, target_name,
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
                            "get_live_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_LIVE),
                            "get_vod_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_VOD),
                            "get_series_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_SERIES),
                            "get_live_streams" => xtream_get_all(&_app_state.config, target_name, COL_LIVE),
                            "get_vod_streams" => xtream_get_all(&_app_state.config, target_name, COL_VOD),
                            "get_series" => xtream_get_all(&_app_state.config, target_name, COL_SERIES),
                            _ => Err(Error::new(std::io::ErrorKind::Unsupported, format!("Cant find action: {}/{}", target_name, action))),
                        } {
                            Ok((path, content)) => {
                                if let Some(file_path) = path {
                                    serve_file(&file_path, req).await
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

pub(crate) fn xtream_api_register() -> Vec<Resource> {
    vec![
        web::resource("/player_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)),
        web::resource("/panel_api.php").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_get)),
        web::resource("/xtream").route(web::get().to(xtream_player_api_get)).route(web::post().to(xtream_player_api_post)),
        web::resource("/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream_alt)),
        web::resource("/live/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_live_stream)),
        web::resource("/movie/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_movie_stream)),
        web::resource("/series/{username}/{password}/{stream_id}").route(web::get().to(xtream_player_api_series_stream)),
        web::resource("/timeshift/{username}/{password}/{duration}/{start}{stream_id}").route(web::get().to(xtream_player_api_timeshift_stream)),
        /* TODO
        web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(xtream_player_api_hlsr_stream))
        web::resource("/hls/{token}/{chunk}").route(web::get().to(xtream_player_api_hls_stream))
        web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream))
         */
    ]
}