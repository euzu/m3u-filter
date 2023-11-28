use actix_web::{HttpRequest, HttpResponse, Resource, web};

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::repository::m3u_repository::get_m3u_file_path;


async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, &_app_state) {
        Some((_, target)) => {
            let filename = target.get_m3u_filename();
            if filename.is_some() {
                if let Some(file_path) = get_m3u_file_path(&_app_state.config, &filename) {
                    return serve_file(&file_path, &req).await;
                }
            }
            HttpResponse::NoContent().finish()
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}

pub(crate) fn m3u_api_register() -> Vec<Resource> {
    vec![
        web::resource("/get.php").route(web::get().to(m3u_api)),
        web::resource("/get.php").route(web::post().to(m3u_api)),
        web::resource("/apiget").route(web::get().to(m3u_api)),
        web::resource("/m3u").route(web::get().to(m3u_api))
    ]
}