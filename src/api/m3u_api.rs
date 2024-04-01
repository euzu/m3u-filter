use actix_web::{HttpRequest, HttpResponse, Resource, web};

use crate::api::api_utils::{get_user_target, serve_file};
use crate::api::api_model::{AppState, UserApiRequest};
use crate::model::api_proxy::ProxyType;
use crate::repository::m3u_repository::{get_m3u_file_paths, rewrite_m3u_playlist};

async fn m3u_api(
    api_req: web::Query<UserApiRequest>,
    req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
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

async fn m3u_stream_api(
    _api_req: web::Query<UserApiRequest>,
    _req: HttpRequest,
    _app_state: web::Data<AppState>,
) -> HttpResponse {
    HttpResponse::BadRequest().finish()
}

pub(crate) fn m3u_api_register() -> Vec<Resource> {
    vec![
        web::resource("/get.php").route(web::get().to(m3u_api)),
        web::resource("/get.php").route(web::post().to(m3u_api)),
        web::resource("/apiget").route(web::get().to(m3u_api)),
        web::resource("/m3u").route(web::get().to(m3u_api)),
        web::resource("/m3u-stream").route(web::get().to(m3u_stream_api))
    ]
}