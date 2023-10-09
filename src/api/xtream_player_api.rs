// https://github.com/tellytv/go.xtream-codes/blob/master/structs.go

use actix_web::{HttpResponse, web, get, HttpRequest};
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use chrono::{Duration, Local};
use crate::api::model_api::{AppState, XtreamAuthorizationResponse, XtreamServerInfo, XtreamUserInfo};
use crate::model::config::Config;
use crate::model::model_config::{default_as_empty_str};
use crate::repository::xtream_repository::{COL_CAT_LIVE, COL_CAT_SERIES, COL_CAT_VOD, COL_LIVE, COL_SERIES, COL_VOD, xtream_get_all};

fn get_user_info(user_name: &str, cfg: &Config) -> XtreamAuthorizationResponse {
    let server = cfg._api_proxy.as_ref().unwrap().server.clone();
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
            password: "dfdfdf".to_string(),
            username: user_name.to_string(),
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct XtreamApiRequest {
    username: String,
    password: String,
    #[serde(default = "default_as_empty_str")]
    action: String,
}

#[get("/player_api.php")]
pub(crate) async fn xtream_player_api(
    api_req: web::Query<XtreamApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match _app_state.config.get_target_for_user(api_req.username.as_str(), api_req.password.as_str()) {
        Some(target_name) => {
            let target = target_name.as_str();
            if api_req.action.is_empty() {
                return HttpResponse::Ok().json(get_user_info(api_req.username.as_str(), &_app_state.config));
            }
            match match api_req.action.as_str() {
                "get_live_categories" => xtream_get_all(&_app_state.config, target, COL_CAT_LIVE),
                "get_vod_categories" => xtream_get_all(&_app_state.config, target, COL_CAT_VOD),
                "get_series_categories" => xtream_get_all(&_app_state.config, target, COL_CAT_SERIES),
                "get_live_streams" => xtream_get_all(&_app_state.config, target, COL_LIVE),
                "get_vod_streams" => xtream_get_all(&_app_state.config, target, COL_VOD),
                "get_series" => xtream_get_all(&_app_state.config, target, COL_SERIES),
                _ => Err(std::io::Error::new(std::io::ErrorKind::Unsupported, format!("Cant find action: {}/{}", target, &api_req.action))),
            } {
                Ok(file_path) => {
                    let file = actix_files::NamedFile::open_async(file_path).await.unwrap()
                        .set_content_type(mime::APPLICATION_JSON)
                        .disable_content_disposition();
                    let mut result = file.into_response(&req);
                    let headers = result.headers_mut();
                    headers.insert(CACHE_CONTROL, HeaderValue::from_bytes("no-cache".as_bytes()).unwrap());
                    result
                }
                Err(_) => HttpResponse::BadRequest().finish()
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