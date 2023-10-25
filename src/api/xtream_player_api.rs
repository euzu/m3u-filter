// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use actix_web::{get, HttpRequest, HttpResponse, web};
use chrono::{Duration, Local};

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::model::api_proxy::{UserCredentials};
use crate::model::config::Config;
use crate::model::model_config::{TargetType};
use crate::repository::xtream_repository::{COL_CAT_LIVE, COL_CAT_SERIES, COL_CAT_VOD, COL_LIVE, COL_SERIES, COL_VOD, xtream_get_all};

fn get_user_info(user: &UserCredentials, cfg: &Config) -> XtreamAuthorizationResponse {
    let server = &cfg._api_proxy.as_ref().unwrap().server;
    let now = Local::now();
    XtreamAuthorizationResponse {
        user_info: XtreamUserInfo {
            active_cons: 0,
            allowed_output_formats: Vec::from(["ts".to_string()]),
            auth: 1,
            created_at: (now - Duration::days(365)).timestamp(), // fake
            exp_date: (now + Duration::days(365)).timestamp(),// fake
            is_trial: 0,
            max_connections: 1,
            message: server.message.to_string(),
            password: user.password.to_string(),
            username: user.username.to_string(),
            status: "Active".to_string(),
        },
        server_info: XtreamServerInfo {
            url: server.ip.to_string(),
            port: server.http_port,
            https_port: server.https_port,
            server_protocol: server.protocol.clone(),
            rtmp_port: server.rtmp_port,
            timezone: server.timezone.to_string(),
            timestamp_now: now.timestamp(),
            time_now: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    }
}


#[get("/player_api.php")]
pub(crate) async fn xtream_player_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, &_app_state) {
        Some((user, target)) => {
            let target_name = &target.name;
            if target.has_output(&TargetType::Xtream) {
                if api_req.action.is_empty() {
                    return HttpResponse::Ok().json(get_user_info(user, &_app_state.config));
                }
                match match api_req.action.as_str() {
                    "get_live_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_LIVE),
                    "get_vod_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_VOD),
                    "get_series_categories" => xtream_get_all(&_app_state.config, target_name, COL_CAT_SERIES),
                    "get_live_streams" => xtream_get_all(&_app_state.config, target_name, COL_LIVE),
                    "get_vod_streams" => xtream_get_all(&_app_state.config, target_name, COL_VOD),
                    "get_series" => xtream_get_all(&_app_state.config, target_name, COL_SERIES),
                    _ => Err(std::io::Error::new(std::io::ErrorKind::Unsupported, format!("Cant find action: {}/{}", target_name, &api_req.action))),
                } {
                    Ok(file_path) => {
                        serve_file(&file_path, &req).await
                    }
                    Err(_) => HttpResponse::BadRequest().finish()
                }
            } else {
                HttpResponse::BadRequest().finish()
            }
        }
        _ => {
            if api_req.action.is_empty() {
                HttpResponse::Unauthorized().finish()
            } else {
                HttpResponse::BadRequest().finish()
            }
        }
    }
}
