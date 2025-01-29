use crate::model::playlist::UUIDType;
use crate::repository::storage::hash_string;

pub fn extract_id_from_url(url: &str) -> Option<String> {
    if let Some(possible_id_and_ext) = url.split('/').next_back() {
        return possible_id_and_ext.rfind('.').map_or_else(|| Some(possible_id_and_ext.to_string()), |index| Some(possible_id_and_ext[..index].to_string()));
    }
    None
}

pub fn get_provider_id(provider_id: &str, url: &str) -> Option<u32> {
    match provider_id.parse::<u32>() {
        Ok(id) => Some(id),
        Err(_) => match extract_id_from_url(url) {
            Some(id) => match id.parse::<u32>() {
                Ok(newid) => {
                    Some(newid)
                }
                Err(_) => None,
            },
            None => None,
        }
    }
}

pub fn generate_playlist_uuid(key: &str, provider_id: &str, url: &str) -> UUIDType {
    if let Some(id) = get_provider_id(provider_id, url) {
        if id > 0 {
            return hash_string(&format!("{key}{id}"));
        }
    }
    hash_string(url)
}