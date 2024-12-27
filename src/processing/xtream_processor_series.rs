use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::playlist::{FetchedPlaylist, PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::processing::xtream_parser::parse_xtream_series_info;
use crate::processing::xtream_processor::{create_resolve_episode_wal_files, create_resolve_info_wal_files, playlist_resolve_download_playlist_item, read_processed_info_ids, should_update_info, write_info_content_to_wal_file};
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_info_file_paths, xtream_update_input_info_file, xtream_update_input_series_episodes_record_from_wal_file, xtream_update_input_series_record_from_wal_file};
use crate::repository::IndexedDocumentReader;
use crate::{create_resolve_options_function_for_xtream_target, handle_error, handle_error_and_return, info_err, notify_err};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use crate::model::xtream::{XtreamSeriesEpisode, XtreamSeriesInfoEpisode};

const TAG_SERIES_INFO_LAST_MODIFIED: &str = "last_modified";

create_resolve_options_function_for_xtream_target!(series);

async fn read_processed_series_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) -> HashMap<u32, u64> {
    read_processed_info_ids(cfg, errors, fpl, PlaylistItemType::SeriesInfo, |ts: &u64| *ts).await
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

fn write_series_episode_record_to_wal_file(
    writer: &mut BufWriter<&File>,
    provider_id: u32,
    episode: &XtreamSeriesInfoEpisode,
) -> std::io::Result<()> {
    let series_episode = XtreamSeriesEpisode::from(episode);
    if let Ok(content_bytes) = bincode::serialize(&series_episode) {
        writer.write_all(&provider_id.to_le_bytes())?;
        let len = u32::try_from(content_bytes.len()).unwrap();
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&content_bytes)?;
    }
    Ok(())
}

fn should_update_series_info(pli: &PlaylistItem, processed_provider_ids: &HashMap<u32, u64>) -> (bool, u32, u64) {
    should_update_info(pli, processed_provider_ids, TAG_SERIES_INFO_LAST_MODIFIED)
}

async fn playlist_resolve_series_info(cfg: &Config, errors: &mut Vec<M3uFilterError>,
                                      processed_fpl: &mut FetchedPlaylist<'_>, resolve_delay: u16) -> bool {
    let mut processed_info_ids = read_processed_series_info_ids(cfg, errors, processed_fpl).await;
    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a wal file and write it once we collected everything.
    let Some((wal_content_file, wal_record_file, wal_content_path, wal_record_path)) = create_resolve_info_wal_files(cfg, processed_fpl.input, XtreamCluster::Series)
    else { return !processed_info_ids.is_empty(); };

    let mut content_writer = BufWriter::new(&wal_content_file);
    let mut record_writer = BufWriter::new(&wal_record_file);
    let mut content_updated = false;

    for pli in processed_fpl.playlistgroups.iter()
        .filter(|&plg| plg.xtream_cluster == XtreamCluster::Series)
        .flat_map(|plg| &plg.channels)
        .filter(|&pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
    {
        let (should_update, provider_id, ts) = should_update_series_info(pli, &processed_info_ids);
        if should_update {
            if let Some(content) = playlist_resolve_download_playlist_item(pli, processed_fpl.input, errors, resolve_delay, XtreamCluster::Series).await {
                handle_error_and_return!(write_info_content_to_wal_file(&mut content_writer, provider_id, &content),
                    |err| errors.push(notify_err!(format!("Failed to resolve series, could not write to content wal file {err}"))));
                processed_info_ids.insert(provider_id, ts);
                handle_error_and_return!(write_series_info_record_to_wal_file(&mut record_writer, provider_id, ts),
                    |err| errors.push(notify_err!(format!("Failed to resolve series wal, could not write to record wal file {err}"))));
                content_updated = true;
            }
        }
    }
    // content_wal contains the provider_id and series_info with episode listing
    // record_wal contains provider_id and timestamp
    if content_updated {
        handle_error!(content_writer.flush(),
            |err| errors.push(notify_err!(format!("Failed to resolve vod, could not write to wal file {err}"))));
        drop(content_writer);
        drop(wal_content_file);
        handle_error!(record_writer.flush(),
            |err| errors.push(notify_err!(format!("Failed to resolve vod tmdb, could not write to wal file {err}"))));
        drop(record_writer);
        drop(wal_record_file);
        handle_error!(xtream_update_input_info_file(cfg, processed_fpl.input, &wal_content_path, XtreamCluster::Series).await,
            |err| errors.push(err));
        handle_error!(xtream_update_input_series_record_from_wal_file(cfg, processed_fpl.input, &wal_record_path).await,
            |err| errors.push(err));
    }

    // we updated now
    // - series_info.db  which contains the original series_info json
    // - series_record.db which contains the series_info provider_id and timestamp
    !processed_info_ids.is_empty()
}
async fn process_series_info(
    cfg: &Config,
    provider_fpl: &mut FetchedPlaylist<'_>,
    errors: &mut Vec<M3uFilterError>,
) -> Vec<PlaylistGroup> {
    let mut result: Vec<PlaylistGroup> = vec![];
    let input = provider_fpl.input;

    let Ok(Some((info_path, idx_path))) = get_input_storage_path(input, &cfg.working_dir)
        .map(|storage_path| xtream_get_info_file_paths(&storage_path, XtreamCluster::Series))
    else {
        errors.push(notify_err!("Failed to open input info file for series".to_string()));
        return result;
    };

    let Ok(_file_lock) = cfg.file_locks.read_lock(&info_path).await else {
        errors.push(notify_err!("Could not lock input info file for series".to_string()));
        return result;
    };

    // Contains the Series Info with episode listing
    let Ok(mut info_reader) = IndexedDocumentReader::<u32, String>::new(&info_path, &idx_path) else { return result; };

    let Some((wal_file, wal_path)) = create_resolve_episode_wal_files(cfg, input) else {
        errors.push(notify_err!("Could not create wal file for series episodes record".to_string()));
        return result;
    };
    let mut wal_writer = BufWriter::new(&wal_file);

    for plg in provider_fpl
        .playlistgroups
        .iter_mut()
        .filter(|plg| plg.xtream_cluster == XtreamCluster::Series)
    {
        let mut group_series = vec![];

        for pli in plg
            .channels
            .iter()
            .filter(|pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
        {
            let Some(provider_id) = pli.header.borrow_mut().get_provider_id() else { continue; };
            let Ok(content) = info_reader.get(&provider_id)  else { continue; };
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(series_content) => {
                    match parse_xtream_series_info(&series_content, pli.header.borrow().group.as_str(), input) {
                        Ok(Some(series)) => {
                            for (episode, pli_episode) in &series {
                                let Some(provider_id) = &pli_episode.header.borrow_mut().get_provider_id() else { continue; };
                                handle_error!(write_series_episode_record_to_wal_file(&mut wal_writer, *provider_id, episode),
                                |err| errors.push(info_err!(format!("Failed to write to series episode wal file: {err}"))));
                            }
                            group_series.extend(series.into_iter().map(|(_, pli)| pli));
                        }
                        Ok(None) => {}
                        Err(err) => {
                            errors.push(err);
                        }
                    }
                }
                Err(err) => errors.push(info_err!(format!("Failed to parse JSON: {err}"))),
            }
        }
        if !group_series.is_empty() {
            result.push(PlaylistGroup {
                id: plg.id,
                title: plg.title.clone(),
                channels: group_series,
                xtream_cluster: XtreamCluster::Series,
            });
        }
    }

    handle_error!(wal_writer.flush(),
            |err| errors.push(notify_err!(format!("Failed to resolve series episodes, could not write to wal file {err}"))));
    drop(wal_writer);
    drop(wal_file);
    handle_error!(xtream_update_input_series_episodes_record_from_wal_file(cfg, input, &wal_path).await,
            |err| errors.push(err));
    result
}


pub async fn playlist_resolve_series(cfg: &Config, target: &ConfigTarget,
                                     errors: &mut Vec<M3uFilterError>,
                                     pipe: &ProcessingPipe,
                                     provider_fpl: &mut FetchedPlaylist<'_>,
                                     processed_fpl: &mut FetchedPlaylist<'_>,
) {
    let (resolve_series, resolve_delay) = get_resolve_series_options(target, processed_fpl);
    if !resolve_series { return; }

    if !playlist_resolve_series_info(cfg, errors, processed_fpl, resolve_delay).await { return; }
    let mut series_playlist = process_series_info(cfg, provider_fpl, errors).await;
    if series_playlist.is_empty() { return; }
    // original content saved into original list
    for plg in &series_playlist {
        provider_fpl.update_playlist(plg);
    }
    // run processing pipe over new items
    for f in pipe {
        let r = f(&mut series_playlist, target);
        if let Some(v) = r {
            series_playlist = v;
        }
    }
    // assign new items to the new playlist
    for plg in &series_playlist {
        processed_fpl.update_playlist(plg);
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