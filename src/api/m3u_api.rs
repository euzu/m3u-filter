use actix_web::{get, HttpRequest, HttpResponse, web};

use crate::api::api_utils::serve_file;
use crate::api::model_api::AppState;
use crate::repository::m3u_repository::get_m3u_file_path;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct M3uApiRequest {
    username: String,
    password: String,
}

#[get("/get.php")]
pub(crate) async fn m3u_api(
    api_req: web::Query<M3uApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match _app_state.config.get_target_for_user(api_req.username.as_str(), api_req.password.as_str()) {
        Some(target) => {
            let filename = target.get_m3u_filename();
            match get_m3u_file_path(&_app_state.config, &filename) {
                Some(file_path) => {
                    serve_file(&file_path, &req).await
                }
                None => HttpResponse::BadRequest().finish()
            }
        }
        None => {
            HttpResponse::BadRequest().finish()
        }
    }
}