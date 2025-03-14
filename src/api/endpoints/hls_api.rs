use crate::api::api_utils::{stream_response};
use crate::api::api_utils::{try_option_bad_request};
use crate::api::model::app_state::AppState;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, TargetType};
use crate::model::playlist::{PlaylistItemType, XtreamCluster};
use crate::processing::parser::hls::rewrite_hls;
use crate::repository::playlist_repository::HLS_EXT;
use crate::utils::network::request;
use crate::utils::network::request::{replace_extension, sanitize_sensitive_info};
use axum::response::IntoResponse;
use log::{debug, error};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[allow(dead_code)]
struct HlsApiPathParams {
    token: String,
    username: String,
    password: String,
    stream_id: u32,
    chunk: u32,
}

pub(in crate::api) async fn handle_hls_stream_request(app_state: &Arc<AppState>,
                                                      user: &ProxyUserCredentials,
                                                      hls_url: &str,
                                                      virtual_id: u32,
                                                      input: &ConfigInput,
                                                      target_type: TargetType) -> impl axum::response::IntoResponse + Send {
    let url = replace_extension(hls_url, HLS_EXT);
    let server_info = app_state.config.get_user_server_info(user).await;
    match request::download_text_content(Arc::clone(&app_state.http_client), input, &url, None).await {
        Ok(content) => {
            let (hls_entry, hls_content) = rewrite_hls(&server_info.get_base_url(), &content, hls_url, virtual_id, user, &target_type, &input.name);
            app_state.hls_cache.add_entry(hls_entry).await;
            axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "application/x-mpegurl")
                .body(hls_content)
                .unwrap()
                .into_response()
        }
        Err(err) => {
            error!("Failed to download m3u8 {}", sanitize_sensitive_info(err.to_string().as_str()));
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
    }
}

async fn hls_api_stream(
    req_headers: axum::http::HeaderMap,
    axum::extract::Path(params): axum::extract::Path<HlsApiPathParams>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    let (user, target) = try_option_bad_request!(
        app_state.config.get_target_for_user(&params.username, &params.password).await, false,
        format!("Could not find any user {}", params.username));
    if !user.has_permissions(&app_state).await {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    let Some(hls_entry) = app_state.hls_cache.get_entry(&params.token).await else { return axum::http::StatusCode::BAD_REQUEST.into_response(); };
    let Some(hls_url) = hls_entry.get_chunk_url(params.chunk) else { return axum::http::StatusCode::BAD_REQUEST.into_response(); };
    let target_name = &target.name;
    let virtual_id = params.stream_id;
    let input = try_option_bad_request!(app_state.config.get_input_by_name(&hls_entry.input_name), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));

    if hls_url.ends_with(HLS_EXT) {
        return handle_hls_stream_request(&app_state, &user, hls_url, virtual_id, input, hls_entry.target_type.clone()).await.into_response();
    }

    // let (pli_url, input_name) = if hls_entry.target_type == TargetType::Xtream {
    //     let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
    //     (pli.url, pli.input_name)
    // } else {
    //     let pli = try_result_bad_request!(m3u_repository::m3u_get_item_for_stream_id(virtual_id, &app_state.config, target).await, true, format!("Failed to read xtream item for stream id {}", virtual_id));
    //     (pli.url, pli.input_name)
    // };
    stream_response(&app_state, hls_url, &req_headers, Some(input), PlaylistItemType::Live, target, &user).await.into_response()
}

pub fn hls_api_register() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/hls/{token}/{username}/{password}/{stream_id}/{chunk}", axum::routing::get(hls_api_stream))
    //cfg.service(web::resource("/hls/{token}/{stream}").route(web::get().to(xtream_player_api_hls_stream)));
    //cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
}