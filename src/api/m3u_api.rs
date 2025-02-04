use actix_web::{web, HttpRequest, HttpResponse};
use bytes::Bytes;
use futures::stream;
use log::{debug, error};

use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, resource_response, separate_number_and_remainder, stream_response};
use crate::api::hls_api::handle_hls_stream_request;
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::model::api_proxy::ProxyType;
use crate::model::config::TargetType;
use crate::model::playlist::{FieldGetAccessor, XtreamCluster};
use crate::repository::m3u_playlist_iterator::{M3U_RESOURCE_PATH, M3U_STREAM_PATH};
use crate::repository::m3u_repository::{m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};
use crate::utils::request_utils::{replace_extension, sanitize_sensitive_info};
use crate::{debug_if_enabled, try_option_bad_request, try_result_bad_request};
use crate::repository::playlist_repository::HLS_EXT;

async fn m3u_api(
    api_req: &UserApiRequest,
    app_state: &AppState,
) -> HttpResponse {
    match get_user_target(api_req, app_state) {
        Some((user, target)) => {
            match m3u_load_rewrite_playlist(&app_state.config, target, &user).await {
                Ok(m3u_iter) => {
                    // Convert the iterator into a stream of `Bytes`
                    let content_stream = stream::iter(m3u_iter.map(|line| Ok::<Bytes, String>(Bytes::from([line.as_bytes(), b"\n"].concat()))));
                    let mut builder = HttpResponse::Ok();
                    builder.content_type(mime::TEXT_PLAIN_UTF_8);
                    if api_req.content_type == "m3u_plus" {
                        builder.insert_header(("Content-Disposition", "attachment; filename=\"playlist.m3u\""));
                    }
                    builder.streaming(content_stream)
                }
                Err(err) => {
                    error!("{}", sanitize_sensitive_info(err.to_string().as_str()));
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
    let (action_stream_id, stream_ext) = separate_number_and_remainder(&stream_id);
    let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
    let Some((user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state) else { return HttpResponse::BadRequest().finish() };

    if !target.has_output(&TargetType::M3u) {
        return HttpResponse::BadRequest().finish();
    }

    let m3u_item = match m3u_get_item_for_stream_id(virtual_id, &app_state.config, target).await {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to get m3u url: {}", sanitize_sensitive_info(err.to_string().as_str()));
            return HttpResponse::BadRequest().finish();
        }
    };

    let is_hls_request = stream_ext.as_deref() == Some(HLS_EXT);

    if user.proxy == ProxyType::Redirect {
        let redirect_url = if is_hls_request { &replace_extension(&m3u_item.url, "m3u8") } else { &m3u_item.url };
        debug_if_enabled!("Redirecting m3u stream request to {}", sanitize_sensitive_info(redirect_url));
        return HttpResponse::Found().insert_header(("Location", redirect_url.as_str())).finish();
    }
    // Reverse proxy mode
    if is_hls_request {
        let target_name = &target.name;
        let input = try_option_bad_request!(app_state.config.get_input_by_name(m3u_item.input_name.as_str()), true,
            format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));
        return handle_hls_stream_request(&app_state, &user, &m3u_item, input, TargetType::M3u).await;
    }

    stream_response(&app_state, m3u_item.url.as_str(), &req, None, m3u_item.item_type, target).await
}

async fn m3u_api_resource(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let (username, password, stream_id, resource) = path.into_inner();
    let Ok(m3u_stream_id) = stream_id.parse::<u32>() else { return HttpResponse::BadRequest().finish() };
    let Some((user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state) else { return HttpResponse::BadRequest().finish() };

    if !target.has_output(&TargetType::M3u) {
        return HttpResponse::BadRequest().finish();
    }
    let m3u_item = match m3u_get_item_for_stream_id(m3u_stream_id, &app_state.config, target).await {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to get m3u url: {}", sanitize_sensitive_info(err.to_string().as_str()));
            return HttpResponse::BadRequest().finish();
        }
    };

    let stream_url = m3u_item.get_field(resource.as_str());
    match stream_url {
        None => HttpResponse::NotFound().finish(),
        Some(url) => {
            if user.proxy == ProxyType::Redirect {
                debug!("Redirecting stream request to {}", sanitize_sensitive_info(&url));
                HttpResponse::Found().insert_header(("Location", url.as_str())).finish()
            } else {
                resource_response(&app_state, url.as_str(), &req, None).await
            }
        }
    }
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
    cfg.service(web::resource(format!("/{M3U_STREAM_PATH}/{{username}}/{{password}}/{{stream_id}}")).route(web::get().to(m3u_api_stream)));
    cfg.service(web::resource(format!("/{M3U_RESOURCE_PATH}/{{username}}/{{password}}/{{stream_id}}/{{resource}}")).route(web::get().to(m3u_api_resource)));
}