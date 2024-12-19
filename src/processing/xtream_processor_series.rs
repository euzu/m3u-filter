use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::playlist::{FetchedPlaylist, PlaylistItemType, XtreamCluster};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use crate::create_resolve_options_function_for_xtream_input;
use crate::processing::playlist_processor::ProcessingPipe;
use crate::processing::xtream_processor::{create_resolve_info_wal_files, playlist_resolve_process_playlist_item, read_processed_info_ids, write_info_content_to_temp_file};
use crate::repository::xtream_repository::xtream_update_input_info_file;

const TAG_series_INFO_INFO: &str = "info";
const TAG_series_INFO_MOVIE_DATA: &str = "movie_data";
const TAG_series_INFO_TMDB_ID: &str = "tmdb_id";
const TAG_series_INFO_STREAM_ID: &str = "stream_id";

create_resolve_options_function_for_xtream_input!(series);

fn write_series_info_tmdb_to_temp_file(writer: &mut BufWriter<&File>, provider_id: u32, tmdb_id: u32) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&tmdb_id.to_le_bytes())?;
    Ok(())
}

fn extract_provider_id_and_tmdb_id_from_series_info(content: &str) -> Option<(u32, u32)> {
    if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content) {
        if let Some(Value::Object(movie_data)) = doc.get_mut(TAG_series_INFO_MOVIE_DATA) {
            if let Some(stream_id_value) = movie_data.get(TAG_series_INFO_STREAM_ID) {
                if let Some(stream_id) = crate::processing::xtream_processor::get_u32_from_serde_value(stream_id_value) {
                    if let Some(Value::Object(info)) = doc.get_mut(TAG_series_INFO_INFO) {
                        if let Some(tmdb_id_value) = info.get(TAG_series_INFO_TMDB_ID) {
                            if let Some(tmdb_id) = crate::processing::xtream_processor::get_u32_from_serde_value(tmdb_id_value) {
                                return Some((stream_id, tmdb_id));
                            }
                        }
                    }
                    return Some((stream_id, 0));
                }
            }
        }
    }
    None
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
    let Some((mut wal_file_info, mut wal_file_tmdb)) = create_resolve_info_wal_files(cfg, processed_fpl.input, XtreamCluster::Series) else { return };

    let mut processed_series_ids = read_processed_info_ids(cfg, errors, processed_fpl, XtreamCluster::Series).await;
    let mut info_writer = BufWriter::new(&wal_file_info);
    let mut tmdb_writer = BufWriter::new(&wal_file_tmdb);
    let mut info_updated = false;
    let mut tmdb_updated = false;
    for pli in processed_fpl.playlistgroups.iter()
        .filter(|plg| plg.xtream_cluster == XtreamCluster::Series)
        .flat_map(|plg| &plg.channels)
        .filter(|pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
    {
        let processed_entry = pli.header.borrow_mut().get_provider_id().as_ref().map_or(false, |pid| processed_series_ids.contains(pid));
        if !processed_entry {
            if let Some(content) = playlist_resolve_process_playlist_item(pli, processed_fpl.input, errors, resolve_delay, XtreamCluster::Series).await {
                if let Some((provider_id, tmdb_id)) = extract_provider_id_and_tmdb_id_from_series_info(&content) {
                    if let Err(err) = write_info_content_to_temp_file(&mut info_writer, provider_id, &content) {
                        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series, could not write to temporary file {err}")));
                        return;
                    }
                    info_updated = true;
                    processed_series_ids.insert(provider_id);
                    if tmdb_id > 0 {
                        if let Err(err) = write_series_info_tmdb_to_temp_file(&mut tmdb_writer, provider_id, tmdb_id) {
                            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series tmdb, could not write to temporary file {err}")));
                            return;
                        }
                        tmdb_updated = true;
                    }
                }
            }
        }
    }
    if info_updated {
        if let Err(err) = info_writer.flush() {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series, could not write to temporary file {err}")));
        }
        drop(info_writer);
        if let Err(err) = xtream_update_input_info_file(cfg, processed_fpl.input, &mut wal_file_info, XtreamCluster::Series).await {
            errors.push(err);
        }
    }
    if tmdb_updated {
        if let Err(err) = tmdb_writer.flush() {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve series tmdb, could not write to temporary file {err}")));
        }
        drop(tmdb_writer);
        if let Err(err) = xtream_update_input_series_tmdb_file(cfg, processed_fpl.input, &mut wal_file_tmdb).await {
            errors.push(err);
        }
    }
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