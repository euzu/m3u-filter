use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{PlaylistEntry, PlaylistItem, XtreamCluster};
use crate::repository::storage::get_input_storage_path;
use crate::utils::download;
use serde_json::Value;
use std::collections::{HashMap};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Error, ErrorKind, Write};

const FILE_SERIES_INFO:&str = "xtream_series_info";
const FILE_VOD_INFO:&str = "xtream_vod_info";
const FILE_SUFFIX_WAL:&str = "wal";

#[macro_export]
macro_rules! handle_error {
    ($stmt:expr) => {
        if let Err(err) = $stmt {
            errors.push(err);
        }
    };

    ($stmt:expr, $map_err:expr) => {
        if let Err(err) = $stmt {
            errors.push($map_err(err));
        }
    };
}
pub(in crate::processing) use handle_error;

#[macro_export]
macro_rules! handle_error_and_return {
    ($stmt:expr, $map_err:expr) => {
        if let Err(err) = $stmt {
            errors.push($map_err(err));
            return;
        }
    };
}
pub(in crate::processing) use handle_error_and_return;

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


pub fn get_u64_from_serde_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(num_val) => num_val.as_u64(),
        Value::String(str_val) => {
            match str_val.parse::<u64>() {
                Ok(val) => Some(val),
                Err(_) => None
            }
        }
        _ => None,
    }
}

pub fn get_u32_from_serde_value(value: &Value) -> Option<u32> {
    get_u64_from_serde_value(value).and_then(|val| u32::try_from(val).ok())
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

pub(in crate::processing) fn write_info_content_to_wal_file(writer: &mut BufWriter<&File>, provider_id: u32, content: &str) -> std::io::Result<()> {
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
                let info_path = storage_path.join(format!("{file_prefix}_record.{FILE_SUFFIX_WAL}"));
                let content_file =  OpenOptions::new().append(true).open(content_path).ok()?;
                let info_file =  OpenOptions::new().append(true).open(info_path).ok()?;
                return Some((content_file, info_file));
            }
            None

        }
        Err(_) => None
    }
}


pub(in crate::processing) fn has_different_ts(ts: &u64, pli: &PlaylistItem, field: &str) -> bool {
    pli.header
        .borrow()
        .additional_properties
        .as_ref()
        .map_or(false, |v| match v {
            Value::Object(map) => {
                if let Some(updated) = map.get(field) {
                    if let Some(update_ts) = get_u64_from_serde_value(updated) {
                        return update_ts != *ts;
                    }
                }
                true
            }
            _ => true,
        })
}

pub(in crate::processing) fn should_update_info(pli: &PlaylistItem, processed_provider_ids: &HashMap<u32, u64>, field: &str) -> bool {
    if let Some(provider_id) = pli.header.borrow_mut().get_provider_id() {
        let timestamp = processed_provider_ids.get(&provider_id);
        timestamp.is_none() || has_different_ts(timestamp.unwrap(), pli, field)
    } else {
        false
    }
}
