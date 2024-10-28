use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Error, ErrorKind};
use std::path::{Path, PathBuf};

use log::error;
use serde_json::{json, Value};

use crate::{create_m3u_filter_error, create_m3u_filter_error_result};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::bplustree::{BPlusTreeQuery};
use crate::repository::id_mapping::IdMapping;
use crate::repository::indexed_document_reader::{IndexedDocumentReader};
use crate::repository::indexed_document_writer::IndexedDocumentWriter;
use crate::repository::storage::{get_target_id_mapping_file, get_target_storage_path, hash_string};
use crate::repository::target_id_mapping::{TargetIdMapping, VirtualIdRecord};
use crate::utils::json_utils::{json_iter_array, json_write_documents_to_file};

pub(crate) static COL_CAT_LIVE: &str = "cat_live";
pub(crate) static COL_CAT_SERIES: &str = "cat_series";
pub(crate) static COL_CAT_VOD: &str = "cat_vod";

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error!(M3uFilterErrorKind::Notify, "failed to write xtream playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

fn get_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{collection}.json"))
}

fn ensure_xtream_storage_path(cfg: &Config, target_name: &str) -> Result<PathBuf, M3uFilterError> {
    if let Some(path) = xtream_get_storage_path(cfg, target_name) {
        if std::fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save xtream data, can't create directory {}", &path.to_str().unwrap());
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg));
        }
        Ok(path)
    } else {
        let msg = format!("Failed to save xtream data, can't create directory for target {target_name}");
        Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg))
    }
}

fn xtream_get_info_file_paths(storage_path: &Path, cluster: XtreamCluster) -> Option<(PathBuf, PathBuf)> {
    if cluster == XtreamCluster::Series {
        let xtream_path = storage_path.join("series_episodes.db");
        let extension = xtream_path.extension().map(|ext| format!("{}_", ext.to_str().unwrap_or("")));
        let index_path = xtream_path.with_extension(format!("{}idx", &extension.unwrap_or_default()));
        return Some((xtream_path, index_path));
    }
    None
}

fn xtream_get_catchup_id_mapping_file_path(storage_path: &Path) -> PathBuf {
    storage_path.join("catchup_mapping.db")
}

fn write_playlists_to_file(storage_path: &Path, collections: Vec<(XtreamCluster, &mut [PlaylistItem])>) -> Result<(), M3uFilterError> {
    for (cluster, playlist) in collections {
        let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, &cluster);
        match IndexedDocumentWriter::new(xtream_path.clone(), idx_path) {
            Ok(mut writer) => {
                for item in playlist {
                    match item.to_xtream() {
                        Ok(xtream) => {
                            match writer.write_doc(item.header.borrow().virtual_id, &xtream) {
                                Ok(_) => {}
                                Err(err) => return Err(cant_write_result!(&xtream_path, err))
                            }
                        },
                        Err(err) => return Err(cant_write_result!(&xtream_path, err))
                    }
                }
                writer.flush().map_err(|err| cant_write_result!(&xtream_path, err))?;
            }
            Err(err) => return Err(cant_write_result!(&xtream_path, err))
        }
    }
    Ok(())
}

fn get_map_item_as_str(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    if let Some(value) = map.get(key) {
        if let Some(result) = value.as_str() {
            return Some(result.to_string());
        }
    }
    None
}

fn load_old_category_ids(path: &Path) -> (u32, HashMap<String, u32>) {
    let mut result: HashMap<String, u32> = HashMap::new();
    let mut max_id: u32 = 0;
    for col_path in [
        get_collection_path(path, COL_CAT_LIVE),
        get_collection_path(path, COL_CAT_VOD),
        get_collection_path(path, COL_CAT_SERIES)] {
        if col_path.exists() {
            if let Ok(file) = File::open(col_path) {
                let reader = BufReader::new(file);
                for entry in json_iter_array::<Value, BufReader<File>>(reader).flatten() {
                    if let Some(item) = entry.as_object() {
                        if let Some(category_id) = get_map_item_as_str(item, "category_id") {
                            if let Some(category_name) = get_map_item_as_str(item, "category_name") {
                                if let Ok(cat_id) = category_id.parse::<u32>() {
                                    result.insert(category_name, cat_id);
                                    max_id = max_id.max(cat_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (max_id, result)
}

pub(crate) fn xtream_get_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    match get_target_storage_path(cfg, target_name) {
        Some(target_path) => Some(target_path.join(std::path::PathBuf::from("xtream"))),
        None => None,
    }
}

pub(crate) fn xtream_get_epg_file_path(path: &Path) -> PathBuf {
    path.join("epg.xml")
}

fn xtream_get_file_paths_for_name(storage_path: &Path, name: &str) -> (PathBuf, PathBuf) {
    let xtream_path = storage_path.join(format!("{}.db", name));
    let extension = xtream_path.extension().map(|ext| format!("{}_", ext.to_str().unwrap_or("")));
    let index_path = xtream_path.with_extension(format!("{}idx", &extension.unwrap_or_default()));
    (xtream_path, index_path)
}

pub(crate) fn xtream_get_file_paths(storage_path: &Path, cluster: &XtreamCluster) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, &cluster.as_str().to_lowercase())
}

pub(crate) fn xtream_get_file_paths_for_series(storage_path: &Path) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, "series")
}

pub(crate) fn xtream_write_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut [PlaylistGroup]) -> Result<(), M3uFilterError> {
    match ensure_xtream_storage_path(cfg, target.name.as_str()) {
        Ok(path) => {
            let mut cat_live_col = vec![];
            let mut cat_series_col = vec![];
            let mut cat_vod_col = vec![];
            let mut live_col = vec![];
            let mut series_col = vec![];
            let mut vod_col = vec![];
            let mut errors = Vec::new();

            // preserve category_ids
            let (max_cat_id, existing_cat_ids) = load_old_category_ids(&path);
            let mut cat_id_counter = max_cat_id;
            for plg in playlist.iter_mut() {
                if !&plg.channels.is_empty() {
                    let cat_id = existing_cat_ids.get(plg.title.as_ref()).unwrap_or_else(|| {
                        cat_id_counter += 1;
                        &cat_id_counter
                    });
                    plg.id = *cat_id;

                    match &plg.xtream_cluster {
                        XtreamCluster::Live => &mut cat_live_col,
                        XtreamCluster::Series => &mut cat_series_col,
                        XtreamCluster::Video => &mut cat_vod_col,
                    }.push(json!({
                      "category_id": format!("{}", &cat_id),
                      "category_name": plg.title.clone(),
                      "parent_id": 0
                    }));

                    for pli in plg.channels.drain(..) {
                        let mut header = pli.header.borrow_mut();
                        // we skip resolved series, because this is only necessary when writing m3u files
                        let col = if header.item_type == PlaylistItemType::Series {
                            None
                        } else {
                            match header.get_provider_id() {
                                Some(_) => {
                                    header.category_id = *cat_id;
                                    Some(match header.xtream_cluster {
                                        XtreamCluster::Live => &mut live_col,
                                        XtreamCluster::Series => &mut series_col,
                                        XtreamCluster::Video => &mut vod_col,
                                    })
                                }
                                None => {
                                    let title = header.title.as_str();
                                    errors.push(format!("Channel does not have an id: {title}"));
                                    None
                                }
                            }
                        };
                        drop(header);
                        if let Some(pl) = col {
                            pl.push(pli);
                        }
                    }
                }
            }

            for (col_path, data) in [
                (get_collection_path(&path, COL_CAT_LIVE), &cat_live_col),
                (get_collection_path(&path, COL_CAT_VOD), &cat_vod_col),
                (get_collection_path(&path, COL_CAT_SERIES), &cat_series_col)] {
                match json_write_documents_to_file(&col_path, data) {
                    Ok(()) => {}
                    Err(err) => {
                        errors.push(format!("Persisting collection failed: {}: {}", &col_path.to_str().unwrap(), err));
                    }
                }
            }

            match write_playlists_to_file(&path, vec![
                (XtreamCluster::Live, &mut live_col),
                (XtreamCluster::Video, &mut vod_col),
                (XtreamCluster::Series, &mut series_col)]) {
                Ok(()) => {}
                Err(err) => {
                    errors.push(format!("Persisting collection failed:{err}"));
                }
            }

            if !errors.is_empty() {
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", errors.join("\n"));
            }
        }
        Err(err) => return Err(err)
    }
    Ok(())
}

pub(crate) fn xtream_get_collection_path(cfg: &Config, target_name: &str, collection_name: &str) -> Result<(Option<PathBuf>, Option<String>), Error> {
    if let Some(path) = xtream_get_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok((Some(col_path), None));
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Cant find collection: {target_name}/{collection_name}")))
}

fn xtream_read_item_for_stream_id(stream_id: u32, storage_path: &Path, cluster: &XtreamCluster) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, cluster);
    return IndexedDocumentReader::<XtreamPlaylistItem>::read_indexed_item(&xtream_path, &idx_path, stream_id);
}

fn xtream_read_series_item_for_stream_id(stream_id: u32, storage_path: &Path) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths_for_series(storage_path);
    return IndexedDocumentReader::<XtreamPlaylistItem>::read_indexed_item(&xtream_path, &idx_path, stream_id);
}

pub(crate) fn xtream_get_item_for_stream_id(virtual_id: u32, config: &Config, target: &ConfigTarget, xtream_cluster: Option<XtreamCluster>) -> Result<XtreamPlaylistItem, Error> {
    if let Some(target_path) = get_target_storage_path(config, target.name.as_str()) {
        if let Some(storage_path) = xtream_get_storage_path(config, target.name.as_str()) {
            match BPlusTreeQuery::<u32, VirtualIdRecord>::try_new(&get_target_id_mapping_file(&target_path)) {
                Ok(mut target_id_mapping) => {
                    return match target_id_mapping.query(&virtual_id) {
                        Some(mapping) => {
                            if mapping.item_type == PlaylistItemType::SeriesInfo {
                                xtream_read_series_item_for_stream_id(virtual_id, &storage_path)
                            } else if mapping.item_type == PlaylistItemType::Series && mapping.parent_virtual_id > 0 {
                                // we load the original series item
                                match xtream_read_series_item_for_stream_id(mapping.parent_virtual_id, &storage_path) {
                                    Ok(mut item) => {
                                        // we need to replace the provider id with the episode provider id
                                        item.provider_id = mapping.provider_id;
                                        Ok(item)
                                    },
                                    Err(err) => Err(err)
                                }
                            } else {
                                let cluster = match xtream_cluster {
                                    Some(c) => Some(c),
                                    None => match XtreamCluster::try_from(mapping.item_type) {
                                        Ok(item_type) => Some(item_type),
                                        Err(_) => None
                                    }
                                };
                                match cluster {
                                    Some(xc) => xtream_read_item_for_stream_id(virtual_id, &storage_path, &xc),
                                    None => Err(Error::new(ErrorKind::Other, format!("Could not determine cluster for xtream item with stream-id {virtual_id}")))
                                }
                            }
                        },
                        None => Err(Error::new(ErrorKind::Other, format!("Could not find mappping for target {} and id {}", target.name, virtual_id))),
                    };
                },
                Err(err) => return Err(Error::new(ErrorKind::Other, format!("Could not load id mappping for target {} err:{}", target.name, err.to_string())))
            };
        } else {
            return Err(Error::new(ErrorKind::Other, format!("Could not find path for target {} xtream output", &target.name)));
        }
    } else {
        return Err(Error::new(ErrorKind::Other, format!("Could not find path for target {}", &target.name)));
    }
}

pub(crate) fn xtream_load_rewrite_playlist(cluster: &XtreamCluster, config: &Config, target: &ConfigTarget, category_id: u32) -> Result<String, Error> {
    if let Some(storage_path) = xtream_get_storage_path(config, target.name.as_str()) {
        let (xtream_path, _) = xtream_get_file_paths(&storage_path, cluster);
        match IndexedDocumentReader::<XtreamPlaylistItem>::new(&xtream_path) {
            Ok(mut reader) => {
                let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
                let result: Vec<Value> = reader.by_ref().filter(|pli| category_id == 0 || pli.category_id == category_id)
                    .map(|pli| pli.to_doc(&options)).collect();
                if reader.by_ref().has_error() {
                    error!("Could not deserialize item {}", &xtream_path.to_str().unwrap());
                } else {
                    return Ok(serde_json::to_string(&result).unwrap());
                }
            }
            Err(err) => {
                error!("Could not deserialize file {} - {}", &xtream_path.to_str().unwrap(), err);
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to find xtream storage for target {}", &target.name)))
}

pub(crate) fn xtream_write_series_info(config: &Config, target_name: &str,
                                       series_id: u32,
                                       content: &str) -> Result<(), Error> {
    if let Some(storage_path) = xtream_get_storage_path(config, target_name) {
        if let Some((info_path, idx_path)) = xtream_get_info_file_paths(&storage_path, XtreamCluster::Series) {
            return match IndexedDocumentWriter::new_append(info_path.clone(), idx_path) {
                Ok(mut writer) => {
                    match writer.write_doc(series_id, content) {
                        Ok(_) => {},
                        Err(_) => return Err(Error::new(ErrorKind::Other, format!("failed to write xtream series info for target {target_name}")))
                    }
                    return Ok(writer.flush()?);
                }
                Err(err) => Err(err)
            };
        }
    }
    Ok(())
}

// Reads the series info entry if exists, otherwise error
pub(crate) fn xtream_load_series_info(config: &Config, target_name: &str, series_id: u32) -> Result<String, Error> {
    if let Some(storage_path) = xtream_get_storage_path(config, target_name) {
        if let Some((info_path, idx_path)) = xtream_get_info_file_paths(&storage_path, XtreamCluster::Series) {
            if info_path.exists() && idx_path.exists() {
                return IndexedDocumentReader::<String>::read_indexed_item(&info_path, &idx_path, series_id);
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to read series info for id {series_id} for {target_name}")))
}

pub(crate) fn xtream_load_catchup_id_mapping(config: &Config, target_name: &str) -> Result<IdMapping<u32>, Error> {
    if let Some(storage_path) = xtream_get_storage_path(config, target_name) {
        let catchup_file = xtream_get_catchup_id_mapping_file_path(&storage_path);
        return Ok(IdMapping::<u32>::new(&catchup_file));
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to load catchup id mapping {target_name}")))
}

pub(crate) fn write_and_get_xtream_series_info(config: &Config, target: &ConfigTarget, pli_series_info: &XtreamPlaylistItem, content: &str) -> Result<String, Error> {
    if let Ok(mut doc) = serde_json::from_str::<Value>(content) {
        if let Some(target_path) = get_target_storage_path(config, target.name.as_str()) {
            let mut target_id_mapping = TargetIdMapping::new(&get_target_id_mapping_file(&target_path));
            if let Some(episodes) = doc.get_mut("episodes").and_then(|e| e.as_object_mut()) {
                let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
                for episode_list in episodes.values_mut() {
                    if let Some(entries) = episode_list.as_array_mut() {
                        for episode in entries.iter_mut().filter_map(|e| e.as_object_mut()) {
                            if let Some(episode_id) = episode.get("id").and_then(|id| id.as_str()) {
                                if let Ok(provider_id) = episode_id.parse::<u32>() {
                                    let uuid = hash_string(&format!("{}/{}", pli_series_info.url, provider_id));
                                    let virtual_id = target_id_mapping.insert_entry(provider_id, uuid, &PlaylistItemType::Series, pli_series_info.virtual_id);
                                    episode.insert("id".to_string(), Value::String(virtual_id.to_string()));
                                }
                            }

                            if options.skip_series_direct_source {
                                episode.insert("direct_source".to_string(), Value::String(String::new()));
                            }
                        }
                    }
                }

                drop(target_id_mapping);
                if let Ok(result) = serde_json::to_string(&doc) {
                    let _ = xtream_write_series_info(config, target.name.as_str(), pli_series_info.virtual_id, &result);
                    return Ok(result);
                }
            }


            // if let Some(episodes) = doc.get_mut("episodes") {
            //     if let Some(episodes_map) = episodes.as_object_mut() {
            //         let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
            //         for (_season, episode_list) in episodes_map {
            //             // Iterate over items in the episode
            //             if let Some(entries) = episode_list.as_array_mut() {
            //                 for entry in entries {
            //                     if let Some(episode) = entry.as_object_mut() {
            //                         if let Some(episode_id) = episode.get("id") {
            //                             if let Ok(provider_id) = episode_id.as_str().unwrap().parse::<u32>() {
            //                                 let uuid = hash_string(format!("{}/{}", pli_series_info.url, provider_id).as_str());
            //                                 let virtual_id = target_id_mapping.insert_entry(provider_id, uuid, &PlaylistItemType::Series, pli_series_info.virtual_id);
            //                                 episode.insert("id".to_string(), Value::String(virtual_id.to_string()));
            //                             }
            //                         }
            //                         if options.skip_series_direct_source {
            //                             episode.insert("direct_source".to_string(), Value::String(String::new()));
            //                         }
            //                     }
            //                 }
            //             }
            //         }
            //     }
            //     drop(target_id_mapping);
            //     if let Ok(result) = serde_json::to_string(&doc) {
            //         let _ = xtream_repository::xtream_write_series_info(config, target.name.as_str(), pli_series_info.virtual_id, &result);
            //         return Ok(result);
            //     }
            // }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to get series info for id {}", pli_series_info.virtual_id)))
}