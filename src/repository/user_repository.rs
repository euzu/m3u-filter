use std::collections::HashSet;
use crate::model::config::Config;
use crate::model::playlist_categories::{PlaylistCategoriesDto, PlaylistCategoryDto};
use crate::utils::file::file_utils;
use crate::utils::json_utils::json_write_documents_to_file;
use log::error;
use std::path::{Path, PathBuf};
use crate::model::playlist::XtreamCluster;

const USER_LIVE_BOUQUET: &str = "live_bouquet.json";
const USER_VOD_BOUQUET: &str = "vod_bouquet.json";
const USER_SERIES_BOUQUET: &str = "series_bouquet.json";

pub fn get_user_storage_path(cfg: &Config, username: &str) -> Option<PathBuf> {
    cfg.user_config_dir.as_ref().and_then(|ucd| file_utils::get_file_path(ucd, Some(std::path::PathBuf::from(username))))
}

fn ensure_user_storage_path(cfg: &Config, username: &str) -> Option<PathBuf> {
    if let Some(path) = get_user_storage_path(cfg, username) {
        if !path.exists() && std::fs::create_dir_all(&path).is_err() {
            error!("Failed to create user config dir, can't create directory {path:?}");
        }
        Some(path)
    } else {
        None
    }
}

pub fn user_get_live_bouquet_path(user_storage_path: &Path) -> PathBuf {
    user_storage_path.join(PathBuf::from(USER_LIVE_BOUQUET))
}

pub fn user_get_vod_bouquet_path(user_storage_path: &Path) -> PathBuf {
    user_storage_path.join(PathBuf::from(USER_VOD_BOUQUET))
}

pub fn user_get_series_bouquet_path(user_storage_path: &Path) -> PathBuf {
    user_storage_path.join(PathBuf::from(USER_SERIES_BOUQUET))
}

pub async fn save_user_bouquet(cfg: &Config, username: &str, bouquet: &PlaylistCategoriesDto) -> Result<(), std::io::Error> {
    if let Some(storage_path) = ensure_user_storage_path(cfg, username) {
        json_write_documents_to_file(&user_get_live_bouquet_path(&storage_path), &bouquet.live)?;
        json_write_documents_to_file(&user_get_vod_bouquet_path(&storage_path), &bouquet.vod)?;
        json_write_documents_to_file(&user_get_series_bouquet_path(&storage_path), &bouquet.series)?;
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("User config path not found for user {username}")))
    }
}

async fn load_user_bouquet_from_file(file: &Path) -> String {
    tokio::fs::read_to_string(file).await.unwrap_or_else(|_err| "[]".to_string())
}

pub async fn load_user_bouquet_as_json(cfg: &Config, username: &str) -> Option<String> {
    if let Some(storage_path) = get_user_storage_path(cfg, username) {
        if storage_path.exists() {
            let live = load_user_bouquet_from_file(&user_get_live_bouquet_path(&storage_path)).await;
            let vod = load_user_bouquet_from_file(&user_get_vod_bouquet_path(&storage_path)).await;
            let series = load_user_bouquet_from_file(&user_get_series_bouquet_path(&storage_path)).await;
            return Some(format!(r#"{{"live": {live}, "vod": {vod}, "series": {series} }}"#));
        }
    }
    None
}

pub(crate) async fn user_get_cluster_bouquet(cfg: &Config, username: &str, cluster: XtreamCluster) -> Option<Vec<PlaylistCategoryDto>> {
    if let Some(storage_path) = get_user_storage_path(cfg, username) {
        if storage_path.exists() {
            let content = load_user_bouquet_from_file(& match cluster {
                XtreamCluster::Live => user_get_live_bouquet_path(&storage_path),
                XtreamCluster::Video => user_get_vod_bouquet_path(&storage_path),
                XtreamCluster::Series => user_get_series_bouquet_path(&storage_path),
            }).await;
            if let Ok(bouquet) = serde_json::from_str::<Vec<PlaylistCategoryDto>>(&content) {
                if !bouquet.is_empty() {
                    return Some(bouquet);
                }
            }
        }
    }
    None
}


pub(crate) async fn user_get_live_bouquet(cfg: &Config, username: &str) -> Option<Vec<PlaylistCategoryDto>> {
    user_get_cluster_bouquet(cfg, username, XtreamCluster::Live).await
}

pub(crate) async fn user_get_vod_bouquet(cfg: &Config, username: &str) -> Option<Vec<PlaylistCategoryDto>> {
    user_get_cluster_bouquet(cfg, username, XtreamCluster::Video).await
}

pub(crate) async fn user_get_series_bouquet(cfg: &Config, username: &str) -> Option<Vec<PlaylistCategoryDto>> {
    user_get_cluster_bouquet(cfg, username, XtreamCluster::Series).await
}

pub async fn user_get_bouquet_filter(config: &Config, username: &str, category_id: &str, cluster: XtreamCluster) -> Option<HashSet<String>> {
    let bouquet = match cluster {
        XtreamCluster::Live => user_get_live_bouquet(config, username).await,
        XtreamCluster::Video => user_get_vod_bouquet(config, username).await,
        XtreamCluster::Series => user_get_series_bouquet(config, username).await,
    };
    let category_id = category_id.trim();
    let mut filter = HashSet::new();
    if !category_id.is_empty() {
        filter.insert(category_id.to_string());
    }
    if let Some(bouquet_categories) = bouquet {
        if !bouquet_categories.is_empty() {
            for c in bouquet_categories {
                filter.insert(c.id);
            }
        }
    }
    if filter.is_empty() {
        None
    } else {
        Some(filter)
    }
}