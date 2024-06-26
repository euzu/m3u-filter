use actix_web::{HttpRequest, HttpResponse, web};
use log::error;

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, stream_response};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::{m3u_get_file_paths, m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};

async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    //_api_req: web::Query<HashMap<String, String>>,
    _req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    //let api_req = UserApiRequest::from_map(&_api_req);
    match get_user_target(&api_req, &app_state) {
        Some((user, target)) => {
            // let filename = target.get_m3u_filename();
            if let Some(content) = m3u_load_rewrite_playlist(&app_state.config, target, &user) {
                HttpResponse::Ok().content_type(mime::TEXT_PLAIN_UTF_8).body(content)
            } else {
                HttpResponse::NoContent().finish()
            }
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}

async fn m3u_api_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    if let Ok(m3u_stream_id) = stream_id.parse::<u32>() {
        if let Some((_user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state) {
            if target.has_output(&TargetType::M3u) {
                if let Some((m3u_path, idx_path)) = m3u_get_file_paths(&app_state.config, target) {
                    match m3u_get_item_for_stream_id(m3u_stream_id, &m3u_path, &idx_path) {
                        Ok(m3u_item) => {
                            return stream_response(m3u_item.url.as_str(), &req, None).await;
                        }
                        Err(err) => {
                            error!("Failed to get m3u url: {}", err);
                        }
                    }
                }
            }
        }
    }
    HttpResponse::BadRequest().finish()
}

pub(crate) fn m3u_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/get.php").route(web::get().to(m3u_api)))
        .service(web::resource("/get.php").route(web::post().to(m3u_api)))
        .service(web::resource("/apiget").route(web::get().to(m3u_api)))
        .service(web::resource("/m3u").route(web::get().to(m3u_api)))
        .service(web::resource("/m3u-stream/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)));
}