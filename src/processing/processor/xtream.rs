use crate::tuliprox_error::{info_err, notify_err};
use crate::tuliprox_error::{str_to_io_error, to_io_error, TuliProxError, TuliProxErrorKind};
use crate::model::{Config, ConfigInput};
use crate::model::{FetchedPlaylist, PlaylistEntry, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::repository::storage::get_input_storage_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage_const;
use crate::repository::xtream_repository::xtream_get_record_file_path;
use crate::utils::file_utils::append_or_crate_file;
use crate::utils::xtream;

pub(in crate::processing) async fn playlist_resolve_download_playlist_item(client: Arc<reqwest::Client>, pli: &PlaylistItem, input: &ConfigInput, errors: &mut Vec<TuliProxError>, resolve_delay: u16, cluster: XtreamCluster) -> Option<String> {
    let mut result = None;
    let provider_id = pli.get_provider_id()?;
    if let Some(info_url) = xtream::get_xtream_player_api_info_url(input, cluster, provider_id) {
        result = match xtream::get_xtream_stream_info_content(client, &info_url, input).await {
            Ok(content) => Some(content),
            Err(err) => {
                errors.push(info_err!(format!("{err}")));
                None
            }
        };
    }
    if resolve_delay > 0 {
        tokio::time::sleep(std::time::Duration::new(u64::from(resolve_delay), 0)).await;
    }
    result
}

pub(in crate::processing) fn write_info_content_to_wal_file(writer: &mut BufWriter<&File>, provider_id: u32, content: &str) -> std::io::Result<()> {
    let length = u32::try_from(content.len()).map_err(to_io_error)?;
    if length > 0 {
        writer.write_all(&provider_id.to_le_bytes())?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(())
}

pub(in crate::processing) fn create_resolve_episode_wal_files(cfg: &Config, input: &ConfigInput) -> Option<(File, PathBuf)> {
    match get_input_storage_path(&input.name, &cfg.working_dir) {
        Ok(storage_path) => {
            let info_path = storage_path.join(format!("{}.{}", crate::model::XC_FILE_SERIES_EPISODE_RECORD, storage_const::FILE_SUFFIX_WAL));
            let info_file = append_or_crate_file(&info_path).ok()?;
            Some((info_file, info_path))
        }
        Err(_) => None
    }
}

pub(in crate::processing) fn create_resolve_info_wal_files(cfg: &Config, input: &ConfigInput, cluster: XtreamCluster) -> Option<(File, File, PathBuf, PathBuf)> {
    match get_input_storage_path(&input.name, &cfg.working_dir) {
        Ok(storage_path) => {
            if let Some(file_prefix) = match cluster {
                XtreamCluster::Live => None,
                XtreamCluster::Video => Some(crate::model::XC_FILE_VOD_INFO),
                XtreamCluster::Series => Some(crate::model::XC_FILE_SERIES_INFO)
            } {
                let content_path = storage_path.join(format!("{file_prefix}_content.{}", storage_const::FILE_SUFFIX_WAL));
                let info_path = storage_path.join(format!("{file_prefix}_record.{}", storage_const::FILE_SUFFIX_WAL));
                let content_file = append_or_crate_file(&content_path).ok()?;
                let info_file = append_or_crate_file(&info_path).ok()?;
                return Some((content_file, info_file, content_path, info_path));
            }
            None
        }
        Err(_) => None
    }
}

pub(in crate::processing) fn should_update_info(pli: &mut PlaylistItem, processed_provider_ids: &HashMap<u32, u64>, field: &str) -> (bool, u32, u64) {
    let Some(provider_id) = pli.header.get_provider_id() else { return (false, 0, 0) };
    let last_modified = pli.header.get_additional_property_as_u64(field);
    let old_timestamp = processed_provider_ids.get(&provider_id);
    (old_timestamp.is_none()
         || last_modified.is_none()
         || *old_timestamp.unwrap() != last_modified.unwrap(), provider_id, last_modified.unwrap_or(0))
}

pub(in crate::processing) async fn read_processed_info_ids<V, F>(cfg: &Config, errors: &mut Vec<TuliProxError>, fpl: &FetchedPlaylist<'_>,
                                                                 item_type: PlaylistItemType, extract_ts: F) -> HashMap<u32, u64>
where
    F: Fn(&V) -> u64,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    let mut processed_info_ids = HashMap::new();

    let fpl_name = &fpl.input.name;
    let file_path = match get_input_storage_path(fpl_name, &cfg.working_dir)
        .map(|storage_path| xtream_get_record_file_path(&storage_path, item_type)).and_then(|opt| opt.ok_or_else(|| str_to_io_error("Not supported")))
    {
        Ok(file_path) => file_path,
        Err(err) => {
            errors.push(notify_err!(format!("Could not create storage path for input {fpl_name}: {err}")));
            return processed_info_ids;
        }
    };

    {
        let file_lock = cfg.file_locks.read_lock(&file_path);
        if let Ok(info_records) = BPlusTree::<u32, V>::load(&file_path) {
            info_records.iter().for_each(|(provider_id, record)| {
                processed_info_ids.insert(*provider_id, extract_ts(record));
            });
        }
        drop(file_lock);
    }
    processed_info_ids
}
