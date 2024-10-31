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
use crate::repository::bplustree::{BPlusTreeQuery, BPlusTreeUpdate};
use crate::repository::indexed_document::{IndexedDocumentGarbageCollector, IndexedDocumentReader, IndexedDocumentWriter};
use crate::repository::storage::{get_target_id_mapping_file, get_target_storage_path, hash_string, FILE_SUFFIX_DB, FILE_SUFFIX_INDEX};
use crate::repository::target_id_mapping::{TargetIdMapping, VirtualIdRecord};
use crate::utils::json_utils::{json_iter_array, json_write_documents_to_file};

pub(crate) static COL_CAT_LIVE: &str = "cat_live";
pub(crate) static COL_CAT_SERIES: &str = "cat_series";
pub(crate) static COL_CAT_VOD: &str = "cat_vod";
const FILE_SERIES_EPISODES: &str = "series_episodes";
const FILE_SERIES: &str = "series";
const FILE_EPG: &str = "epg.xml";
const PATH_XTREAM: &str = "xtream";
const TAG_CATEGORY_ID: &str = "category_id";
const TAG_CATEGORY_NAME: &str = "category_name";
const TAG_DIRECT_SOURCE: &str = "direct_source";
const TAG_PARENT_ID: &str = "parent_id";

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error!(M3uFilterErrorKind::Notify, "failed to write xtream playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

macro_rules! try_option_ok {
    ($option:expr) => {
        match $option {
            Some(value) => value,
            None => return Ok(()),
        }
    };
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
        let xtream_path = storage_path.join(format!("{FILE_SERIES_EPISODES}.{FILE_SUFFIX_DB}"));
        let index_path = storage_path.join(format!("{FILE_SERIES_EPISODES}.{FILE_SUFFIX_INDEX}"));
        return Some((xtream_path, index_path));
    }
    None
}

fn write_playlists_to_file(cfg: &Config, storage_path: &Path, collections: Vec<(XtreamCluster, &mut [PlaylistItem])>) -> Result<(), M3uFilterError> {
    for (cluster, playlist) in collections {
        let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, cluster);
        {
            let _file_lock = cfg.file_locks.write_lock(&xtream_path).map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, format!("{err}")))?;
            match IndexedDocumentWriter::new(xtream_path.clone(), idx_path) {
                Ok(mut writer) => {
                    for item in playlist {
                        match item.to_xtream() {
                            Ok(xtream) => {
                                match writer.write_doc(item.header.borrow().virtual_id, &xtream) {
                                    Ok(()) => {}
                                    Err(err) => return Err(cant_write_result!(&xtream_path, err))
                                }
                            }
                            Err(err) => return Err(cant_write_result!(&xtream_path, err))
                        }
                    }
                    writer.store().map_err(|err| cant_write_result!(&xtream_path, err))?;
                }
                Err(err) => return Err(cant_write_result!(&xtream_path, err))
            }
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
                        if let Some(category_id) = get_map_item_as_str(item, TAG_CATEGORY_ID) {
                            if let Some(category_name) = get_map_item_as_str(item, TAG_CATEGORY_NAME) {
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
    get_target_storage_path(cfg, target_name).map(|target_path| target_path.join(PathBuf::from(PATH_XTREAM)))
}

pub(crate) fn xtream_get_epg_file_path(path: &Path) -> PathBuf {
    path.join(FILE_EPG)
}

fn xtream_get_file_paths_for_name(storage_path: &Path, name: &str) -> (PathBuf, PathBuf) {
    let xtream_path = storage_path.join(format!("{name}.{FILE_SUFFIX_DB}"));
    let index_path = storage_path.join(format!("{name}.{FILE_SUFFIX_INDEX}"));
    (xtream_path, index_path)
}

pub(crate) fn xtream_get_file_paths(storage_path: &Path, cluster: XtreamCluster) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, &cluster.as_str().to_lowercase())
}

pub(crate) fn xtream_get_file_paths_for_series(storage_path: &Path) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, FILE_SERIES)
}

fn xtream_garbage_collect(config: &Config, target_name: &str) -> std::io::Result<()> {
    // Garbage collect series
    let storage_path = try_option_ok!(xtream_get_storage_path(config, target_name));
    let (info_path, idx_path) = try_option_ok!(xtream_get_info_file_paths(&storage_path, XtreamCluster::Series));
    {
        let _file_lock = config.file_locks.write_lock(&info_path)?;
        IndexedDocumentGarbageCollector::new(info_path, idx_path)?.garbage_collect()?;
    }
    Ok(())
}

pub(crate) fn xtream_write_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut [PlaylistGroup]) -> Result<(), M3uFilterError> {
    let path = ensure_xtream_storage_path(cfg, target.name.as_str())?;
    let mut errors = Vec::new();
    let mut cat_live_col = vec![];
    let mut cat_series_col = vec![];
    let mut cat_vod_col = vec![];
    let mut live_col = vec![];
    let mut series_col = vec![];
    let mut vod_col = vec![];

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
              TAG_CATEGORY_ID: format!("{}", &cat_id),
              TAG_CATEGORY_NAME: plg.title.clone(),
              TAG_PARENT_ID: 0
            }));

            for pli in plg.channels.drain(..) {
                let mut header = pli.header.borrow_mut();
                // we skip resolved series, because this is only necessary when writing m3u files
                let col = if header.item_type == PlaylistItemType::Series {
                    None
                } else if header.get_provider_id().is_some() {
                    header.category_id = *cat_id;
                    Some(match header.xtream_cluster {
                        XtreamCluster::Live => &mut live_col,
                        XtreamCluster::Series => &mut series_col,
                        XtreamCluster::Video => &mut vod_col,
                    })
                } else {
                    let title = header.title.as_str();
                    errors.push(format!("Channel does not have an id: {title}"));
                    None
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

    match write_playlists_to_file(cfg, &path, vec![
        (XtreamCluster::Live, &mut live_col),
        (XtreamCluster::Video, &mut vod_col),
        (XtreamCluster::Series, &mut series_col)]) {
        Ok(()) => {
            if let Err(err) = xtream_garbage_collect(cfg, &target.name) {
                if err.kind() != ErrorKind::NotFound {
                    errors.push(format!("Garbage collection failed:{err}"));
                }
            }
        }
        Err(err) => {
            errors.push(format!("Persisting collection failed:{err}"));
        }
    }

    if !errors.is_empty() {
        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", errors.join("\n"));
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

fn xtream_read_item_for_stream_id(cfg: &Config, stream_id: u32, storage_path: &Path, cluster: XtreamCluster) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, cluster);
    {
        let _file_lock = cfg.file_locks.read_lock(&xtream_path)?;
        IndexedDocumentReader::<XtreamPlaylistItem>::read_indexed_item(&xtream_path, &idx_path, stream_id)
    }
}

fn xtream_read_series_item_for_stream_id(cfg: &Config, stream_id: u32, storage_path: &Path) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths_for_series(storage_path);
    {
        let _file_lock = cfg.file_locks.read_lock(&xtream_path)?;
        IndexedDocumentReader::<XtreamPlaylistItem>::read_indexed_item(&xtream_path, &idx_path, stream_id)
    }
}

macro_rules! try_cluster {
    ($xtream_cluster:expr, $item_type:expr, $virtual_id:expr) => {
        $xtream_cluster.or_else(|| XtreamCluster::try_from($item_type).ok())
            .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not determine cluster for xtream item with stream-id {}", $virtual_id)))
    };
}

pub(crate) fn xtream_get_item_for_stream_id(
    virtual_id: u32,
    config: &Config,
    target: &ConfigTarget,
    xtream_cluster: Option<XtreamCluster>,
) -> Result<XtreamPlaylistItem, Error> {
    let target_path = get_target_storage_path(config, target.name.as_str())
        .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {}", &target.name)))?;
    let storage_path = xtream_get_storage_path(config, target.name.as_str())
        .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {} xtream output", &target.name)))?;
    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.read_lock(&target_id_mapping_file)
            .map_err(|err| Error::new(ErrorKind::Other, format!("Could not get lock for id mapping for target {} err:{err}", target.name)))?;

        let mut target_id_mapping = BPlusTreeQuery::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file)
            .map_err(|err| Error::new(ErrorKind::Other, format!("Could not load id mapping for target {} err:{err}", target.name)))?;

        let mapping = target_id_mapping
            .query(&virtual_id)
            .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find mapping for target {} and id {}", target.name, virtual_id)))?;

        match mapping.item_type {
            PlaylistItemType::SeriesInfo => xtream_read_series_item_for_stream_id(config, virtual_id, &storage_path),
            PlaylistItemType::SeriesEpisode => {
                let mut item = xtream_read_series_item_for_stream_id(config, mapping.parent_virtual_id, &storage_path)?;
                item.provider_id = mapping.provider_id;
                Ok(item)
            }
            PlaylistItemType::Catchup => {
                let cluster = try_cluster!(xtream_cluster, mapping.item_type, virtual_id)?;
                let mut item = xtream_read_item_for_stream_id(config, mapping.parent_virtual_id, &storage_path, cluster)?;
                item.provider_id = mapping.provider_id;
                Ok(item)
            }
            _ => {
                let cluster = try_cluster!(xtream_cluster, mapping.item_type, virtual_id)?;
                xtream_read_item_for_stream_id(config, virtual_id, &storage_path, cluster)
            }
        }
    }
}


pub(crate) fn xtream_load_rewrite_playlist(cluster: XtreamCluster, config: &Config, target: &ConfigTarget, category_id: u32) -> Result<String, Error> {
    if let Some(storage_path) = xtream_get_storage_path(config, target.name.as_str()) {
        let (xtream_path, idx_path) = xtream_get_file_paths(&storage_path, cluster);
        {
            let _file_lock = config.file_locks.read_lock(&xtream_path)?;
            match IndexedDocumentReader::<XtreamPlaylistItem>::new(&xtream_path, &idx_path) {
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
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to find xtream storage for target {}", &target.name)))
}

pub(crate) fn xtream_write_series_info(config: &Config, target_name: &str,
                                       series_info_id: u32,
                                       content: &str) -> Result<(), Error> {
    let target_path = try_option_ok!(get_target_storage_path(config, target_name));
    let storage_path = try_option_ok!(xtream_get_storage_path(config, target_name));
    let (info_path, idx_path) = try_option_ok!(xtream_get_info_file_paths(&storage_path, XtreamCluster::Series));

    {
        let _file_lock = config.file_locks.write_lock(&info_path)?;
        let mut writer = IndexedDocumentWriter::new_append(info_path.clone(), idx_path)?;
        writer
            .write_doc(series_info_id, content)
            .map_err(|_| Error::new(ErrorKind::Other, format!("failed to write xtream series info for target {target_name}")))?;

        writer.store()?;
    }
    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.write_lock(&target_id_mapping_file)?;
        if let Ok(mut target_id_mapping) = BPlusTreeUpdate::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file) {
            if let Some(record) = target_id_mapping.query(&series_info_id) {
                let new_record = record.copy_update_timestamp();
                let _ = target_id_mapping.update(&series_info_id, new_record);
            }
        };
    }

    Ok(())
}

// Reads the series info entry if exists, otherwise error
pub(crate) fn xtream_load_series_info(config: &Config, target_name: &str, series_id: u32) -> Option<String> {
    let target_path = get_target_storage_path(config, target_name)?;
    let storage_path = xtream_get_storage_path(config, target_name)?;

    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.read_lock(&target_id_mapping_file).map_err(|err| {
            error!("Could not lock id mapping for target {target_name}: {}", err);
            Error::new(ErrorKind::Other, format!("ID mapping load error for target {target_name}"))
        }).ok()?;
        let mut target_id_mapping = BPlusTreeQuery::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file)
            .map_err(|err| {
                error!("Could not load id mapping for target {target_name}: {}", err);
                Error::new(ErrorKind::Other, format!("ID mapping load error for target {target_name}"))
            }).ok()?;

        if let Some(id_record) = target_id_mapping.query(&series_id) {
            if id_record.is_expired() {
                return None;
            }
        }
    }

    let (info_path, idx_path) = xtream_get_info_file_paths(&storage_path, XtreamCluster::Series)?;

    if info_path.exists() && idx_path.exists() {
        {
            let _file_lock = config.file_locks.read_lock(&info_path).map_err(|err| {
                error!("Could not lock document {:?}: {}", info_path, err);
                Error::new(ErrorKind::Other, format!("Document Reader error for target {target_name}"))
            }).ok()?;
            return match IndexedDocumentReader::<String>::read_indexed_item(&info_path, &idx_path, series_id) {
                Ok(content) => Some(content),
                Err(err) => {
                    error!("Failed to read series info for id {series_id} for {target_name}: {}", err);
                    None
                }
            };
        }
    }
    None
}

pub(crate) fn write_and_get_xtream_series_info(
    config: &Config,
    target: &ConfigTarget,
    pli_series_info: &XtreamPlaylistItem,
    content: &str,
) -> Result<String, Error> {
    let mut doc = serde_json::from_str::<Value>(content)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to parse JSON content"))?;

    let target_path = get_target_storage_path(config, target.name.as_str())
        .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {}", target.name)))?;

    let episodes = doc.get_mut("episodes")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::new(ErrorKind::Other, "No episodes found in content"))?;

    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.write_lock(&target_id_mapping_file)
            .map_err(|err| Error::new(ErrorKind::Other, format!("Could not load id mapping for target {} err:{err}", target.name)))?;
        let mut target_id_mapping = TargetIdMapping::new(&target_id_mapping_file);
        let options = XtreamMappingOptions::from_target_options(target.options.as_ref());

        for episode_list in episodes.values_mut().filter_map(Value::as_array_mut) {
            for episode in episode_list.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(provider_id) = episode.get("id").and_then(Value::as_str).and_then(|id| id.parse::<u32>().ok()) {
                    let uuid = hash_string(&format!("{}/{}", pli_series_info.url, provider_id));
                    let virtual_id = target_id_mapping.insert_entry(uuid, provider_id, PlaylistItemType::SeriesEpisode, pli_series_info.virtual_id);
                    episode.insert("id".to_string(), Value::String(virtual_id.to_string()));
                }
                if options.skip_series_direct_source {
                    episode.insert(TAG_DIRECT_SOURCE.to_string(), Value::String(String::new()));
                }
            }
        }

        drop(target_id_mapping);
    }
    let result = serde_json::to_string(&doc)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to serialize updated series info"))?;
    xtream_write_series_info(config, target.name.as_str(), pli_series_info.virtual_id, &result).ok();

    Ok(result)
}