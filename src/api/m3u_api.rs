use actix_web::{get, HttpRequest, HttpResponse, web};

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::repository::m3u_repository::get_m3u_file_path;

#[get("/get.php")]
pub(crate) async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, &_app_state) {
        Some((_, target)) => {
            let filename = target.get_m3u_filename();
            match get_m3u_file_path(&_app_state.config, &filename) {
                Some(file_path) => {
                    serve_file(&file_path, &req).await
                }
                None => HttpResponse::NoContent().finish()
            }
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}