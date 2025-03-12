use crate::api::api_utils::{get_user_target, get_user_target_by_credentials, redirect, resource_response, separate_number_and_remainder, stream_response, try_option_bad_request, try_result_bad_request};
use crate::api::endpoints::hls_api::handle_hls_stream_request;
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::model::api_proxy::ProxyType;
use crate::model::config::TargetType;
use crate::model::playlist::{FieldGetAccessor, XtreamCluster};
use crate::repository::m3u_playlist_iterator::{M3U_RESOURCE_PATH, M3U_STREAM_PATH};
use crate::repository::m3u_repository::{m3u_get_item_for_stream_id, m3u_load_rewrite_playlist};
use crate::repository::playlist_repository::HLS_EXT;
use crate::utils::network::request::{replace_extension, sanitize_sensitive_info};
use crate::utils::debug_if_enabled;
use axum::response::IntoResponse;
use bytes::Bytes;
use futures::stream;
use log::{debug, error};
use std::sync::Arc;

async fn m3u_api(
    api_req: &UserApiRequest,
    app_state: &AppState,
) -> impl axum::response::IntoResponse + Send {
    match get_user_target(api_req, app_state).await {
        Some((user, target)) => {
            match m3u_load_rewrite_playlist(&app_state.config, target, &user).await {
                Ok(m3u_iter) => {
                    // Convert the iterator into a stream of `Bytes`
                    let content_stream = stream::iter(m3u_iter.map(|line| Ok::<Bytes, String>(Bytes::from([line.to_string().as_bytes(), b"\n"].concat()))));

                    let mut builder = axum::response::Response::builder()
                        .status(axum::http::StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, mime::TEXT_PLAIN_UTF_8.to_string());
                    if api_req.content_type == "m3u_plus" {
                        builder = builder.header("Content-Disposition", "attachment; filename=\"playlist.m3u\"");
                    }
                    builder.body(axum::body::Body::from_stream(content_stream)).unwrap().into_response()
                }
                Err(err) => {
                    error!("{}", sanitize_sensitive_info(err.to_string().as_str()));
                    axum::http::StatusCode::NO_CONTENT.into_response()
                }
            }
        }
        None => axum::http::StatusCode::BAD_REQUEST.into_response(),
    }
}


async fn m3u_api_get(axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
                     axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    m3u_api(&api_req, &app_state).await
}

async fn m3u_api_post(
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Form(api_req): axum::extract::Form<UserApiRequest>,
) -> impl axum::response::IntoResponse + Send {
    m3u_api(&api_req, &app_state).await.into_response()
}

async fn m3u_api_stream(
    req_headers: axum::http::HeaderMap,
    axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
    axum::extract::Path((username, password, stream_id)): axum::extract::Path<(String, String, String)>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    let (action_stream_id, stream_ext) = separate_number_and_remainder(&stream_id);
    let virtual_id: u32 = try_result_bad_request!(action_stream_id.trim().parse());
    let Some((user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state).await
    else { return axum::http::StatusCode::BAD_REQUEST.into_response() };
    if !user.has_permissions(&app_state).await {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    if !target.has_output(&TargetType::M3u) {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }

    let m3u_item = match m3u_get_item_for_stream_id(virtual_id, &app_state.config, target).await {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to get m3u url: {}", sanitize_sensitive_info(err.to_string().as_str()));
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    let input = app_state.config.get_input_by_name(m3u_item.input_name.as_str());

    let is_hls_request = stream_ext.as_deref() == Some(HLS_EXT);

    if user.proxy == ProxyType::Redirect {
        let redirect_url = if is_hls_request { &replace_extension(&m3u_item.url, "m3u8") } else { &m3u_item.url };
        // TODO alias processing
        debug_if_enabled!("Redirecting m3u stream request to {}", sanitize_sensitive_info(redirect_url));
        return redirect(redirect_url.as_str()).into_response();
    }
    // Reverse proxy mode
    if is_hls_request {
        let target_name = &target.name;
        let hls_input = try_option_bad_request!(input, true,
            format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));
        return handle_hls_stream_request(&app_state, &user, &m3u_item, hls_input, TargetType::M3u).await.into_response();
    }

    stream_response(&app_state, m3u_item.url.as_str(), &req_headers, input, m3u_item.item_type, target, &user).await.into_response()
}

async fn m3u_api_resource(
    req_headers: axum::http::HeaderMap,
    axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
    axum::extract::Path((username, password, stream_id, resource)): axum::extract::Path<(String, String, String, String)>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    let Ok(m3u_stream_id) = stream_id.parse::<u32>() else { return axum::http::StatusCode::BAD_REQUEST.into_response() };
    let Some((user, target)) = get_user_target_by_credentials(&username, &password, &api_req, &app_state).await
    else { return axum::http::StatusCode::BAD_REQUEST.into_response() };
    if !user.has_permissions(&app_state).await {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    if !target.has_output(&TargetType::M3u) {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    let m3u_item = match m3u_get_item_for_stream_id(m3u_stream_id, &app_state.config, target).await {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to get m3u url: {}", sanitize_sensitive_info(err.to_string().as_str()));
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    let stream_url = m3u_item.get_field(resource.as_str());
    match stream_url {
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
        Some(url) => {
            if user.proxy == ProxyType::Redirect {
                debug!("Redirecting stream request to {}", sanitize_sensitive_info(&url));
                redirect(url.as_str()).into_response()
            } else {
                resource_response(&app_state, url.as_str(), &req_headers, None).await.into_response()
            }
        }
    }
}

macro_rules! register_m3u_stream_routes {
    ($router:expr, [$($path:expr),*]) => {{
        $router
        $(
        .route(&format!("/{M3U_STREAM_PATH}/{}/{{username}}/{{password}}/{{stream_id}}", $path), axum::routing::get(m3u_api_stream))
            // $cfg.service(web::resource(format!("/{M3U_STREAM_PATH}/{}/{{username}}/{{password}}/{{stream_id}}", $path)).route(web::get().to(m3u_api_stream)));
        )*
    }};
}

macro_rules! register_m3u_api_routes {
    ($router:expr, [$($path:expr),*]) => {{
        $router
        $(
            .route(&format!("/{}", $path), axum::routing::get(m3u_api_get))
            .route(&format!("/{}", $path), axum::routing::post(m3u_api_post))
            // $cfg.service(web::resource(format!("/{}", $path)).route(web::get().to(m3u_api_get)).route(web::post().to(m3u_api_post)));
        )*
    }};
}

pub fn m3u_api_register() -> axum::Router<Arc<AppState>> {
    let mut router = axum::Router::new();
    router = register_m3u_api_routes!(router, ["get.php", "apiget", "m3u"]);
    register_m3u_stream_routes!(router, ["live", "movie", "series"])
    .route(&format!("/{M3U_STREAM_PATH}/{{username}}/{{password}}/{{stream_id}}"), axum::routing::get(m3u_api_stream))
    .route(&format!("/{M3U_RESOURCE_PATH}/{{username}}/{{password}}/{{stream_id}}/{{resource}}"), axum::routing::get(m3u_api_resource))
}