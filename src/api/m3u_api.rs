use actix_web::{web, HttpRequest, HttpResponse};
use bytes::Bytes;
use futures::stream;
use log::{debug, error};

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, is_stream_share_enabled, stream_response};
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::model::api_proxy::ProxyType;
use crate::model::config::TargetType;
use crate::repository::m3u_playlist_iterator::M3U_STREAM_PATH;
use crate::repository::m3u_repository::{m3u_get_file_paths, m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};
use crate::repository::storage::get_target_storage_path;
use crate::utils::request_utils::mask_sensitive_info;

async fn m3u_api(
    api_req: &UserApiRequest,
    app_state: &AppState,
) -> HttpResponse {
    match get_user_target(api_req, app_state) {
        Some((user, target)) => {
            match m3u_load_rewrite_playlist(&app_state.config, target, &user).await {
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

async fn m3u_api_get(api_req: web::Query<UserApiRequest>,
                     app_state: web::Data<AppState>,
) -> HttpResponse {
    m3u_api(&api_req.into_inner(), &app_state).await
}
async fn m3u_api_post(
    api_req: web::Form<UserApiRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    m3u_api(&api_req.into_inner(), &app_state).await
}

async fn m3u_api_stream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id) = path.into_inner();
    let Ok(m3u_stream_id) = stream_id.parse::<u32>() else { return HttpResponse::BadRequest().finish() };
    let Some((user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state) else { return HttpResponse::BadRequest().finish() };

    if !target.has_output(&TargetType::M3u) {
        return HttpResponse::BadRequest().finish();
    }

    let Some(target_path) = get_target_storage_path(&app_state.config, target.name.as_str()) else {
        error!("Failed to get target path for {}", target.name);
        return HttpResponse::BadRequest().finish();
    };

    let (m3u_path, idx_path) = m3u_get_file_paths(&target_path);
    let m3u_item = match m3u_get_item_for_stream_id(&app_state.config, m3u_stream_id, &m3u_path, &idx_path).await {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to get m3u url: {}", mask_sensitive_info(err.to_string().as_str()));
            return HttpResponse::BadRequest().finish();
        }
    };

    if user.proxy == ProxyType::Redirect {
        let stream_url = m3u_item.url;
        debug!("Redirecting stream request to {}", mask_sensitive_info(&stream_url));
        return HttpResponse::Found().insert_header(("Location", stream_url.to_string())).finish();
    }

    let share_live_streams = is_stream_share_enabled(m3u_item.item_type, target);
    stream_response(&app_state, m3u_item.url.as_str(), &req, None, share_live_streams).await
}

macro_rules! register_m3u_stream_routes {
    ($cfg:expr, [$($path:expr),*]) => {{
        $(
            $cfg.service(web::resource(format!("/{M3U_STREAM_PATH}/{}/{{username}}/{{password}}/{{stream_id}}", $path)).route(web::get().to(m3u_api_stream)));
        )*
    }};
}

macro_rules! register_m3u_api_routes {
    ($cfg:expr, [$($path:expr),*]) => {{
        $(
            $cfg.service(web::resource(format!("/{}", $path)).route(web::get().to(m3u_api_get)).route(web::post().to(m3u_api_post)));
        )*
    }};
}

pub fn m3u_api_register(cfg: &mut web::ServiceConfig) {
    register_m3u_api_routes!(cfg, ["get.php", "apiget", "m3u"]);
    register_m3u_stream_routes!(cfg, ["live", "movie", "series"]);
    cfg.service(web::resource(format!("/{M3U_STREAM_PATH}/{}", "{username}/{password}/{stream_id}")).route(web::get().to(m3u_api_stream)));
}