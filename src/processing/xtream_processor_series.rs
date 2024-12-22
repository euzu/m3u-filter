use std::collections::HashMap;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::playlist::{FetchedPlaylist, PlaylistItem, PlaylistItemType, XtreamCluster};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use crate::{create_resolve_options_function_for_xtream_input, handle_error, handle_error_and_return};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::processing::xtream_processor::{create_resolve_info_wal_files, get_u32_from_serde_value,
                                          get_u64_from_serde_value, playlist_resolve_process_playlist_item,
                                          should_update_info, write_info_content_to_wal_file};
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_record_file_path, xtream_update_input_info_file, xtream_update_input_record_from_wal_file};

const TAG_SERIES_INFO_SERIES_ID: &str = "series_id";
const TAG_SERIES_INFO_LAST_MODIFIED: &str = "last_modified";

create_resolve_options_function_for_xtream_input!(series);

fn write_series_info_tmdb_to_wal_file(writer: &mut BufWriter<&File>, provider_id: u32, tmdb_id: u32) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&tmdb_id.to_le_bytes())?;
    Ok(())
}

async fn read_processed_series_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>,
                                     cluster: XtreamCluster) -> HashMap<u32, u64> {
    let mut processed_info_ids = HashMap::new();
    {
        match get_input_storage_path(fpl.input, &cfg.working_dir)
            .map(|storage_path| xtream_get_record_file_path(&storage_path, cluster)).await {
            Ok(file_path) => {
                match cfg.file_locks.read_lock(&file_path).await {
                    Ok(file_lock) => {
                        if let Ok(info_records) = BPlusTree::<u32, u64>::load(&file_path) {
                            info_records.traverse(|keys, timestamps| {
                                for (provider_id, ts) in keys.iter().zip(timestamps.iter()) {
                                    processed_info_ids.insert(*provider_id, *ts);
                                }
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

fn extract_info_record_from_series_info(content: &str) -> Option<(u32, u64)> {
    let doc = serde_json::from_str::<Map<String, Value>>(content).ok()?;
    let provider_id = get_u32_from_serde_value(doc.get(TAG_SERIES_INFO_SERIES_ID)?)?;
    let last_modified = get_u64_from_serde_value(doc.get(TAG_SERIES_INFO_LAST_MODIFIED)?)
        .unwrap_or(0);
    Some((provider_id, last_modified))
}

fn should_update_series_info(pli: &PlaylistItem, processed_provider_ids: &HashMap<u32, u64>) -> bool {
    should_update_info(pli, processed_provider_ids, "last_modified")
}

pub async fn playlist_resolve_series(cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>,
                                     pipe: &ProcessingPipe,
                                     provider_fpl: &mut FetchedPlaylist<'_>,
                                     processed_fpl: &mut FetchedPlaylist<'_>
) {
    let (resolve_series, resolve_delay) = get_resolve_series_options(target, processed_fpl);
    if !resolve_series { return; }

    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a wal file and write it once we collected everything.
    let Some((mut wal_file_content, mut wal_file_record)) = create_resolve_info_wal_files(cfg, processed_fpl.input, XtreamCluster::Series)
    else { return; };

    let mut processed_info_ids = read_processed_series_info_ids(cfg, errors, processed_fpl, XtreamCluster::Series).await;
    let mut content_writer = BufWriter::new(&wal_file_content);
    let mut record_writer = BufWriter::new(&wal_file_record);
    let mut content_updated = false;

    for pli in processed_fpl.playlistgroups.iter()
        .filter(|&plg| plg.xtream_cluster == XtreamCluster::Series)
        .flat_map(|plg| &plg.channels)
        .filter(|&pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
        .filter(|&pli| should_update_series_info(pli, &processed_info_ids))
    {
        // content contains all episodes
        if let Some(content) = playlist_resolve_process_playlist_item(pli, processed_fpl.input, errors, resolve_delay, XtreamCluster::Series).await {
            if let Some((provider_id, ts)) = extract_info_record_from_series_info(&content) {
                handle_error_and_return!(write_info_content_to_wal_file(&mut content_writer, provider_id, &content), |err| M3uFilterError::new( M3uFilterErrorKind::Notify, format!("Failed to resolve series, could not write to content wal file {err}")));
                processed_info_ids.insert(provider_id, ts);
                handle_error_and_return!(write_info_record_to_wal_file(&mut record_writer, provider_id, &info_record), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series wal, could not write to record wal file {err}")));
                content_updated = true;
            }
        }
    }
    if content_updated {
        handle_error!(content_writer.flush(), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to wal file {err}")));
        drop(content_writer);
        handle_error!(record_writer.flush(), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to wal file {err}")));
        drop(record_writer);
        handle_error!(xtream_update_input_info_file(cfg, processed_fpl.input, &mut wal_file_content, XtreamCluster::Series).await);
        handle_error!(xtream_update_input_record_from_wal_file(cfg, processed_fpl.input, &mut wal_file_record, XtreamCluster::Series).await);
    }


    Now update provider_fpl

}

//
// pub(in crate::processing)  async fn playlist_resolve_series(target: &ConfigTarget, errors: &mut Vec<M3uFilterError>,
//                                                             pipe: &ProcessingPipe,
//                                                             provider_fpl: &mut FetchedPlaylist<'_>,
//                                                             processed_fpl: &mut FetchedPlaylist<'_>) {
//     let (resolve_series, resolve_series_delay) =
//         if let Some(options) = &target.options {
//             (options.xtream_resolve_series && provider_fpl.input.input_type == InputType::Xtream && target.has_output(&TargetType::M3u),
//              options.xtream_resolve_series_delay)
//         } else {
//             (false, 0)
//         };
//     if resolve_series {
//         // collect all series in the processed lists
//         let to_process_uuids: HashSet<Rc<UUIDType>> = processed_fpl.playlistgroups.iter()
//             .filter(|plg| plg.xtream_cluster == XtreamCluster::Series)
//             .flat_map(|plg| &plg.channels)
//             .filter(|pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
//             .map(|pli| Rc::clone(&pli.header.borrow().uuid)).collect();
//         let mut series_playlist = download::get_xtream_playlist_series(provider_fpl, to_process_uuids, errors, resolve_series_delay).await;
//         // original content saved into original list
//         for plg in &series_playlist {
//             provider_fpl.update_playlist(plg);
//         }
//         // run processing pipe over new items
//         for f in pipe {
//             let r = f(&mut series_playlist, target);
//             if let Some(v) = r {
//                 series_playlist = v;
//             }
//         }
//         // assign new items to the new playlist
//         for plg in &series_playlist {
//             processed_fpl.update_playlist(plg);
//         }
//     }
// }