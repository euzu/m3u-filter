use crate::model::playlist::{PlaylistItemType, UUIDType};
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
