use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::{Config, ConfigTarget, InputType};
use crate::model::{FetchedPlaylist, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::processing::processor::xtream::{create_resolve_info_wal_files, playlist_resolve_download_playlist_item, read_processed_info_ids, should_update_info, write_info_content_to_wal_file};
use crate::repository::xtream_repository::{xtream_update_input_info_file, xtream_update_input_vod_record_from_wal_file, InputVodInfoRecord};
use crate::m3u_filter_error::{notify_err};
use crate::processing::processor::{handle_error, handle_error_and_return, create_resolve_options_function_for_xtream_target};
use crate::utils::{get_u32_from_serde_value, get_u64_from_serde_value};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Arc;
use std::time::Instant;
use log::{info, log_enabled, Level};
use crate::utils::file_utils::file_writer;

create_resolve_options_function_for_xtream_target!(vod);

async fn read_processed_vod_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) -> HashMap<u32, u64> {
    read_processed_info_ids(cfg, errors, fpl, PlaylistItemType::Video, |record: &InputVodInfoRecord| record.ts).await
}

fn extract_info_record_from_vod_info(content: &str) -> Option<(u32, InputVodInfoRecord)> {
    let doc = serde_json::from_str::<Map<String, Value>>(content).ok()?;

    let movie_data = doc.get(crate::model::XC_TAG_VOD_INFO_MOVIE_DATA)?.as_object()?;
    let provider_id = get_u32_from_serde_value(
        movie_data.get(crate::model::XC_TAG_VOD_INFO_STREAM_ID)?,
    )?;

    let added = movie_data
        .get(crate::model::XC_TAG_VOD_INFO_ADDED)
        .and_then(get_u64_from_serde_value)
        .unwrap_or(0);

    let tmdb_id = doc.get(crate::model::XC_TAG_VOD_INFO_INFO)?.as_object()
        .and_then(|info| info.get(crate::model::XC_TAG_VOD_INFO_TMDB_ID))
        .and_then(get_u32_from_serde_value)
        .unwrap_or(0);

    Some((provider_id, InputVodInfoRecord {
        tmdb_id,
        ts: added,
    }))
}

fn write_vod_info_record_to_wal_file(
    writer: &mut BufWriter<&File>,
    provider_id: u32,
    record: &InputVodInfoRecord,
) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&record.tmdb_id.to_le_bytes())?;
    writer.write_all(&record.ts.to_le_bytes())?;
    Ok(())
}

fn should_update_vod_info(pli: &mut PlaylistItem, processed_provider_ids: &HashMap<u32, u64>) -> (bool, u32, u64) {
    should_update_info(pli, processed_provider_ids, crate::model::XC_TAG_VOD_INFO_ADDED)
}

pub async fn playlist_resolve_vod(client: Arc<reqwest::Client>, cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>, fpl: &mut FetchedPlaylist<'_>) {
    let (resolve_movies, resolve_delay) = get_resolve_vod_options(target, fpl);
    if !resolve_movies { return; }

    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a wal file and write it once we collected everything.
    let Some((wal_content_file, wal_record_file, wal_content_path, wal_record_path)) = create_resolve_info_wal_files(cfg, fpl.input, XtreamCluster::Video)
    else { return; };

    let mut processed_info_ids = read_processed_vod_info_ids(cfg, errors, fpl).await;
    let mut content_writer = file_writer(&wal_content_file);
    let mut record_writer = file_writer(&wal_record_file);
    let mut content_updated = false;

    // TODO merge both filters to one
    let vod_info_count = fpl.playlistgroups.iter()
        .flat_map(|plg| &plg.channels)
        .filter(|&pli| pli.header.xtream_cluster == XtreamCluster::Video).count();

    let vod_info_iter = fpl.playlistgroups.iter_mut()
        .flat_map(|plg| plg.channels.iter_mut())
        .filter(|pli| pli.header.xtream_cluster == XtreamCluster::Video);

    info!("Found {vod_info_count} vod info to resolve");
    let start_time = Instant::now();
    let mut processed_vod_info_count = 0;
    let mut last_processed_vod_info_count = 0;

    for pli in  vod_info_iter {
        let (should_update, _provider_id, _ts) = should_update_vod_info(pli, &processed_info_ids);
        if should_update {
            if let Some(content) = playlist_resolve_download_playlist_item(Arc::clone(&client), pli, fpl.input, errors, resolve_delay, XtreamCluster::Video).await {
                if let Some((provider_id, info_record)) = extract_info_record_from_vod_info(&content) {
                    let ts = info_record.ts;
                    handle_error_and_return!(write_info_content_to_wal_file(&mut content_writer, provider_id, &content),
                        |err| errors.push(notify_err!(format!("Failed to resolve vod, could not write to content wal file {err}"))));
                    processed_info_ids.insert(provider_id, ts);
                    handle_error_and_return!(write_vod_info_record_to_wal_file(&mut record_writer, provider_id, &info_record),
                        |err| errors.push(notify_err!(format!("Failed to resolve vod wal, could not write to record wal file {err}"))));
                    content_updated = true;
                }
            }
        }
        if log_enabled!(Level::Info) {
            processed_vod_info_count += 1;
            let elapsed = start_time.elapsed().as_secs();
            if elapsed > 0 && ((processed_vod_info_count - last_processed_vod_info_count) > 50) && (elapsed % 30 == 0) {
                info!("resolved {processed_vod_info_count}/{vod_info_count} vod info");
                last_processed_vod_info_count = processed_vod_info_count;
            }
        }
    }
    if last_processed_vod_info_count != processed_vod_info_count {
        info!("resolved {processed_vod_info_count}/{vod_info_count} vod info");
    }
    if content_updated {
        handle_error!(content_writer.flush(),
            |err| errors.push(notify_err!(format!("Failed to resolve vod, could not write to wal file {err}"))));
        handle_error!(record_writer.flush(),
            |err| errors.push(notify_err!(format!("Failed to resolve vod tmdb, could not write to wal file {err}"))));
        drop(content_writer);
        drop(record_writer);
        drop(wal_content_file);
        drop(wal_record_file);
        handle_error!(xtream_update_input_info_file(cfg, fpl.input, &wal_content_path, XtreamCluster::Video).await,
            |err| errors.push(err));
        handle_error!(xtream_update_input_vod_record_from_wal_file(cfg, fpl.input, &wal_record_path).await,
            |err| errors.push(err));
    }
}
