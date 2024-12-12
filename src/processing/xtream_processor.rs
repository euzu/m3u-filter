use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput, ConfigTarget, InputType, TargetType};
use crate::model::playlist::{FetchedPlaylist, PlaylistEntry, PlaylistItem, PlaylistItemType, UUIDType, XtreamCluster};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_info_file_paths, xtream_update_input_vod_info_file};
use crate::repository::IndexedDocumentQuery;
use crate::utils::download;
use crate::utils::download::get_xtream_stream_info_content;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Write};
use std::rc::Rc;

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

async fn playlist_resolve_movies_process_playlist_item(pli: &PlaylistItem, input: &ConfigInput, errors: &mut Vec<M3uFilterError>, resolve_delay: u16) -> Option<String> {
    let mut result = None;
    let provider_id = pli.get_provider_id().unwrap_or(0);
    if let Some(info_url) = download::get_xtream_player_api_info_url(input, XtreamCluster::Video, provider_id) {
        result = match get_xtream_stream_info_content(&info_url, input).await {
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

fn write_content_to_file(writer: &mut BufWriter<&File>, uuid: &[u8;32], content: &str) -> std::io::Result<()> {
    let length = u32::try_from(content.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
    if length > 0 {
        writer.write_all(uuid)?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(())
}


fn get_resolve_movies_options(target: &ConfigTarget, fpl: &FetchedPlaylist) -> (bool, u16) {
    let (resolve_movies, resolve_delay) =
        if let Some(options) = &target.options {
            (options.xtream_resolve_movies && fpl.input.input_type == InputType::Xtream, options.xtream_resolve_movies_delay)
        } else {
            (false, 0)
        };
    (resolve_movies, resolve_delay)
}

fn create_temp_file() -> Result<File, M3uFilterError> {
    match tempfile::tempfile() {
        Ok(temp_file) => Ok(temp_file),
        Err(err) => Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Cant resolve movies, could not create temporary file {err}")))
    }
}

pub async fn playlist_resolve_movies(cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) {
    let (resolve_movies, resolve_delay) = get_resolve_movies_options(target, fpl);
    if !resolve_movies {
        return;
    }

    // we cant write to the indexed-document directly because of the write lock and time-consuming operation.
    // All readers would be waiting for the lock and the app would be unresponsive.
    // We collect the content into a temp file and write it once we collected everything.
    let temp_file = match create_temp_file() {
        Ok(value) => value,
        Err(err) => {
            errors.push(err);
            return;
        }
    };

    let mut processed_vod_ids = read_processed_vod_info_ids(cfg, errors, fpl).await;
    let mut writer = BufWriter::new(&temp_file);
    for pli in fpl.playlistgroups.iter().flat_map(|plg| &plg.channels) {
        if !processed_vod_ids.contains(pli.header.borrow().get_uuid().as_ref()) {
            if let Some(content) = playlist_resolve_movies_process_playlist_item(pli, fpl.input, errors, resolve_delay).await {
                processed_vod_ids.insert(*pli.header.borrow().uuid);
                if let Err(err) = write_content_to_file(&mut writer, &pli.header.borrow().uuid, &content) {
                    errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve movies, could not write to temporary file {err}")));
                    return;
                }
            }
        }
    }
    if let Err(err) = writer.flush() {
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to resolve movies, could not write to temporary file {err}")));
    }

    if let Err(err) = xtream_update_input_vod_info_file(cfg, fpl.input, &temp_file).await {
        errors.push(err);
    }
}

async fn read_processed_vod_info_ids(cfg: &Config, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) -> HashSet<UUIDType> {
    let mut processed_vod_ids = HashSet::new();
    {
        match get_input_storage_path(fpl.input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, XtreamCluster::Video)) {
            Ok(Some((file_path, idx_path))) => {
                match cfg.file_locks.read_lock(&file_path).await {
                    Ok(file_lock) => {
                        if let Ok(mut info_id_mapping) = IndexedDocumentQuery::<UUIDType, String>::try_new(&idx_path) {
                            info_id_mapping.traverse(|keys, _| {
                                for uuid in keys { processed_vod_ids.insert(*uuid); }
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
