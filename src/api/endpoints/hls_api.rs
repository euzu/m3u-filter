use crate::api::api_utils::{get_user_target_by_credentials, stream_response};
use crate::api::api_utils::{try_option_bad_request, try_result_bad_request};
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, TargetType};
use crate::model::playlist::{PlaylistEntry, PlaylistItemType, XtreamCluster};
use crate::processing::parser::hls::{rewrite_hls, M3U_HLSR_PREFIX};
use crate::repository::playlist_repository::HLS_EXT;
use crate::repository::{m3u_repository, xtream_repository};
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
    channel: String,
    hash: String,
    chunk: String,
}

pub(in crate::api) async fn handle_hls_stream_request(app_state: &Arc<AppState>, user: &ProxyUserCredentials, pli: &dyn PlaylistEntry, input: &ConfigInput, target_type: TargetType) -> impl axum::response::IntoResponse + Send {
    let url = replace_extension(&pli.get_provider_url(), HLS_EXT);
    match request::download_text_content(Arc::clone(&app_state.http_client), input, &url, None).await {
        Ok(content) => {
            let hls_content = rewrite_hls(&content, pli.get_virtual_id(), user, &target_type);
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
    req_headers: &axum::http::HeaderMap,
    api_req: &UserApiRequest,
    params: HlsApiPathParams,
    app_state: &Arc<AppState>,
    target_type: TargetType,
) -> impl axum::response::IntoResponse + Send {
    let (user, target) = try_option_bad_request!(
        get_user_target_by_credentials(&params.username, &params.password, api_req, app_state).await,
        false,
        format!("Could not find any user {}", params.username));
    if !user.has_permissions(app_state).await {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    let target_name = &target.name;
    let virtual_id: u32 = try_result_bad_request!(params.channel.parse());
    let (pli_url, input_name) = if target_type == TargetType::Xtream {
        let (pli, _) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
        (pli.url, pli.input_name)
    } else {
        let pli = try_result_bad_request!(m3u_repository::m3u_get_item_for_stream_id(virtual_id, &app_state.config, target).await, true, format!("Failed to read xtream item for stream id {}", virtual_id));
        (pli.url, pli.input_name)
    };
    let input = try_option_bad_request!(app_state.config.get_input_by_name(&input_name), true, format!("Cant find input for target {target_name}, context {}, stream_id {virtual_id}", XtreamCluster::Live));
    // let input_username = input.username.as_ref().map_or("", |v| v);
    // let input_password = input.password.as_ref().map_or("", |v| v);
    // let input_url =  input.url.as_str();

    // we don't respond as hlsr, we take the original stream, because the location could be different and then it does not work
    // The next problem is, different url to same channel causes to fail stream share.
    // let stream_url = format!("{input_url}/hlsr/{token}/{input_username}/{input_password}/{}/{hash}/{chunk}", pli.provider_id);
    stream_response(app_state, &pli_url, req_headers, Some(input), PlaylistItemType::Live, target, &user).await.into_response()
}

#[axum::debug_handler]
async fn hls_api_stream_xtream(
    req_headers: axum::http::HeaderMap,
    axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
    axum::extract::Path(params): axum::extract::Path<HlsApiPathParams>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    hls_api_stream(&req_headers, &api_req, params, &app_state, TargetType::Xtream).await.into_response()
}

#[axum::debug_handler]
async fn hls_api_stream_m3u(
    req_headers: axum::http::HeaderMap,
    axum::extract::Query(api_req): axum::extract::Query<UserApiRequest>,
    axum::extract::Path(params): axum::extract::Path<HlsApiPathParams>,
    axum::extract::State(app_state): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse + Send {
    hls_api_stream(&req_headers, &api_req, params, &app_state, TargetType::M3u).await.into_response()
}

pub fn hls_api_register() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}", axum::routing::get(hls_api_stream_xtream))
        .route(&format!("/{M3U_HLSR_PREFIX}/{{token}}/{{username}}/{{password}}/{{channel}}/{{hash}}/{{chunk}}"), axum::routing::get(hls_api_stream_m3u))
    //cfg.service(web::resource("/hls/{token}/{stream}").route(web::get().to(xtream_player_api_hls_stream)));
    //cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
}