use std::path::{Path, PathBuf};
use std::fmt::Write;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput};
use crate::utils::file_utils;

pub(in crate::repository) const FILE_SUFFIX_DB: &str = "db";
pub(in crate::repository) const FILE_SUFFIX_INDEX: &str = "idx";

const FILE_ID_MAPPING: &str = "id_mapping.db";

pub(crate) fn hash_string(url: &str) -> [u8; 32] {
    let hash = blake3::hash(url.as_bytes());
    hash.into() // convert to hash array
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02X}");
        output
    })
}

pub(crate) fn hash_string_as_hex(url: &str) -> String {
    hex_encode(&hash_string(url))
}

pub(in crate::repository) fn get_target_id_mapping_file(target_path: &Path) -> PathBuf {
    target_path.join(PathBuf::from(FILE_ID_MAPPING))
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

pub(crate) fn get_input_storage_path(input: &ConfigInput, working_dir: &str) -> std::io::Result<PathBuf> {
    let name =  format!("input_{}", input.name.clone().unwrap_or_else(|| format!("{}", input.id)));
    let path = Path::new(working_dir).join(name);

    // Create the directory and return the path or propagate the error
    std::fs::create_dir_all(&path).map(|()| path)
}
