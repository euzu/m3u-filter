use std::path::PathBuf;
use crate::model::config::Config;
use crate::model::playlist_categories::PlaylistCategoriesDto;
use crate::utils::file::file_utils;
use crate::utils::json_utils::json_write_documents_to_file;

const USER_BOUQUET: &str = "bouquet.json";

pub fn get_user_storage_path(cfg: &Config, username: &str) -> Option<PathBuf> {
    cfg.user_config_dir.as_ref().and_then(|ucd| file_utils::get_file_path(&ucd, Some(std::path::PathBuf::from(username))))
}

pub fn user_get_bouquet_path(cfg: &Config, username: &str) -> Option<PathBuf> {
    get_user_storage_path(cfg, username).map(|user_path| user_path.join(PathBuf::from(USER_BOUQUET)))
}

pub async fn save_user_bouquet(cfg: &Config, username: &str, bouquet: &PlaylistCategoriesDto) -> Result<(), std::io::Error> {
    if let Some(file_path) = user_get_bouquet_path(cfg, username) {

        if !file_path.exists() {
            if let Some(parent) = file_path.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    let msg = format!("Failed to create user config dir, can't create directory {}",&file_path.to_str().unwrap());
                    return Err(std::io::Error::new(std::io::ErrorKind::NotFound, msg));
                }
            }
        }
        return json_write_documents_to_file(&file_path, bouquet);
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("User config oath not found for user {username}")))
}
