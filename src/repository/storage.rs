use std::path::{Path, PathBuf};
use std::fmt::Write;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config};
use crate::model::playlist::UUIDType;
use crate::m3u_filter_error::{notify_err};
use crate::utils::file::file_utils;

pub(in crate::repository) const FILE_SUFFIX_DB: &str = "db";
pub(in crate::repository) const FILE_SUFFIX_INDEX: &str = "idx";

const FILE_ID_MAPPING: &str = "id_mapping.db";

#[inline]
pub fn hash_bytes(bytes: &[u8]) -> UUIDType {
    blake3::hash(bytes).into()
}

/// generates a hash from a string
#[inline]
pub fn hash_string(text: &str) -> UUIDType {
    hash_bytes(text.as_bytes())
}


#[inline]
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02X}");
        output
    })
}

pub fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("hex string must have even length".to_string());
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i+2], 16)
                .map_err(|e| format!("invalid hex at position {i}: {e}"))
        })
        .collect()
}

pub fn hash_string_as_hex(url: &str) -> String {
    hex_encode(&hash_string(url))
}

pub(in crate::repository) fn get_target_id_mapping_file(target_path: &Path) -> PathBuf {
    target_path.join(PathBuf::from(FILE_ID_MAPPING))
}

pub fn ensure_target_storage_path(cfg: &Config, target_name: &str) -> Result<PathBuf, M3uFilterError> {
    if let Some(path) = get_target_storage_path(cfg, target_name) {
        if std::fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save target data, can't create directory {path:?}");
            return Err(notify_err!(msg));
        }
        Ok(path)
    } else {
        let msg = format!("Failed to save target data, can't create directory for target {target_name}");
        Err(notify_err!(msg))
    }
}

pub fn get_target_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}

pub fn get_input_storage_path(input_name: &str, working_dir: &str) -> std::io::Result<PathBuf> {
    let name =  format!("input_{input_name}");
    let path = Path::new(working_dir).join(name);
    // Create the directory and return the path or propagate the error
    std::fs::create_dir_all(&path).map(|()| path)
}
