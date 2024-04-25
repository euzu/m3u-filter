use actix_web::{HttpRequest, HttpResponse, Resource, web};
use log::error;

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, serve_file, stream_response};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::api_proxy::ProxyType;
use crate::model::config::TargetType;
use crate::repository::m3u_repository::{get_m3u_file_paths, get_m3u_url_for_stream_id, rewrite_m3u_playlist};

async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    //_api_req: web::Query<HashMap<String, String>>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    //let api_req = UserApiRequest::from_map(&_api_req);
    match get_user_target(&api_req, &app_state) {
        Some((user, target)) => {
            let filename = target.get_m3u_filename();
            if filename.is_some() {
                if let Some((m3u_path, _url_path, _idx_path)) = get_m3u_file_paths(&app_state.config, &filename) {
                    if user.proxy == ProxyType::Reverse {
                        if let Some(content) = rewrite_m3u_playlist(&app_state.config, target, &user) {
                            return HttpResponse::Ok().content_type(mime::TEXT_PLAIN_UTF_8).body(content);
                        }
                        HttpResponse::NoContent().finish();
                    } else {
                        return serve_file(&m3u_path, &req, mime::TEXT_PLAIN_UTF_8).await;
                    }
                }
            }
            HttpResponse::NoContent().finish()
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
                let filename = target.get_m3u_filename();
                if filename.is_some() {
                    if let Some((_m3u_path, url_path, idx_path)) = get_m3u_file_paths(&app_state.config, &filename) {
                        match get_m3u_url_for_stream_id(m3u_stream_id, &url_path, &idx_path) {
                            Ok(stream_url) => {
                                return stream_response(&stream_url, &req, None).await
                            }
                            Err(err) => {
                                error!("Failed to get m3u url: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }
    HttpResponse::BadRequest().finish()
}

pub(crate) fn m3u_api_register() -> Vec<Resource> {
    vec![
        web::resource("/get.php").route(web::get().to(m3u_api)),
        web::resource("/get.php").route(web::post().to(m3u_api)),
        web::resource("/apiget").route(web::get().to(m3u_api)),
        web::resource("/m3u").route(web::get().to(m3u_api)),
        web::resource("/m3u-stream/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream))
    ]
}