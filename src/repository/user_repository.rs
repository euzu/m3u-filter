use crate::model::api_proxy::{ProxyType, ProxyUserCredentials, ProxyUserStatus, TargetUser};
use crate::model::config::{Config, TargetType};
use crate::model::playlist::XtreamCluster;
use crate::model::playlist_categories::{PlaylistBouquetDto, TargetBouquetDto};
use crate::repository::bplustree::BPlusTree;
use crate::utils::file::file_utils;
use crate::utils::json_utils::json_write_documents_to_file;
use log::{error};
use std::collections::{HashMap, HashSet};
use std::io::Error;
use std::path::{Path, PathBuf};
use chrono::Local;
use crate::repository::xtream_repository::xtream_get_playlist_categories;

const USER_LIVE_BOUQUET: &str = "live_bouquet.json";
const USER_VOD_BOUQUET: &str = "vod_bouquet.json";
const USER_SERIES_BOUQUET: &str = "series_bouquet.json";
const API_USER_DB_FILE: &str = "api_user.db";

// This is a Helper class to store all user into one Database file.
// For the Config files we keep the old structure where a user is assigned to a target.
// But for storing inside one db file it is easier to store the target next to the user.
// due to known issue with  bincode and skip_serialization_if we have to list all fields and cant use ProxyUserCredentials
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredProxyUserCredentials {
    pub target: String,
    pub username: String,
    pub password: String,
    pub token: Option<String>,
    pub proxy: ProxyType,
    pub server: Option<String>,
    pub epg_timeshift: Option<String>,
    pub created_at: Option<i64>,
    pub exp_date: Option<i64>,
    pub max_connections: Option<u32>,
    pub status: Option<ProxyUserStatus>,
}

impl StoredProxyUserCredentials {
    fn from(proxy: &ProxyUserCredentials, target_name: &str) -> Self {
        Self {
            target: String::from(target_name),
            username: proxy.username.clone(),
            password: proxy.password.clone(),
            token: proxy.token.clone(),
            proxy: proxy.proxy.clone(),
            server: proxy.server.clone(),
            epg_timeshift: proxy.epg_timeshift.clone(),
            created_at: proxy.created_at,
            exp_date: proxy.exp_date,
            max_connections: proxy.max_connections,
            status: proxy.status.clone(),
        }
    }

    fn to(stored: &StoredProxyUserCredentials) -> ProxyUserCredentials {
        ProxyUserCredentials {
            username: stored.username.clone(),
            password: stored.password.clone(),
            token: stored.token.clone(),
            proxy: stored.proxy.clone(),
            server: stored.server.clone(),
            epg_timeshift: stored.epg_timeshift.clone(),
            created_at: stored.created_at,
            exp_date: stored.exp_date,
            max_connections: stored.max_connections,
            status: stored.status.clone(),
        }
    }
}


pub fn get_api_user_db_path(cfg: &Config) -> PathBuf {
    PathBuf::from(&cfg.t_config_path).join(API_USER_DB_FILE)
}


fn add_target_user_to_user_tree(target_users: &[TargetUser], user_tree: &mut BPlusTree<String, StoredProxyUserCredentials>) {
    for target_user in target_users {
        for user in &target_user.credentials {
            let store_user: StoredProxyUserCredentials = StoredProxyUserCredentials::from(user, &target_user.target);
            user_tree.insert(user.username.clone(), store_user);
        }
    }
}

pub fn merge_api_user(cfg: &Config, target_users: &[TargetUser]) -> Result<u64, std::io::Error> {
    let path = get_api_user_db_path(cfg);
    let lock = cfg.file_locks.read_lock(&path);
    let mut user_tree: BPlusTree<String, StoredProxyUserCredentials> = BPlusTree::load(&path).unwrap_or_else(|_| BPlusTree::new());
    drop(lock);
    add_target_user_to_user_tree(target_users, &mut user_tree);
    let _lock = cfg.file_locks.write_lock(&path);
    user_tree.store(&path)
}

/// # Panics
///
/// Will panic if `backup_dir` is not given
pub fn backup_api_user_db_file(cfg: &Config, path: &Path) {
    let backup_dir = cfg.backup_dir.as_ref().unwrap().as_str();
    let backup_path = PathBuf::from(backup_dir).join(format!("{API_USER_DB_FILE}_{}", Local::now().format("%Y%m%d_%H%M%S")));
    let _lock = cfg.file_locks.read_lock(path);
    match std::fs::copy(path, &backup_path) {
        Ok(_) => {}
        Err(err) => { error!("Could not backup file {}:{}", &backup_path.to_str().unwrap_or("?"), err) }
    }
}

pub fn store_api_user(cfg: &Config, target_users: &[TargetUser]) -> Result<u64, std::io::Error> {
    let mut user_tree = BPlusTree::<String, StoredProxyUserCredentials>::new();
    add_target_user_to_user_tree(target_users, &mut user_tree);
    let path = get_api_user_db_path(cfg);
    backup_api_user_db_file(cfg, &path);
    let _lock = cfg.file_locks.write_lock(&path);
    user_tree.store(&path)
}

pub fn load_api_user(cfg: &Config) -> Result<Vec<TargetUser>, std::io::Error> {
    let path = get_api_user_db_path(cfg);
    let lock = cfg.file_locks.read_lock(&path);
    let user_tree = BPlusTree::<String, StoredProxyUserCredentials>::load(&path)?;
    drop(lock);
    let mut target_users: HashMap<String, TargetUser> = HashMap::new();
    for (_uname, stored_user) in &user_tree {
        let proxy_user: ProxyUserCredentials = StoredProxyUserCredentials::to(stored_user);
        let target_name = stored_user.target.clone();
        match target_users.entry(target_name) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let target = entry.get_mut();
                target.credentials.push(proxy_user);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(TargetUser {
                    target: stored_user.target.clone(),
                    credentials: vec![proxy_user],
                });
            }
        }
    }
    Ok(target_users.into_values().collect())
}

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

fn user_get_live_bouquet_path(user_storage_path: &Path, target: &TargetType) -> PathBuf {
    user_storage_path.join(PathBuf::from(format!("{}_{USER_LIVE_BOUQUET}", target.to_string().to_lowercase())))
}

fn user_get_vod_bouquet_path(user_storage_path: &Path, target: &TargetType) -> PathBuf {
    user_storage_path.join(PathBuf::from(format!("{}_{USER_VOD_BOUQUET}", target.to_string().to_lowercase())))
}

fn user_get_series_bouquet_path(user_storage_path: &Path, target: &TargetType) -> PathBuf {
    user_storage_path.join(PathBuf::from(format!("{}_{USER_SERIES_BOUQUET}", target.to_string().to_lowercase())))
}

fn save_user_bouquet_for_target(storage_path: &Path, target: TargetType, bouquet: &TargetBouquetDto)-> Result<(), Error> {
    json_write_documents_to_file(&user_get_live_bouquet_path(storage_path, &target), &bouquet.live)?;
    json_write_documents_to_file(&user_get_vod_bouquet_path(storage_path, &target), &bouquet.vod)?;
    json_write_documents_to_file(&user_get_series_bouquet_path(storage_path, &target), &bouquet.series)?;
    Ok(())
}

pub fn save_user_bouquet(cfg: &Config, username: &str, bouquet: &PlaylistBouquetDto) -> Result<(), std::io::Error> {
    if let Some(storage_path) = ensure_user_storage_path(cfg, username) {
        if let Some(xb) =  &bouquet.xtream {
            save_user_bouquet_for_target(&storage_path, TargetType::Xtream, xb)?;
        }
        if let Some(mb) =  &bouquet.m3u {
            save_user_bouquet_for_target(&storage_path, TargetType::M3u, mb)?;
        }
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("User config path not found for user {username}")))
    }
}

async fn load_user_bouquet_from_file(file: &Path) -> Option<String> {
    tokio::fs::read_to_string(file).await.ok()
}

pub async fn load_user_bouquet_as_json(cfg: &Config, username: &str, target: TargetType) -> Option<String> {
    if let Some(storage_path) = get_user_storage_path(cfg, username) {
        if storage_path.exists() {
            let live = load_user_bouquet_from_file(&user_get_live_bouquet_path(&storage_path, &target)).await;
            let vod = load_user_bouquet_from_file(&user_get_vod_bouquet_path(&storage_path, &target)).await;
            let series = load_user_bouquet_from_file(&user_get_series_bouquet_path(&storage_path, &target)).await;
            return Some(format!(r#"{{"live": {}, "vod": {}, "series": {} }}"#,
                                live.unwrap_or("null".to_string()),
                                vod.unwrap_or("null".to_string()),
                                series.unwrap_or("null".to_string()),
            ));
        }
    }
    None
}

async fn user_get_cluster_bouquet(cfg: &Config, username: &str, target: &TargetType, cluster: XtreamCluster) -> Option<Vec<String>> {
    if let Some(storage_path) = get_user_storage_path(cfg, username) {
        if storage_path.exists() {
            let content = load_user_bouquet_from_file(&match cluster {
                XtreamCluster::Live => user_get_live_bouquet_path(&storage_path, target),
                XtreamCluster::Video => user_get_vod_bouquet_path(&storage_path, target),
                XtreamCluster::Series => user_get_series_bouquet_path(&storage_path, target),
            }).await;
            if let Some(value) = content {
                if let Ok(bouquet) = serde_json::from_str::<Vec<String>>(&value) {
                    if !bouquet.is_empty() {
                        return Some(bouquet);
                    }
                }
            }
        }
    }
    None
}


pub(crate) async fn user_get_live_bouquet(cfg: &Config, username: &str, target: &TargetType) -> Option<Vec<String>> {
    user_get_cluster_bouquet(cfg, username, target, XtreamCluster::Live).await
}

pub(crate) async fn user_get_vod_bouquet(cfg: &Config, username: &str, target: &TargetType) -> Option<Vec<String>> {
    user_get_cluster_bouquet(cfg, username, target, XtreamCluster::Video).await
}

pub(crate) async fn user_get_series_bouquet(cfg: &Config, username: &str, target: &TargetType) -> Option<Vec<String>> {
    user_get_cluster_bouquet(cfg, username, target, XtreamCluster::Series).await
}

// TODO save user bouquet for xtream with category id, and not load it every time when playlist read
pub async fn user_get_bouquet_filter(config: &Config, target_name: &str, username: &str, category_id: &str, target: TargetType, cluster: XtreamCluster) -> Option<HashSet<String>> {
    let bouquet = match cluster {
        XtreamCluster::Live => user_get_live_bouquet(config, username, &target).await,
        XtreamCluster::Video => user_get_vod_bouquet(config, username, &target).await,
        XtreamCluster::Series => user_get_series_bouquet(config, username, &target).await,
    };
    let category_id = category_id.trim();
    let mut filter = HashSet::new();
    if !category_id.is_empty() {
        filter.insert(category_id.to_string());
    }
    if let Some(bouquet_categories) = bouquet {
        if !bouquet_categories.is_empty() {
            if target == TargetType::Xtream {
                if let Some(xtream_categories) = xtream_get_playlist_categories(config, target_name,cluster).await  {
                    let map: HashMap<String, String> = xtream_categories
                        .into_iter()
                        .map(|item| (item.name, item.id))
                        .collect();
                    for c in bouquet_categories {
                        if let Some(id) = map.get(&c) {
                            filter.insert(id.to_string());
                        }
                    }
                }
            } else {
                for c in bouquet_categories {
                    filter.insert(c);
                }
            }
        }
    }
    if filter.is_empty() {
        None
    } else {
        Some(filter)
    }
}


#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use crate::model::api_proxy::{ProxyType, ProxyUserStatus};
    use super::*;


    #[test]
    pub fn save_target_user() {
        let user =
        TargetUser {
            target: "test".to_string(),
            credentials: vec![
                ProxyUserCredentials {
                    username: "Test".to_string(),
                    password: "Test".to_string(),
                    token: Some("Test".to_string()),
                    proxy: ProxyType::Reverse,
                    server: Some("default".to_string()),
                    epg_timeshift: None,
                    created_at: None,
                    exp_date: Some(1672705545),
                    max_connections: Some(1),
                    status: Some(ProxyUserStatus::Active),
                },
                ProxyUserCredentials {
                    username: "Test2".to_string(),
                    password: "Test".to_string(),
                    token: Some("Test".to_string()),
                    proxy: ProxyType::Reverse,
                    server: Some("default".to_string()),
                    epg_timeshift: None,
                    created_at: None,
                    exp_date: Some(1672705545),
                    max_connections: Some(1),
                    status: Some(ProxyUserStatus::Expired),
                },
                ProxyUserCredentials {
                    username: "Test3".to_string(),
                    password: "Test".to_string(),
                    token: Some("Test".to_string()),
                    proxy: ProxyType::Reverse,
                    server: Some("default".to_string()),
                    epg_timeshift: None,
                    created_at: None,
                    exp_date: Some(1672705545),
                    max_connections: Some(1),
                    status: Some(ProxyUserStatus::Expired),
                },
                ProxyUserCredentials {
                    username: "Test4".to_string(),
                    password: "Test".to_string(),
                    token: Some("Test".to_string()),
                    proxy: ProxyType::Reverse,
                    server: Some("default".to_string()),
                    epg_timeshift: None,
                    created_at: None,
                    exp_date: Some(1672705545),
                    max_connections: Some(1),
                    status: Some(ProxyUserStatus::Expired),
                }
            ]
        };

        let mut cfg = Config::default();
        let target_user = vec![user];
        cfg.t_config_path = temp_dir().to_string_lossy().to_string();
        let _ = store_api_user(&cfg, &target_user);

        let user_list = load_api_user(&cfg);
        assert!(user_list.is_ok());
        assert_eq!(user_list.as_ref().unwrap().len(), 1);
        assert_eq!(user_list.as_ref().unwrap().get(0).unwrap().credentials.len(), 4);
    }
}