use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, InputType};
use crate::model::playlist::{FetchedPlaylist, XtreamCluster};
use crate::processing::xtream_processor::{create_resolve_info_wal_files, playlist_resolve_process_playlist_item, read_processed_info_ids, write_info_content_to_temp_file};
use crate::repository::xtream_repository::{xtream_update_input_info_file, xtream_update_input_vod_tmdb_file};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use crate::create_resolve_options_function_for_xtream_input;

const TAG_VOD_INFO_INFO: &str = "info";
const TAG_VOD_INFO_MOVIE_DATA: &str = "movie_data";
const TAG_VOD_INFO_TMDB_ID: &str = "tmdb_id";
const TAG_VOD_INFO_STREAM_ID: &str = "stream_id";

create_resolve_options_function_for_xtream_input!(video);

fn write_vod_info_tmdb_to_temp_file(writer: &mut BufWriter<&File>, provider_id: u32, tmdb_id: u32) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&tmdb_id.to_le_bytes())?;
    Ok(())
}

fn extract_provider_id_and_tmdb_id_from_vod_info(content: &str) -> Option<(u32, u32)> {
    if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content) {
        if let Some(Value::Object(movie_data)) = doc.get_mut(TAG_VOD_INFO_MOVIE_DATA) {
            if let Some(stream_id_value) = movie_data.get(TAG_VOD_INFO_STREAM_ID) {
                if let Some(stream_id) = crate::processing::xtream_processor::get_u32_from_serde_value(stream_id_value) {
                    if let Some(Value::Object(info)) = doc.get_mut(TAG_VOD_INFO_INFO) {
                        if let Some(tmdb_id_value) = info.get(TAG_VOD_INFO_TMDB_ID) {
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

pub async fn playlist_resolve_vod(cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) {
    let (resolve_movies, resolve_delay) = get_resolve_video_options(target, fpl);
    if !resolve_movies { return; }

    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a temp file and write it once we collected everything.
    let Some((mut wal_file_config, mut wal_file_tmdb)) = create_resolve_info_wal_files(cfg, fpl.input, XtreamCluster::Video) else { return };

    let mut processed_vod_ids = read_processed_info_ids(cfg, errors, fpl, XtreamCluster::Video).await;
    let mut content_writer = BufWriter::new(&wal_file_config);
    let mut tmdb_writer = BufWriter::new(&wal_file_tmdb);
    let mut content_updated = false;
    let mut tmdb_updated = false;
    for pli in fpl.playlistgroups.iter().flat_map(|plg| &plg.channels).filter(|chan| chan.header.borrow().xtream_cluster == XtreamCluster::Video) {
        let processed_entry = pli.header.borrow_mut().get_provider_id().as_ref().map_or(false, |pid| processed_vod_ids.contains(pid));
        if !processed_entry {
            if let Some(content) = playlist_resolve_process_playlist_item(pli, fpl.input, errors, resolve_delay, XtreamCluster::Video).await {
                if let Some((provider_id, tmdb_id)) = extract_provider_id_and_tmdb_id_from_vod_info(&content) {
                    if let Err(err) = write_info_content_to_temp_file(&mut content_writer, provider_id, &content) {
                        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to temporary file {err}")));
                        return;
                    }
                    content_updated = true;
                    processed_vod_ids.insert(provider_id);
                    if tmdb_id > 0 {
                        if let Err(err) = write_vod_info_tmdb_to_temp_file(&mut tmdb_writer, provider_id, tmdb_id) {
                            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to temporary file {err}")));
                            return;
                        }
                        tmdb_updated = true;
                    }
                }
            }
        }
    }
    if content_updated {
        if let Err(err) = content_writer.flush() {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to wal file {err}")));
        }
        drop(content_writer);
        if let Err(err) = xtream_update_input_info_file(cfg, fpl.input, &mut wal_file_config, XtreamCluster::Video).await {
            errors.push(err);
        }
    }
    if tmdb_updated {
        if let Err(err) = tmdb_writer.flush() {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to wal file {err}")));
        }
        drop(tmdb_writer);
        if let Err(err) = xtream_update_input_vod_tmdb_file(cfg, fpl.input, &mut wal_file_tmdb).await {
            errors.push(err);
        }
    }
}
