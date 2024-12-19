use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{FetchedPlaylist, PlaylistEntry, PlaylistItem, XtreamCluster};
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_info_file_paths};
use crate::repository::IndexedDocumentIndex;
use crate::utils::download;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Error, ErrorKind, Write};


const FILE_SERIES_INFO:&str = "xtream_series_info";
const FILE_VOD_INFO:&str = "xtream_vod_info";
const FILE_SUFFIX_WAL:&str = "wal";

#[macro_export]
macro_rules! create_resolve_options_function_for_xtream_input {
    ($cluster:ident) => {
        paste::paste! { // Paste Makro benötigt!
            fn [<get_resolve_ $cluster _options>](target: &ConfigTarget, fpl: &FetchedPlaylist) -> (bool, u16) {
                let (resolve, resolve_delay) =
                    target.options.as_ref().map_or((false, 0), |opt| {
                        (opt.[<xtream_resolve_ $cluster>] && fpl.input.input_type == InputType::Xtream,
                         opt.[<xtream_resolve_ $cluster _delay>])
                    });
                (resolve, resolve_delay)
            }
        }
    };
}


pub fn get_u32_from_serde_value(value: &Value) -> Option<u32> {
    match value {
        Value::Number(num_val) => num_val.as_u64().and_then(|val| u32::try_from(val).ok()),
        Value::String(str_val) => {
            match str_val.parse::<u32>() {
                Ok(sid) => Some(sid),
                Err(_) => None
            }
        }
        _ => None,
    }
}

pub(in crate::processing) async fn playlist_resolve_process_playlist_item(pli: &PlaylistItem, input: &ConfigInput, errors: &mut Vec<M3uFilterError>, resolve_delay: u16, cluster: XtreamCluster) -> Option<String> {
    let mut result = None;
    let provider_id = pli.get_provider_id().unwrap_or(0);
    if let Some(info_url) = download::get_xtream_player_api_info_url(input, cluster, provider_id) {
        result = match download::get_xtream_stream_info_content(&info_url, input).await {
            Ok(content) => Some(content),
            Err(err) => {
                errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("{err}")));
                None
            }
        };
    }
    if resolve_delay > 0 {
        actix_web::rt::time::sleep(std::time::Duration::new(u64::from(resolve_delay), 0)).await;
    }
    result
}


pub(in crate::processing) async fn read_processed_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>, cluster: XtreamCluster) -> HashSet<u32> {
    let mut processed_info_ids = HashSet::new();
    {
        match get_input_storage_path(fpl.input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, cluster)) {
            Ok(Some((file_path, idx_path))) => {
                match cfg.file_locks.read_lock(&file_path).await {
                    Ok(file_lock) => {
                        if let Ok(info_id_mapping) = IndexedDocumentIndex::<u32>::load(&idx_path) {
                            info_id_mapping.traverse(|keys, _| {
                                for doc_id in keys { processed_info_ids.insert(*doc_id); }
                            });
                        }
                        drop(file_lock);
                    }
                    Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("{err}"))),
                }
            }
            Ok(None) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Could not create storage path for input {}", &fpl.input.name.as_ref().map_or("?", |v| v)))),
            Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Could not create storage path for input {err}"))),
        }
    }
    processed_info_ids
}

pub(in crate::processing) fn write_info_content_to_temp_file(writer: &mut BufWriter<&File>, provider_id: u32, content: &str) -> std::io::Result<()> {
    let length = u32::try_from(content.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
    if length > 0 {
        writer.write_all(&provider_id.to_le_bytes())?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(())
}

pub(in crate::processing) fn create_resolve_info_wal_files(cfg: &Config, input: &ConfigInput, cluster: XtreamCluster) -> Option<(File, File)> {
    match get_input_storage_path(input, &cfg.working_dir) {
        Ok(storage_path) => {
            if let Some(file_prefix) = match cluster {
                XtreamCluster::Live => None,
                XtreamCluster::Video => Some(FILE_SERIES_INFO),
                XtreamCluster::Series => Some(FILE_VOD_INFO)
            } {
                let content_path = storage_path.join(format!("{file_prefix}_content.{FILE_SUFFIX_WAL}"));
                let tmdb_path = storage_path.join(format!("{file_prefix}_tmdb.{FILE_SUFFIX_WAL}"));
                let content_file =  OpenOptions::new().append(true).open(content_path).ok()?;
                let tmdb_file =  OpenOptions::new().append(true).open(tmdb_path).ok()?;
                return Some((content_file, tmdb_file));
            }
            None

        }
        Err(_) => None
    }
}
