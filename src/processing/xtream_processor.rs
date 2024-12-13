use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput, ConfigTarget, InputType, TargetType};
use crate::model::playlist::{FetchedPlaylist, PlaylistEntry, PlaylistItem, PlaylistItemType, UUIDType, XtreamCluster};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_info_file_paths, xtream_update_input_vod_info_file, xtream_update_input_vod_tmdb_file};
use crate::repository::IndexedDocumentQuery;
use crate::utils::download;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Write};
use std::rc::Rc;

const TAG_VOD_INFO_INFO: &str = "info";
const TAG_VOD_INFO_MOVIE_DATA: &str = "movie_data";
const TAG_VOD_INFO_TMDB_ID: &str = "tmdb_id";
const TAG_VOD_INFO_STREAM_ID: &str = "stream_id";

pub async fn playlist_resolve_series(target: &ConfigTarget, errors: &mut Vec<M3uFilterError>,
                                     pipe: &ProcessingPipe,
                                     provider_fpl: &mut FetchedPlaylist<'_>,
                                     processed_fpl: &mut FetchedPlaylist<'_>) {
    let (resolve_series, resolve_series_delay) =
        if let Some(options) = &target.options {
            (options.xtream_resolve_series && provider_fpl.input.input_type == InputType::Xtream && target.has_output(&TargetType::M3u),
             options.xtream_resolve_series_delay)
        } else {
            (false, 0)
        };
    if resolve_series {
        // collect all series in the processed lists
        let to_process_uuids: HashSet<Rc<UUIDType>> = processed_fpl.playlistgroups.iter()
            .filter(|plg| plg.xtream_cluster == XtreamCluster::Series)
            .flat_map(|plg| &plg.channels)
            .filter(|pli| pli.header.borrow().item_type == PlaylistItemType::SeriesInfo)
            .map(|pli| Rc::clone(&pli.header.borrow().uuid)).collect();
        let mut series_playlist = download::get_xtream_playlist_series(provider_fpl, to_process_uuids, errors, resolve_series_delay).await;
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
}

async fn playlist_resolve_vod_process_playlist_item(pli: &PlaylistItem, input: &ConfigInput, errors: &mut Vec<M3uFilterError>, resolve_delay: u16) -> Option<String> {
    let mut result = None;
    let provider_id = pli.get_provider_id().unwrap_or(0);
    if let Some(info_url) = download::get_xtream_player_api_info_url(input, XtreamCluster::Video, provider_id) {
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

fn write_vod_info_content_to_temp_file(writer: &mut BufWriter<&File>, provider_id: u32, content: &str) -> std::io::Result<()> {
    let length = u32::try_from(content.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
    if length > 0 {
        writer.write_all(&provider_id.to_le_bytes())?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(())
}

fn write_vod_info_tmdb_to_temp_file(writer: &mut BufWriter<&File>, provider_id: u32, tmdb_id: u32) -> std::io::Result<()> {
    writer.write_all(&provider_id.to_le_bytes())?;
    writer.write_all(&tmdb_id.to_le_bytes())?;
    Ok(())
}

fn get_resolve_video_options(target: &ConfigTarget, fpl: &FetchedPlaylist) -> (bool, u16) {
    let (resolve_movies, resolve_delay) =
        target.options.as_ref().map_or((false, 0), |opt| (opt.xtream_resolve_video && fpl.input.input_type == InputType::Xtream, opt.xtream_resolve_video_delay));
    (resolve_movies, resolve_delay)
}

fn get_u32_from_serde_value(value: &Value) -> Option<u32> {
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

fn extract_provider_id_and_tmdb_id_from_vod_info(content: &str) -> Option<(u32, u32)> {
    if let Ok(mut doc) = serde_json::from_str::<Map<String, Value>>(content) {
        if let Some(Value::Object(movie_data)) = doc.get_mut(TAG_VOD_INFO_MOVIE_DATA) {
            if let Some(stream_id_value) = movie_data.get(TAG_VOD_INFO_STREAM_ID) {
                if let Some(stream_id) = get_u32_from_serde_value(stream_id_value) {
                    if let Some(Value::Object(info)) = doc.get_mut(TAG_VOD_INFO_INFO) {
                        if let Some(tmdb_id_value) = info.get(TAG_VOD_INFO_TMDB_ID) {
                            if let Some(tmdb_id) = get_u32_from_serde_value(tmdb_id_value) {
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

fn create_resolve_vod_info_temp_files(errors: &mut Vec<M3uFilterError>) -> Option<(File, File)> {
    let temp_file_info = match tempfile::tempfile() {
        Ok(value) => value,
        Err(err) => {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Cant resolve vod, could not create temporary file {err}")));
            return None;
        }
    };
    let temp_file_tmdb = match tempfile::tempfile() {
        Ok(value) => value,
        Err(err) => {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Cant resolve vod tmdb, could not create temporary file {err}")));
            return None;
        }
    };
    Some((temp_file_info, temp_file_tmdb))
}

pub async fn playlist_resolve_vod(cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) {
    let (resolve_movies, resolve_delay) = get_resolve_video_options(target, fpl);
    if !resolve_movies { return; }

    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a temp file and write it once we collected everything.
    let Some((temp_file_info, temp_file_tmdb)) = create_resolve_vod_info_temp_files(errors) else { return };

    let mut processed_vod_ids = read_processed_vod_info_ids(cfg, errors, fpl).await;
    let mut info_writer = BufWriter::new(&temp_file_info);
    let mut tmdb_writer = BufWriter::new(&temp_file_tmdb);
    for pli in fpl.playlistgroups.iter().flat_map(|plg| &plg.channels) {
        let a = pli.header.borrow_mut().get_provider_id().as_ref().map_or(false, |pid| processed_vod_ids.contains(pid));
        if !a {
            if let Some(content) = playlist_resolve_vod_process_playlist_item(pli, fpl.input, errors, resolve_delay).await {
                if let Some((provider_id, tmdb_id)) = extract_provider_id_and_tmdb_id_from_vod_info(&content) {
                    if let Err(err) = write_vod_info_content_to_temp_file(&mut info_writer, provider_id, &content) {
                        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to temporary file {err}")));
                        return;
                    }
                    processed_vod_ids.insert(provider_id);
                    if tmdb_id > 0 {
                        if let Err(err) = write_vod_info_tmdb_to_temp_file(&mut tmdb_writer, provider_id, tmdb_id) {
                            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to temporary file {err}")));
                            return;
                        }
                    }
                    // TODO create tmdb_id index for kodi export
                }
            }
        }
    }
    if let Err(err) = info_writer.flush() {
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod, could not write to temporary file {err}")));
    }
    if let Err(err) = tmdb_writer.flush() {
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve vod tmdb, could not write to temporary file {err}")));
    }

    if let Err(err) = xtream_update_input_vod_info_file(cfg, fpl.input, &temp_file_info).await {
        errors.push(err);
    }
    if let Err(err) = xtream_update_input_vod_tmdb_file(cfg, fpl.input, &temp_file_tmdb).await {
        errors.push(err);
    }

}

async fn read_processed_vod_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) -> HashSet<u32> {
    let mut processed_vod_ids = HashSet::new();
    {
        match get_input_storage_path(fpl.input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, XtreamCluster::Video)) {
            Ok(Some((file_path, idx_path))) => {
                match cfg.file_locks.read_lock(&file_path).await {
                    Ok(file_lock) => {
                        if let Ok(mut info_id_mapping) = IndexedDocumentQuery::<u32, String>::try_new(&idx_path) {
                            info_id_mapping.traverse(|keys, _| {
                                for doc_id in keys { processed_vod_ids.insert(*doc_id); }
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
    processed_vod_ids
}