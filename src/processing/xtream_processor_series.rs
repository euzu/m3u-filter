use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::playlist::{FetchedPlaylist, PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::processing::xtream_processor::{create_resolve_info_wal_files, get_u32_from_serde_value, get_u64_from_serde_value, playlist_resolve_process_playlist_item, read_processed_info_ids, should_update_info, write_info_content_to_wal_file};
use crate::repository::xtream_repository::{xtream_get_info_file_paths, xtream_update_input_info_file, xtream_update_input_series_record_from_wal_file};
use crate::{create_resolve_options_function_for_xtream_target, handle_error, handle_error_and_return};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use log::error;
use crate::repository::bplustree::BPlusTree;
use crate::repository::IndexedDocumentIndex;
use crate::repository::storage::get_input_storage_path;

const TAG_SERIES_INFO_SERIES_ID: &str = "series_id";
const TAG_SERIES_INFO_LAST_MODIFIED: &str = "last_modified";

create_resolve_options_function_for_xtream_target!(series);

fn write_series_info_tmdb_to_wal_file(writer: &mut BufWriter<&File>, provider_id: u32, tmdb_id: u32) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&tmdb_id.to_le_bytes())?;
    Ok(())
}

async fn read_processed_series_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>,
                                        cluster: XtreamCluster) -> HashMap<u32, u64> {
    read_processed_info_ids(cfg, errors, fpl, cluster, |ts: &u64| *ts).await
}

fn extract_info_record_from_series_info(content: &str) -> Option<(u32, u64)> {
    let doc = serde_json::from_str::<Map<String, Value>>(content).ok()?;
    let provider_id = get_u32_from_serde_value(doc.get(TAG_SERIES_INFO_SERIES_ID)?)?;
    let last_modified = get_u64_from_serde_value(doc.get(TAG_SERIES_INFO_LAST_MODIFIED)?).unwrap_or(0);
    Some((provider_id, last_modified))
}

fn write_series_info_record_to_wal_file(
    writer: &mut BufWriter<&File>,
    provider_id: u32,
    ts: u64,
) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&ts.to_le_bytes())?;
    Ok(())
}

fn should_update_series_info(pli: &PlaylistItem, processed_provider_ids: &HashMap<u32, u64>) -> bool {
    should_update_info(pli, processed_provider_ids, "last_modified")
}

async fn playlist_resolve_series_info(cfg: &Config, errors: &mut Vec<M3uFilterError>,
                                      processed_fpl: &mut FetchedPlaylist<'_>, resolve_delay: u16) -> HashMap<u32, u64>{
    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a wal file and write it once we collected everything.
    let Some((mut wal_content_file, mut wal_record_file, wal_content_path, wal_record_path)) = create_resolve_info_wal_files(cfg, processed_fpl.input, XtreamCluster::Series)
    else { return HashMap::new(); };

    let mut processed_info_ids = read_processed_series_info_ids(cfg, errors, processed_fpl, XtreamCluster::Series).await;
    let mut content_writer = BufWriter::new(&wal_content_file);
    let mut record_writer = BufWriter::new(&wal_record_file);
    let mut content_updated = false;

    for pli in processed_fpl.playlistgroups.iter()
        .filter(|&plg| plg.xtream_cluster == XtreamCluster::Series)
        .flat_map(|plg| &plg.channels)
        .filter(|&pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
    {
        let should_update = should_update_series_info(pli, &processed_info_ids);
        if should_update {
            if let Some(content) = playlist_resolve_process_playlist_item(pli, processed_fpl.input, errors, resolve_delay, XtreamCluster::Series).await {
                if let Some((provider_id, ts)) = extract_info_record_from_series_info(&content) {
                    handle_error_and_return!(write_info_content_to_wal_file(&mut content_writer, provider_id, &content), |err| M3uFilterError::new( M3uFilterErrorKind::Notify, format!("Failed to resolve series, could not write to content wal file {err}")));
                    processed_info_ids.insert(provider_id, ts);
                    handle_error_and_return!(write_series_info_record_to_wal_file(&mut record_writer, provider_id, ts), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series wal, could not write to record wal file {err}")));
                    content_updated = true;
                }
            }
        }
    }
    if content_updated {
        handle_error!(content_writer.flush(), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to wal file {err}")));
        drop(content_writer);
        handle_error!(record_writer.flush(), |err| M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to wal file {err}")));
        drop(record_writer);
        handle_error!(xtream_update_input_info_file(cfg, processed_fpl.input, &mut wal_content_file, &wal_content_path, XtreamCluster::Series).await, |err| errors.push(err));
        handle_error!(xtream_update_input_series_record_from_wal_file(cfg, processed_fpl.input, &mut wal_record_file, &wal_record_path).await, |err| errors.push(err));
    }

    processed_info_ids
}

pub async fn playlist_resolve_series(cfg: &Config, target: &ConfigTarget,
                                     errors: &mut Vec<M3uFilterError>,
                                     pipe: &ProcessingPipe,
                                     provider_fpl: &mut FetchedPlaylist<'_>,
                                     processed_fpl: &mut FetchedPlaylist<'_>,
) {
    let (resolve_series, resolve_delay) = get_resolve_series_options(target, processed_fpl);
    if !resolve_series { return; }

    let processed_ids = playlist_resolve_series_info(cfg, errors, processed_fpl, resolve_delay).await;
    if processed_ids.is_empty() { return; }

    if let Ok(Some((info_path, idx_path))) =  get_input_storage_path(provider_fpl.input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, XtreamCluster::Series)) {
        match cfg.file_locks.read_lock(&info_path).await {
            Ok(_file_lock) => {
                let index = IndexedDocumentIndex::<u32>::load(&idx_path).unwrap_or_else(|err| {
                    error!("Failed to load index {idx_path:?}: {err}");
                    IndexedDocumentIndex::<u32>::new()
                });
                let mut result: Vec<PlaylistGroup> = vec![];
                for plg in &mut provider_fpl.playlistgroups.iter()
                    .filter(|&plg| plg.xtream_cluster == XtreamCluster::Series)
                {
                    let mut group_series: Vec<PlaylistItem> = vec![];
                    for pli in plg.channels.iter().filter(|&pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo) {
                        if let Some(provider_id) = pli.header.borrow_mut().get_provider_id() {
                            if processed_ids.contains_key(&provider_id) {
                                if let Some(offset) = index.query(&provider_id) {
                                    IndexedDocumentReader::
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, "Could not lock input info file for series".to_string()))
        }
    } else {
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, "Failed to open input info file for series".to_string()));
    }


    // Now update provider_fpl


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