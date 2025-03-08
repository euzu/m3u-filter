use crate::api::api_utils::{get_user_target_by_username, get_username_from_auth_header};
use crate::api::model::app_state::AppState;
use crate::auth::authenticator::validator_user;
use crate::model::config::{Config, ConfigTarget, TargetType};
use crate::model::playlist::XtreamCluster;
use crate::model::playlist_categories::PlaylistBouquetDto;
use crate::model::xtream::PlaylistXtreamCategory;
use crate::repository::user_repository::{load_user_bouquet_as_json, save_user_bouquet};
use crate::repository::{m3u_repository};
use actix_web::body::BodyStream;
use actix_web::middleware::Compress;
use actix_web::{web, HttpResponse};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use actix_web_httpauth::middleware::HttpAuthentication;
use bytes::Bytes;
use futures::{stream, StreamExt};
use log::error;
use std::collections::HashSet;
use std::sync::Arc;
use crate::repository::xtream_repository::xtream_get_playlist_categories;

fn get_categories_from_xtream(categories: Option<Vec<PlaylistXtreamCategory>>) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    if let Some(cats) = categories {
        for category in cats {
            groups.push(category.name.to_string());
        }
    }
    groups
}


fn get_categories_from_m3u_playlist(target: &ConfigTarget, config: &Arc<Config>) -> Vec<String> {
    let mut groups = Vec::new();
    if let Some((_guard, iter)) = m3u_repository::iter_raw_m3u_playlist(config, target) {
        let mut unique_groups = HashSet::new();
        for (item, _has_next) in iter {
            if !unique_groups.contains(item.group.as_str()) {
                unique_groups.insert(item.group.to_string());
                groups.push(item.group.to_string());
            }
        }
    }
    groups
}

async fn playlist_categories(
    credentials: Option<BearerAuth>,
    app_state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    if let Some(username) = get_username_from_auth_header(credentials, &app_state) {
        if let Some((user, target)) = get_user_target_by_username(username.as_str(), &app_state).await {
            if !user.has_permissions(&app_state) {
                return HttpResponse::Forbidden().finish();
            }
            let config = &app_state.config;
            let target_name = &target.name;
            let xtream_stream = if target.has_output(&TargetType::Xtream) {
                let live_categories = get_categories_from_xtream(xtream_get_playlist_categories(config, target_name, XtreamCluster::Live).await);
                let vod_categories = get_categories_from_xtream(xtream_get_playlist_categories(config, target_name, XtreamCluster::Video).await);
                let series_categories = get_categories_from_xtream(xtream_get_playlist_categories(config, target_name, XtreamCluster::Series).await);
                stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r#"{"live": "#)),
                    Ok::<Bytes, String>(Bytes::from(serde_json::to_string(&live_categories).unwrap_or("[]".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#", "vod": "#.to_string())),
                    Ok::<Bytes, String>(Bytes::from(serde_json::to_string(&vod_categories).unwrap_or("[]".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#", "series": "#)),
                    Ok::<Bytes, String>(Bytes::from(serde_json::to_string(&series_categories).unwrap_or("[]".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r"}")),
                ])
            } else {
                stream::iter(vec![Ok::<Bytes, String>(Bytes::from(r#"{"live":[],"vod":[],"series":[]}"#))])
            };

            let m3u_stream = if target.has_output(&TargetType::M3u) {
                let live_categories = get_categories_from_m3u_playlist(target, config);
                stream::iter(vec![
                    Ok::<Bytes, String>(Bytes::from(r#"{"live": "#)),
                    Ok::<Bytes, String>(Bytes::from(serde_json::to_string(&live_categories).unwrap_or("[]".to_string()))),
                    Ok::<Bytes, String>(Bytes::from(r#","vod":[],"series":[]}"#)),
                ])
            } else {
                stream::iter(vec![Ok::<Bytes, String>(Bytes::from(r#"{"live":[],"vod":[],"series":[]}"#))])
            };

            let json_stream = stream::once(async { Ok::<Bytes, String>(Bytes::from(r#"{"xtream": "#)) })
                .chain(xtream_stream)
                .chain(stream::once(async { Ok::<Bytes, String>(Bytes::from(r#", "m3u": "#)) }))
                .chain(m3u_stream)
                .chain(stream::once(async { Ok::<Bytes, String>(Bytes::from("}")) }));


            return HttpResponse::Ok()
                .content_type(mime::APPLICATION_JSON)
                .body(BodyStream::new(json_stream));
        }
    }
    HttpResponse::BadRequest().finish()
}

async fn save_playlist_bouquet(
    credentials: Option<BearerAuth>,
    app_state: web::Data<Arc<AppState>>,
    req: web::Json<PlaylistBouquetDto>,
) -> HttpResponse {
    if let Some(username) = get_username_from_auth_header(credentials, &app_state) {
        if let Some((user, target)) = get_user_target_by_username(username.as_str(), &app_state).await {
            if !user.has_permissions(&app_state) {
                return HttpResponse::Forbidden().finish();
            }
            match save_user_bouquet(&app_state.config, &target.name, &username, &req.0).await {
                Ok(()) => {
                    return HttpResponse::Ok().finish();
                }
                Err(err) => {
                    error!("Saving bouquet for {username} failed: {err}");
                }
            }
        }
    }
    HttpResponse::BadRequest().finish()
}

async fn playlist_bouquet(
    credentials: Option<BearerAuth>,
    app_state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    if let Some(username) = get_username_from_auth_header(credentials, &app_state) {
        if let Some((user, _target)) = get_user_target_by_username(username.as_str(), &app_state).await {
            if !user.has_permissions(&app_state) {
                return HttpResponse::Forbidden().finish();
            }
            let xtream = load_user_bouquet_as_json(&app_state.config, &username, TargetType::Xtream).await;
            let m3u = load_user_bouquet_as_json(&app_state.config, &username, TargetType::M3u).await;
            return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(
                format!(r#"{{"xtream": {}, "m3u": {} }}"#, xtream.unwrap_or("null".to_string()), m3u.unwrap_or("null".to_string())));
        }
    }
    HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body("{}")
}


pub fn user_api_register(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/v1/user")
        .wrap(HttpAuthentication::with_fn(validator_user))
        .wrap(Compress::default())
        .route("/playlist/categories", web::get().to(playlist_categories))
        .route("/playlist/bouquet", web::get().to(playlist_bouquet))
        .route("/playlist/bouquet", web::post().to(save_playlist_bouquet)));
}
