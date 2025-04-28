use crate::api::api_utils::{bad_response_with_delete_cookie, check_force_provider, create_session_cookie_for_provider, force_provider_stream_response, get_stream_alternative_url};
use crate::api::api_utils::{get_stream_info_from_crypted_cookie, try_option_bad_request};
use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream::{create_custom_video_stream_response, CustomVideoStreamType};
use crate::model::api_proxy::{ProxyUserCredentials, UserConnectionPermission};
use crate::model::config::ConfigInput;
use crate::model::playlist::{PlaylistItemType, XtreamCluster};
use crate::processing::parser::hls::{rewrite_hls, RewriteHlsProps};
use crate::utils::constants::HLS_EXT;
use crate::utils::network::request;
use crate::utils::network::request::{is_hls_url, replace_url_extension, sanitize_sensitive_info};
use axum::response::IntoResponse;
use log::{debug, error};
use serde::Deserialize;
use std::sync::Arc;
use crate::api::model::provider_config::ProviderConfig;

#[derive(Debug, Deserialize)]
struct HlsApiPathParams {
    username: String,
    password: String,
    input_id: u16,
    stream_id: u32,
    token: String,
}

fn hls_response(hls_content: String, cookie: Option<String>) -> impl IntoResponse + Send {
    let mut builder = axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/x-mpegurl");
    if let Some(cookie) = cookie {
        builder = builder.header(axum::http::header::SET_COOKIE, cookie);
    }
    builder.body(hls_content)
        .unwrap()
        .into_response()
}

pub(in crate::api) async fn handle_hls_stream_request(app_state: &Arc<AppState>,
                                                      user: &ProxyUserCredentials,
                                                      provider_name: Option<String>,
                                                      hls_url: &str,
                                                      virtual_id: u32,
                                                      input: &ConfigInput) -> impl IntoResponse + Send {
    let url = replace_url_extension(hls_url, HLS_EXT);
    let server_info = app_state.config.get_user_server_info(user).await;

    let grace_token = app_state.active_users.get_or_create_token(&user.username).await;
    let create_stream_and_cookie = |provider_cfg: &Arc<ProviderConfig>| {
        let stream_url = get_stream_alternative_url(&url, input, provider_cfg);
        let cookie = create_session_cookie_for_provider(
            &app_state.config.t_encrypt_secret,
            &grace_token.clone().unwrap_or_default(),
            virtual_id,
            &provider_cfg.name,
            &stream_url,
        );
        (stream_url, Some(provider_cfg.name.to_string()), cookie)
    };

    let (request_url, provider, cookie) = match provider_name {
        None => match app_state.active_provider.get_next_provider(&input.name).await {
            Some(provider_cfg) => create_stream_and_cookie(&provider_cfg),
            None => (url, None, None),
        },
        Some(provider) => match app_state.active_provider.force_exact_acquire_connection(&provider).await.get_provider_config() {
            Some(provider_cfg) => create_stream_and_cookie(&provider_cfg),
            None => (url, None, None),
        },
    };

    match request::download_text_content(Arc::clone(&app_state.http_client), input, &request_url, None).await {
        Ok((content, response_url)) => {
            let rewrite_hls_props = RewriteHlsProps {
                secret: &app_state.config.t_encrypt_secret,
                base_url: &server_info.get_base_url(),
                content: &content,
                hls_url: response_url,
                virtual_id,
                input_id: input.id,
                provider_name: provider.unwrap_or_default(), // this should not happen
                user_token: grace_token.unwrap_or_default().to_string(),
            };
            let hls_content = rewrite_hls(user, &rewrite_hls_props);
            hls_response(hls_content, cookie).into_response()
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

    let target_name = &target.name;
    let virtual_id = params.stream_id;
    let input = try_option_bad_request!(app_state.config.get_input_by_id(params.input_id), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));

    let (_provider_name, connection_permission) = check_force_provider(&app_state, virtual_id, PlaylistItemType::LiveHls, &req_headers, &user).await;
    if connection_permission == UserConnectionPermission::Exhausted {
        return create_custom_video_stream_response(&app_state.config, &CustomVideoStreamType::UserConnectionsExhausted).into_response();
    }

    let Some((stream_token, stream_virtual_id, stream_provider_name, hls_url)) = get_stream_info_from_crypted_cookie(&app_state.config.t_encrypt_secret, &params.token)
    else {
        return bad_response_with_delete_cookie().into_response();
    };

    if stream_virtual_id != virtual_id || app_state.active_users.get_token(&user.username).await.is_some_and(|t| ! t.eq(&stream_token)) {
        return bad_response_with_delete_cookie().into_response();
    }

    let provider_name = Some(stream_provider_name);

    if is_hls_url(&hls_url) {
        return handle_hls_stream_request(&app_state, &user, provider_name, &hls_url, virtual_id, input).await.into_response();
    }

    // if provider_name.is_some() {
    // TODO we decode twice the cookie, one time to check for connection permission and one time in force_provider_stream_response
    force_provider_stream_response(&app_state, &params.token, virtual_id, PlaylistItemType::LiveHls, &req_headers, input, &user).await.into_response()
    // } else {
    //     stream_response(&app_state, virtual_id, PlaylistItemType::LiveHls, &hls_url, &req_headers, input, target, &user, connection_permission).await.into_response()
    // }
}

pub fn hls_api_register() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/hls/{username}/{password}/{input_id}/{stream_id}/{token}", axum::routing::get(hls_api_stream))
    //cfg.service(web::resource("/hls/{token}/{stream}").route(web::get().to(xtream_player_api_hls_stream)));
    //cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
}
