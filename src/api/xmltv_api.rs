use std::path::{PathBuf};
use actix_web::{get, HttpRequest, HttpResponse, web};

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::config::{Config, ConfigTarget};
use crate::model::model_config::TargetType;
use crate::repository::m3u_repository::get_m3u_epg_file_path;
use crate::repository::xtream_repository::{get_xtream_epg_file_path, get_xtream_storage_path};
use crate::utils::path_exists;


fn get_epg_path_for_target(config: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    for output in &target.output {
        match output.target {
            TargetType::M3u => {
                if let Some(epg_path) = get_m3u_epg_file_path(config, &target.get_m3u_filename()) {
                    if path_exists(&epg_path) {
                        return Some(epg_path);
                    }
                }
            }
            TargetType::Xtream => {
                if let Some(storage_path) = get_xtream_storage_path(config, &target.name) {
                    let epg_path=  get_xtream_epg_file_path(&storage_path);
                    if path_exists(&epg_path) {
                        return Some(epg_path);
                    }
                }
            }
            TargetType::Strm => {}
        }
    }
    None
}

#[get("/xmltv.php")]
pub(crate) async fn xmltv_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    match get_user_target(&api_req, &_app_state) {
        Some((_, target)) => {
            match get_epg_path_for_target(&_app_state.config, target) {
                None => HttpResponse::BadRequest().finish(),
                Some(epg_path) => serve_file(&epg_path, &req).await
            }
        }
        None => HttpResponse::BadRequest().finish()
    }
}