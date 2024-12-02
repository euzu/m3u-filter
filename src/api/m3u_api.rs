use actix_web::{HttpRequest, HttpResponse, web};
use log::error;
use futures::{stream};
use bytes::Bytes;

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, stream_response};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::{m3u_get_file_paths, m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};
use crate::repository::storage::get_target_storage_path;
use crate::utils::request_utils::mask_sensitive_info;

async fn m3u_api(
    api_req: UserApiRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, &app_state) {
        Some((user, target)) => {
            match m3u_load_rewrite_playlist(&app_state.config, target, &user) {
                Ok(m3u_iter) => {
                    // Convert the iterator into a stream of `Bytes`
                    let content_stream = stream::iter(m3u_iter.map(|line| Ok::<Bytes, String>(Bytes::from(format!("{line}\n")))));
                    HttpResponse::Ok()
                        .content_type(mime::TEXT_PLAIN_UTF_8)
                        .streaming(content_stream)
                }
                Err(err) => {
                    error!("{}", mask_sensitive_info(err.to_string().as_str()));
                    HttpResponse::NoContent().finish()
                }
            }
        }
        None => HttpResponse::BadRequest().finish(),
    }
}

async fn m3u_api_get(    api_req: web::Query<UserApiRequest>,
                         app_state: web::Data<AppState>,
) -> HttpResponse {
    m3u_api(api_req.into_inner(), app_state).await
}
async fn m3u_api_post(
    api_req: web::Form<UserApiRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    m3u_api(api_req.into_inner(), app_state).await
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
                match get_target_storage_path(&app_state.config, target.name.as_str()) {
                    Some(target_path) => {
                        let (m3u_path, idx_path) = m3u_get_file_paths(&target_path);
                        match m3u_get_item_for_stream_id(&app_state.config, m3u_stream_id, &m3u_path, &idx_path) {
                            Ok(m3u_item) => {
                                return stream_response(m3u_item.url.as_str(), &req, None).await;
                            }
                            Err(err) => {
                                error!("Failed to get m3u url: {}", mask_sensitive_info(err.to_string().as_str()));
                            }
                        }
                    }
                    None => {
                        error!("Failed to get target path for {}", target.name);
                    }
                }
            }
        }
    }
    HttpResponse::BadRequest().finish()
}

pub fn m3u_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/get.php").route(web::get().to(m3u_api_get)).route(web::post().to(m3u_api_post)))
        .service(web::resource("/get.php").route(web::post().to(m3u_api_get)).route(web::post().to(m3u_api_post)))
        .service(web::resource("/apiget").route(web::get().to(m3u_api_get)).route(web::post().to(m3u_api_post)))
        .service(web::resource("/m3u").route(web::get().to(m3u_api_get)).route(web::post().to(m3u_api_post)))
        .service(web::resource("/m3u-stream/live/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)))
        .service(web::resource("/m3u-stream/movie/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)))
        .service(web::resource("/m3u-stream/series/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)))
        .service(web::resource("/m3u-stream/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)));
}