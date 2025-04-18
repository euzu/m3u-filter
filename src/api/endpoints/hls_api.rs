use crate::api::api_utils::stream_response;
use crate::api::api_utils::try_option_bad_request;
use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream::{create_custom_video_stream_response, CustomVideoStreamType};
use crate::model::api_proxy::{ProxyUserCredentials, UserConnectionPermission};
use crate::model::config::{ConfigInput};
use crate::model::playlist::{PlaylistItemType, XtreamCluster};
use crate::processing::parser::hls::{rewrite_hls, RewriteHlsProps};
use crate::utils::network::request;
use crate::utils::network::request::{is_hls_url, replace_url_extension, sanitize_sensitive_info};
use axum::response::IntoResponse;
use log::{debug, error};
use serde::Deserialize;
use std::sync::Arc;
use crate::utils::constants::HLS_EXT;
use crate::utils::crypto_utils;

#[derive(Debug, Deserialize)]
struct HlsApiPathParams {
    username: String,
    password: String,
    input_id: u16,
    stream_id: u32,
    token: String,
}

fn hls_response(hls_content: String) -> impl IntoResponse + Send {
        axum::response::Response::builder()
            .status(axum::http::StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, "application/x-mpegurl")
            .body(hls_content)
            .unwrap()
            .into_response()
}

pub(in crate::api) async fn handle_hls_stream_request(app_state: &Arc<AppState>,
                                                      user: &ProxyUserCredentials,
                                                      hls_url: &str,
                                                      virtual_id: u32,
                                                      input: &ConfigInput) -> impl IntoResponse + Send {
    let url = replace_url_extension(hls_url, HLS_EXT);
    let server_info = app_state.config.get_user_server_info(user).await;

    match request::download_text_content(Arc::clone(&app_state.http_client), input, &url, None).await {
        Ok((content, response_url)) => {
            let rewrite_hls_props = RewriteHlsProps {
                secret: &app_state.config.t_encrypt_secret,
                base_url: &server_info.get_base_url(),
                content: &content,
                hls_url: response_url,
                virtual_id,
                input_id: input.id,
            };
            let hls_content = rewrite_hls(user, &rewrite_hls_props);
            hls_response(hls_content).into_response()
        }
        Err(err) => {
            error!("Failed to download m3u8 {}", sanitize_sensitive_info(err.to_string().as_str()));
            create_custom_video_stream_response(&app_state.config, &CustomVideoStreamType::ChannelUnavailable).into_response()
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
    if user.permission_denied(&app_state) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let connection_permission = user.connection_permission(&app_state).await;
    if connection_permission == UserConnectionPermission::Exhausted {
        return create_custom_video_stream_response(&app_state.config, &CustomVideoStreamType::UserConnectionsExhausted).into_response();
    }

    let Ok(hls_url) = crypto_utils::decrypt_text(&app_state.config.t_encrypt_secret, &params.token) else { return axum::http::StatusCode::BAD_REQUEST.into_response(); };

    let target_name = &target.name;
    let virtual_id = params.stream_id;
    let input = try_option_bad_request!(app_state.config.get_input_by_id(params.input_id), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));

    if is_hls_url(&hls_url) {
        return handle_hls_stream_request(&app_state, &user, &hls_url, virtual_id, input).await.into_response();
    }

    stream_response(&app_state, &hls_url, &req_headers, Some(input), PlaylistItemType::LiveHls, target, &user, connection_permission).await.into_response()
}

pub fn hls_api_register() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/hls/{username}/{password}/{input_id}/{stream_id}/{token}", axum::routing::get(hls_api_stream))
    //cfg.service(web::resource("/hls/{token}/{stream}").route(web::get().to(xtream_player_api_hls_stream)));
    //cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_hls_api_register() {

    }

}