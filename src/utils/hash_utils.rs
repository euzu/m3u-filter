use base64::Engine;
use base64::engine::general_purpose;
use crate::model::{PlaylistItemType, UUIDType};
use crate::repository::storage::hash_string;

pub fn extract_id_from_url(url: &str) -> Option<String> {
    if let Some(possible_id_and_ext) = url.split('/').next_back() {
        return possible_id_and_ext.rfind('.').map_or_else(|| Some(possible_id_and_ext.to_string()), |index| Some(possible_id_and_ext[..index].to_string()));
    }
    None
}

pub fn get_provider_id(provider_id: &str, url: &str) -> Option<u32> {
    provider_id.parse::<u32>().ok().or_else(|| {
        extract_id_from_url(url)?.parse::<u32>().ok()
    })
}

pub fn generate_playlist_uuid(key: &str, provider_id: &str, item_type: PlaylistItemType, url: &str) -> UUIDType {
    if let Some(id) = get_provider_id(provider_id, url) {
        if id > 0 {
            return hash_string(&format!("{key}{id}{item_type}"));
        }
    }
    hash_string(url)
}

pub fn u32_to_base64(value: u32) -> String {
    // big-endian is safer and more portable when you care about consistent ordering or cross-platform data
    let bytes = value.to_be_bytes();
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn base64_to_u32(encoded: &str) -> Option<u32> {
    let decoded = general_purpose::URL_SAFE_NO_PAD.decode(encoded).ok()?;

    if decoded.len() != 4 {
        return None;
    }

    let arr: [u8; 4] = decoded
        .as_slice()
        .try_into().ok()?;
    Some(u32::from_be_bytes(arr))
}