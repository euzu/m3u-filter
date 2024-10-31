use std::path::{Path, PathBuf};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::Config;
use crate::utils::file_utils;

pub(crate) fn hash_string(url: &str) -> [u8; 32] {
    let hash = blake3::hash(url.as_bytes());
    hash.into() // convert to hash array
}

pub(in crate::repository) fn get_target_id_mapping_file(target_path: &Path) -> PathBuf {
    target_path.join(PathBuf::from("id_mapping.db"))
}

pub(crate) fn ensure_target_storage_path(cfg: &Config, target_name: &str) -> Result<PathBuf, M3uFilterError> {
    if let Some(path) = get_target_storage_path(cfg, target_name) {
        if std::fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save target data, can't create directory {}", &path.to_str().unwrap());
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg));
        }
        Ok(path)
    } else {
        let msg = format!("Failed to save target data, can't create directory for target {target_name}");
        Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg))
    }
}

pub(crate) fn get_target_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}
