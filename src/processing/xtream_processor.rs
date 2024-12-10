use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput, ConfigTarget, InputType, TargetType};
use crate::model::playlist::{FetchedPlaylist, PlaylistEntry, PlaylistItem, PlaylistItemType, UUIDType, XtreamCluster};
use crate::processing::playlist_processor::ProcessingPipe;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::xtream_get_info_file_paths;
use crate::repository::IndexedDocumentQuery;
use crate::utils::download;
use crate::utils::download::get_xtream_stream_info_content;
use std::collections::HashSet;
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

pub async fn playlist_resolve_movies(cfg: &Config, target: &ConfigTarget, errors: &mut Vec<M3uFilterError>, fpl: &FetchedPlaylist<'_>) {
    let (resolve_movies, resolve_delay) =
        if let Some(options) = &target.options {
            (options.xtream_resolve_movies && fpl.input.input_type == InputType::Xtream, options.xtream_resolve_movies_delay)
        } else {
            (false, 0)
        };
    if !resolve_movies {
        return;
    }
    match get_input_storage_path(fpl.input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, XtreamCluster::Video)) {
        Ok(Some((file_path, idx_path))) => {
            match cfg.file_locks.write_lock(&file_path).await {
                Ok(_file_lock) => {
                    match IndexedDocumentWriter::<UUIDType, String>::try_new(&idx_path) {
                        Ok(mut info_id_mapping) => {
                            for pli in fpl.playlistgroups.iter().flat_map(|plg| &plg.channels) {
                                if info_id_mapping.query(pli.header.borrow().get_uuid()).is_none() {
                                    playlist_resolve_movies_process_playlist_item(pli, fpl.input, errors, resolve_delay).await;
                                }
                            }
                        }
                        Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Could not load id mapping for input {err}"))),
                    }
                }
                Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, format!("{err}"))),
            }
        }
        Ok(None) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Could not create storage path for input {}", &fpl.input.name.as_ref().map_or("?", |v| v)))),
        Err(err) => errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Could not create storage path for input {err}"))),
    }
}