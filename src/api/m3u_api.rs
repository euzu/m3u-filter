use actix_web::{HttpRequest, HttpResponse, web};
use log::error;
use futures::{stream};
use bytes::Bytes;

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, stream_response};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::config::TargetType;
use crate::repository::m3u_repository::{m3u_get_file_paths, m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};
use crate::repository::storage::get_target_storage_path;

async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    //_api_req: web::Query<HashMap<String, String>>,
    _req: HttpRequest,
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
                    error!("{err}");
                    HttpResponse::NoContent().finish()
                }
            }
        }
        None => HttpResponse::BadRequest().finish(),
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
                match get_target_storage_path(&app_state.config, target.name.as_str()) {
                    Some(target_path) => {
                        let (m3u_path, idx_path) = m3u_get_file_paths(&target_path);
                        match m3u_get_item_for_stream_id(&app_state.config, m3u_stream_id, &m3u_path, &idx_path) {
                            Ok(m3u_item) => {
                                return stream_response(m3u_item.url.as_str(), &req, None).await;
                            }
                            Err(err) => {
                                error!("Failed to get m3u url: {}", err);
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

pub(crate) fn m3u_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/get.php").route(web::get().to(m3u_api)))
        .service(web::resource("/get.php").route(web::post().to(m3u_api)))
        .service(web::resource("/apiget").route(web::get().to(m3u_api)))
        .service(web::resource("/m3u").route(web::get().to(m3u_api)))
        .service(web::resource("/m3u-stream/{username}/{password}/{stream_id}").route(web::get().to(m3u_api_stream)));
}