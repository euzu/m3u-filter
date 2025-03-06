use std::sync::Arc;
use crate::api::api_utils::{get_user_target_by_username, get_username_from_auth_header};
use crate::api::model::app_state::AppState;
use crate::auth::authenticator::validator_user;
use crate::model::config::TargetType;
use crate::model::playlist_categories::PlaylistCategoriesDto;
use crate::repository::user_repository::{load_user_bouquet_as_json, save_user_bouquet};
use crate::repository::xtream_repository;
use actix_web::body::BodyStream;
use actix_web::middleware::Compress;
use actix_web::{web, HttpResponse};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use actix_web_httpauth::middleware::HttpAuthentication;
use bytes::Bytes;
use futures::stream;
use serde::{Deserialize, Serialize};
use std::io::Error;
use std::path::PathBuf;
use log::error;

#[derive(Deserialize, Serialize)]
struct PlaylistXtreamCategory {
    #[serde(alias = "category_id")]
    pub id: String,
    #[serde(alias = "category_name")]
    pub name: String,
}

pub(crate) async fn get_categories_content(action: Result<(Option<PathBuf>, Option<String>), Error>) -> Option<String> {
    if let Ok((Some(file_path), _content)) = action {
        if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
            // TODO deserialize like sax parser
            if let Ok(categories) = serde_json::from_str::<Vec<PlaylistXtreamCategory>>(&content) {
                return serde_json::to_string(&categories).ok();
            }
        }
    }
    None
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
            if target.has_output(&TargetType::Xtream) {
                let live_categories = get_categories_content(xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_LIVE)).await;
                let vod_categories = get_categories_content(xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_VOD)).await;
                let series_categories = get_categories_content(xtream_repository::xtream_get_collection_path(config, target_name, xtream_repository::COL_CAT_SERIES)).await;
                let json_stream =
                    stream::iter(vec![
                        Ok::<Bytes, String>(Bytes::from(r#"{"live": "#.to_string())),
                        Ok::<Bytes, String>(Bytes::from(live_categories.unwrap_or("null".to_string()))),
                        Ok::<Bytes, String>(Bytes::from(r#", "vod": "#.to_string())),
                        Ok::<Bytes, String>(Bytes::from(vod_categories.unwrap_or("null".to_string()))),
                        Ok::<Bytes, String>(Bytes::from(r#", "series": "#.to_string())),
                        Ok::<Bytes, String>(Bytes::from(series_categories.unwrap_or("null".to_string()))),
                        Ok::<Bytes, String>(Bytes::from(r"}".to_string())),
                    ]);
                return HttpResponse::Ok()
                    .content_type(mime::APPLICATION_JSON)
                    .body(BodyStream::new(json_stream));
            } else if target.has_output(&TargetType::M3u) {}
        }
    }
    HttpResponse::BadRequest().finish()
}

async fn save_playlist_bouquet(
    credentials: Option<BearerAuth>,
    app_state: web::Data<Arc<AppState>>,
    req: web::Json<PlaylistCategoriesDto>,
) -> HttpResponse {
    if let Some(username) = get_username_from_auth_header(credentials, &app_state) {
        if let Some((user, _target)) = get_user_target_by_username(username.as_str(), &app_state).await {
            if !user.has_permissions(&app_state) {
                return HttpResponse::Forbidden().finish();
            }
            match save_user_bouquet(&app_state.config, &username, &req.0).await {
                Ok(()) => {
                    return HttpResponse::Ok().finish();
                },
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
            if let Some(bouquet) = load_user_bouquet_as_json(&app_state.config, &username).await {
                return HttpResponse::Ok().content_type(mime::APPLICATION_JSON).body(bouquet);
            }
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
