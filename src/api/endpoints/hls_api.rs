use std::sync::Arc;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web::web::Data;
use log::{debug, error};
use crate::api::api_utils::{get_user_target_by_credentials, stream_response};
use crate::api::model::app_state::AppState;
use crate::api::model::request::UserApiRequest;
use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, TargetType};
use crate::model::playlist::{PlaylistEntry, PlaylistItemType, XtreamCluster};
use crate::processing::parser::hls::{rewrite_hls, M3U_HLSR_PREFIX};
use crate::api::api_utils::{try_option_bad_request, try_result_bad_request};
use crate::repository::{m3u_repository, xtream_repository};
use crate::repository::playlist_repository::HLS_EXT;
use crate::utils::network::request;
use crate::utils::network::request::{replace_extension, sanitize_sensitive_info};

pub(in crate::api) async fn handle_hls_stream_request(app_state: &Data<AppState>, user: &ProxyUserCredentials, pli: &dyn PlaylistEntry, input: &ConfigInput, target_type: TargetType) -> HttpResponse {
    let url = replace_extension(&pli.get_provider_url(), HLS_EXT);
    match request::download_text_content(Arc::clone(&app_state.http_client), input, &url, None).await {
        Ok(content) => {
            let hls_content = rewrite_hls(&content, pli.get_virtual_id(), user, &target_type);
            HttpResponse::Ok().content_type("application/x-mpegurl").body(hls_content)
        }
        Err(err) => {
            error!("Failed to download m3u8 {}", sanitize_sensitive_info(err.to_string().as_str()));
            HttpResponse::NoContent().finish()
        }
    }
}

async fn hls_api_stream(
    req: &HttpRequest,
    api_req: &web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String, String)>,
    app_state: &web::Data<AppState>,
    target_type: TargetType
) -> HttpResponse {
    let (_token, username, password, channel, _hash, _chunk) = path.into_inner();
    let (user, target) = try_option_bad_request!(
        get_user_target_by_credentials(&username, &password, api_req, app_state).await,
        false,
        format!("Could not find any user {username}"));
    if !user.is_active(&app_state) {
        debug!("User access denied: {user:?}");
        return HttpResponse::Forbidden().finish();
    }

    let target_name = &target.name;
    let virtual_id: u32 = try_result_bad_request!(channel.parse());
    let (pli_url, input_name) = if target_type == TargetType::Xtream {
        let (pli, _ ) = try_result_bad_request!(xtream_repository::xtream_get_item_for_stream_id(virtual_id, &app_state.config, target, None), true, format!("Failed to read xtream item for stream id {}", virtual_id));
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
    stream_response(app_state, &pli_url, req, Some(input), PlaylistItemType::Live, target, &user).await
}

async fn hls_api_stream_xtream(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    hls_api_stream(&req, &api_req, path, &app_state, TargetType::Xtream).await
}

async fn hls_api_stream_m3u(
    req: HttpRequest,
    api_req: web::Query<UserApiRequest>,
    path: web::Path<(String, String, String, String, String, String)>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    hls_api_stream(&req, &api_req, path, &app_state, TargetType::M3u).await
}


pub fn hls_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/hlsr/{token}/{username}/{password}/{channel}/{hash}/{chunk}").route(web::get().to(hls_api_stream_xtream)));
    cfg.service(web::resource(format!("/{M3U_HLSR_PREFIX}/{{token}}/{{username}}/{{password}}/{{channel}}/{{hash}}/{{chunk}}")).route(web::get().to(hls_api_stream_m3u)));
    //cfg.service(web::resource("/hls/{token}/{stream}").route(web::get().to(xtream_player_api_hls_stream)));
    //cfg.service(web::resource("/play/{token}/{type}").route(web::get().to(xtream_player_api_play_stream)));
}