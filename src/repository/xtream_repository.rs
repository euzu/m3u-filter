use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Error, ErrorKind, Read};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

use log::error;
use serde_json::{json, Map, Value};

use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistEntry, PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::bplustree::{BPlusTree, BPlusTreeQuery, BPlusTreeUpdate};
use crate::repository::indexed_document::{IndexedDocumentGarbageCollector, IndexedDocumentWriter, IndexedDocumentDirectAccess};
use crate::repository::storage::{get_input_storage_path, get_target_id_mapping_file, get_target_storage_path, hash_string, FILE_SUFFIX_DB, FILE_SUFFIX_INDEX};
use crate::repository::target_id_mapping::{TargetIdMapping, VirtualIdRecord};
use crate::repository::xtream_playlist_iterator::XtreamPlaylistIterator;
use crate::utils::json_utils::{json_iter_array, json_write_documents_to_file};
use crate::{create_m3u_filter_error, create_m3u_filter_error_result, notify_err, info_err};

pub static COL_CAT_LIVE: &str = "cat_live";
pub static COL_CAT_SERIES: &str = "cat_series";
pub static COL_CAT_VOD: &str = "cat_vod";
const FILE_SERIES_EPISODES: &str = "series_episodes";
const FILE_VOD_INFO: &str = "vod_info";
const FILE_VOD_INFO_RECORD: &str = "vod_info_record";
const FILE_SERIES_INFO_RECORD: &str = "series_info_record";
const FILE_SERIES_EPISODE_RECORD: &str = "series_episode_record";
const FILE_SERIES: &str = "series";
pub const FILE_EPG: &str = "epg.xml";
const PATH_XTREAM: &str = "xtream";
const TAG_CATEGORY_ID: &str = "category_id";
const TAG_CATEGORY_NAME: &str = "category_name";
const TAG_DIRECT_SOURCE: &str = "direct_source";
const TAG_PARENT_ID: &str = "parent_id";
const TAG_MOVIE_DATA: &str = "movie_data";
const TAG_STREAM_ID: &str = "stream_id";

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error!(
            M3uFilterErrorKind::Notify,
            "failed to write xtream playlist: {} - {}",
            $path.to_str().unwrap(),
            $err
        )
    };
}

macro_rules! try_option_ok {
    ($option:expr) => {
        match $option {
            Some(value) => value,
            None => return Ok(()),
        }
    };
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputVodInfoRecord {
    pub(crate) tmdb_id: u32,
    pub(crate) ts: u64,
}

fn get_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{collection}.json"))
}

fn ensure_xtream_storage_path(cfg: &Config, target_name: &str) -> Result<PathBuf, M3uFilterError> {
    if let Some(path) = xtream_get_storage_path(cfg, target_name) {
        if std::fs::create_dir_all(&path).is_err() {
            let msg = format!(
                "Failed to save xtream data, can't create directory {}",
                &path.to_str().unwrap()
            );
            return Err(notify_err!(msg));
        }
        Ok(path)
    } else {
        let msg = format!("Failed to save xtream data, can't create directory for target {target_name}");
        Err(notify_err!(msg))
    }
}

pub fn xtream_get_info_file_paths(
    storage_path: &Path,
    cluster: XtreamCluster,
) -> Option<(PathBuf, PathBuf)> {
    if cluster == XtreamCluster::Series {
        let xtream_path = storage_path.join(format!("{FILE_SERIES_EPISODES}.{FILE_SUFFIX_DB}"));
        let index_path = storage_path.join(format!("{FILE_SERIES_EPISODES}.{FILE_SUFFIX_INDEX}"));
        return Some((xtream_path, index_path));
    } else if cluster == XtreamCluster::Video {
        let xtream_path = storage_path.join(format!("{FILE_VOD_INFO}.{FILE_SUFFIX_DB}"));
        let index_path = storage_path.join(format!("{FILE_VOD_INFO}.{FILE_SUFFIX_INDEX}"));
        return Some((xtream_path, index_path));
    }
    None
}

pub fn xtream_get_record_file_path(storage_path: &Path, item_type: PlaylistItemType) -> Option<PathBuf> {
    match item_type {
        PlaylistItemType::Video => Some(storage_path.join(format!("{FILE_VOD_INFO_RECORD}.{FILE_SUFFIX_DB}"))),
        PlaylistItemType::SeriesInfo | PlaylistItemType::Series => Some(storage_path.join(format!("{FILE_SERIES_INFO_RECORD}.{FILE_SUFFIX_DB}"))),
        PlaylistItemType::SeriesEpisode => Some(storage_path.join(format!("{FILE_SERIES_EPISODE_RECORD}.{FILE_SUFFIX_DB}"))),
        _ => None,
    }
}
async fn write_playlists_to_file(
    cfg: &Config,
    storage_path: &Path,
    collections: Vec<(XtreamCluster, &mut [&PlaylistItem])>,
) -> Result<(), M3uFilterError> {
    for (cluster, playlist) in collections {
        let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, cluster);
        {
            let _file_lock = cfg.file_locks.write_lock(&xtream_path).await.map_err(|err| info_err!(format!("{err}")))?;
            match IndexedDocumentWriter::new(xtream_path.clone(), idx_path) {
                Ok(mut writer) => {
                    for item in playlist {
                        let xtream = item.to_xtream();
                        match writer.write_doc(item.header.borrow().virtual_id, &xtream) {
                            Ok(()) => {}
                            Err(err) => return Err(cant_write_result!(&xtream_path, err)),
                        }
                    }
                    writer.store().map_err(|err| cant_write_result!(&xtream_path, err))?;
                }
                Err(err) => return Err(cant_write_result!(&xtream_path, err)),
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
    for cat in [COL_CAT_LIVE, COL_CAT_VOD, COL_CAT_SERIES] {
        let col_path = get_collection_path(path, cat);
        if col_path.exists() {
            if let Ok(file) = File::open(col_path) {
                let reader = BufReader::new(file);
                for entry in json_iter_array::<Value, BufReader<File>>(reader).flatten() {
                    if let Some(item) = entry.as_object() {
                        if let Some(category_id) = get_map_item_as_str(item, TAG_CATEGORY_ID) {
                            if let Some(category_name) = get_map_item_as_str(item, TAG_CATEGORY_NAME)
                            {
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

pub fn xtream_get_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    get_target_storage_path(cfg, target_name).map(|target_path| target_path.join(PathBuf::from(PATH_XTREAM)))
}

pub fn xtream_get_epg_file_path(path: &Path) -> PathBuf {
    path.join(FILE_EPG)
}

fn xtream_get_file_paths_for_name(storage_path: &Path, name: &str) -> (PathBuf, PathBuf) {
    let xtream_path = storage_path.join(format!("{name}.{FILE_SUFFIX_DB}"));
    let index_path = storage_path.join(format!("{name}.{FILE_SUFFIX_INDEX}"));
    (xtream_path, index_path)
}

pub fn xtream_get_file_paths(storage_path: &Path, cluster: XtreamCluster) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, &cluster.as_str().to_lowercase())
}

pub fn xtream_get_file_paths_for_series(storage_path: &Path) -> (PathBuf, PathBuf) {
    xtream_get_file_paths_for_name(storage_path, FILE_SERIES)
}

async fn xtream_garbage_collect(config: &Config, target_name: &str) -> std::io::Result<()> {
    // Garbage collect series
    let storage_path = try_option_ok!(xtream_get_storage_path(config, target_name));
    let (info_path, idx_path) = try_option_ok!(xtream_get_info_file_paths(
        &storage_path,
        XtreamCluster::Series
    ));
    {
        let _file_lock = config.file_locks.write_lock(&info_path).await?;
        IndexedDocumentGarbageCollector::<u32>::new(info_path, idx_path)?.garbage_collect()?;
    }
    Ok(())
}

pub async fn xtream_write_playlist(
    target: &ConfigTarget,
    cfg: &Config,
    playlist: &mut [PlaylistGroup],
) -> Result<(), M3uFilterError> {
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

            for pli in &plg.channels {
                let mut header = pli.header.borrow_mut();
                let col = match header.item_type {
                    PlaylistItemType::Series => {
                        // we skip resolved series, because this is only necessary when writing m3u files
                        None
                    }
                    PlaylistItemType::LiveUnknown | PlaylistItemType::LiveHls => {
                        header.category_id = *cat_id;
                        Some(&mut live_col)
                    }
                    _ => {
                        if header.get_provider_id().is_some() {
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
        (get_collection_path(&path, COL_CAT_SERIES), &cat_series_col),
    ] {
        match json_write_documents_to_file(&col_path, data) {
            Ok(()) => {}
            Err(err) => {
                errors.push(format!("Persisting collection failed: {}: {}", &col_path.to_str().unwrap(), err));
            }
        }
    }

    match write_playlists_to_file(
        cfg,
        &path,
        vec![
            (XtreamCluster::Live, &mut live_col),
            (XtreamCluster::Video, &mut vod_col),
            (XtreamCluster::Series, &mut series_col),
        ],
    ).await {
        Ok(()) => {
            if let Err(err) = xtream_garbage_collect(cfg, &target.name).await {
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
        return create_m3u_filter_error_result!(
            M3uFilterErrorKind::Notify,
            "{}",
            errors.join("\n")
        );
    }

    Ok(())
}

pub fn xtream_get_collection_path(
    cfg: &Config,
    target_name: &str,
    collection_name: &str,
) -> Result<(Option<PathBuf>, Option<String>), Error> {
    if let Some(path) = xtream_get_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok((Some(col_path), None));
        }
    }
    Err(Error::new(
        ErrorKind::Other,
        format!("Cant find collection: {target_name}/{collection_name}"),
    ))
}

async fn xtream_read_item_for_stream_id(
    cfg: &Config,
    stream_id: u32,
    storage_path: &Path,
    cluster: XtreamCluster,
) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths(storage_path, cluster);
    {
        let _file_lock = cfg.file_locks.read_lock(&xtream_path).await?;
        IndexedDocumentDirectAccess::read_indexed_item::<u32, XtreamPlaylistItem>(&xtream_path, &idx_path, &stream_id)
    }
}

async fn xtream_read_series_item_for_stream_id(
    cfg: &Config,
    stream_id: u32,
    storage_path: &Path,
) -> Result<XtreamPlaylistItem, Error> {
    let (xtream_path, idx_path) = xtream_get_file_paths_for_series(storage_path);
    {
        let _file_lock = cfg.file_locks.read_lock(&xtream_path).await?;
        IndexedDocumentDirectAccess::read_indexed_item::<u32, XtreamPlaylistItem>(&xtream_path, &idx_path, &stream_id)
    }
}

macro_rules! try_cluster {
    ($xtream_cluster:expr, $item_type:expr, $virtual_id:expr) => {
        $xtream_cluster
            .or_else(|| XtreamCluster::try_from($item_type).ok())
            .ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not determine cluster for xtream item with stream-id {}",$virtual_id)))
    };
}

pub async fn xtream_get_item_for_stream_id(
    virtual_id: u32,
    config: &Config,
    target: &ConfigTarget,
    xtream_cluster: Option<XtreamCluster>,
) -> Result<XtreamPlaylistItem, Error> {
    let target_path = get_target_storage_path(config, target.name.as_str()).ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {}", &target.name)))?;
    let storage_path = xtream_get_storage_path(config, target.name.as_str()).ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {} xtream output", &target.name)))?;
    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.read_lock(&target_id_mapping_file).await.map_err(|err| Error::new(ErrorKind::Other, format!("Could not get lock for id mapping for target {} err:{err}", target.name)))?;

        let mut target_id_mapping = BPlusTreeQuery::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file).map_err(|err| Error::new(ErrorKind::Other, format!("Could not load id mapping for target {} err:{err}", target.name)))?;
        let mapping = target_id_mapping.query(&virtual_id).ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find mapping for target {} and id {}", target.name, virtual_id)))?;
        match mapping.item_type {
            PlaylistItemType::SeriesInfo => {
                xtream_read_series_item_for_stream_id(config, virtual_id, &storage_path).await
            }
            PlaylistItemType::SeriesEpisode => {
                let mut item = xtream_read_series_item_for_stream_id(config, mapping.parent_virtual_id, &storage_path).await?;
                item.provider_id = mapping.provider_id;
                Ok(item)
            }
            PlaylistItemType::Catchup => {
                let cluster = try_cluster!(xtream_cluster, mapping.item_type, virtual_id)?;
                let mut item = xtream_read_item_for_stream_id(config, mapping.parent_virtual_id, &storage_path, cluster).await?;
                item.provider_id = mapping.provider_id;
                Ok(item)
            }
            _ => {
                let cluster = try_cluster!(xtream_cluster, mapping.item_type, virtual_id)?;
                xtream_read_item_for_stream_id(config, virtual_id, &storage_path, cluster).await
            }
        }
    }
}

pub async fn xtream_load_rewrite_playlist(
    cluster: XtreamCluster,
    config: &Config,
    target: &ConfigTarget,
    category_id: u32,
) -> Result<Box<dyn Iterator<Item=String>>, M3uFilterError> {
    Ok(Box::new(XtreamPlaylistIterator::new(cluster, config, target, category_id).await?))
}

pub async fn xtream_write_series_info(
    config: &Config,
    target_name: &str,
    series_info_id: u32,
    content: &str,
) -> Result<(), Error> {
    let target_path = try_option_ok!(get_target_storage_path(config, target_name));
    let storage_path = try_option_ok!(xtream_get_storage_path(config, target_name));
    let (info_path, idx_path) = try_option_ok!(xtream_get_info_file_paths(
        &storage_path,
        XtreamCluster::Series
    ));

    {
        let _file_lock = config.file_locks.write_lock(&info_path).await?;
        let mut writer = IndexedDocumentWriter::new_append(info_path, idx_path)?;
        writer.write_doc(series_info_id, content).map_err(|_| Error::new(ErrorKind::Other, format!("failed to write xtream series info for target {target_name}")))?;
        writer.store()?;
    }
    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.write_lock(&target_id_mapping_file).await?;
        if let Ok(mut target_id_mapping) = BPlusTreeUpdate::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file) {
            if let Some(record) = target_id_mapping.query(&series_info_id) {
                let new_record = record.copy_update_timestamp();
                let _ = target_id_mapping.update(&series_info_id, new_record);
            }
        };
    }

    Ok(())
}

pub async fn xtream_write_vod_info(
    config: &Config,
    target_name: &str,
    virtual_id: u32,
    content: &str,
) -> Result<(), Error> {
    let storage_path = try_option_ok!(xtream_get_storage_path(config, target_name));
    let (info_path, idx_path) = try_option_ok!(xtream_get_info_file_paths(&storage_path, XtreamCluster::Video));
    {
        let _file_lock = config.file_locks.write_lock(&info_path).await?;
        let mut writer = IndexedDocumentWriter::new_append(info_path, idx_path)?;
        writer.write_doc(virtual_id, content).map_err(|_| Error::new(ErrorKind::Other, format!("failed to write xtream vod info for target {target_name}")))?;
        writer.store()?;
    }
    Ok(())
}

async fn xtream_get_series_info_mapping(
    config: &Config,
    target_name: &str,
    series_id: u32,
) -> Option<VirtualIdRecord> {
    xtream_get_info_mapping(config, target_name, series_id).await.filter(|id_record| !id_record.is_expired())
}

async fn xtream_get_info_mapping(config: &Config, target_name: &str, info_id: u32) -> Option<VirtualIdRecord> {
    let target_path = get_target_storage_path(config, target_name)?;

    let target_id_mapping_file = get_target_id_mapping_file(&target_path);
    let _file_lock = config.file_locks.read_lock(&target_id_mapping_file).await.map_err(|err| {
        error!("Could not lock id mapping for target {target_name}: {}", err);
        Error::new(ErrorKind::Other, format!("ID mapping load error for target {target_name}"))
    }).ok()?;
    BPlusTreeQuery::<u32, VirtualIdRecord>::try_new(&target_id_mapping_file).map_err(|err| {
        error!("Could not load id mapping for target {target_name}: {}", err);
        Error::new(ErrorKind::Other, format!("ID mapping load error for target {target_name}"))
    }).ok().map(|mut tree| tree.query(&info_id))?
}

// Reads the series info entry if exists
pub async fn xtream_load_series_info(
    config: &Config,
    target_name: &str,
    series_id: u32,
) -> Option<String> {
    xtream_get_series_info_mapping(config, target_name, series_id).await?;

    let storage_path = xtream_get_storage_path(config, target_name)?;

    let (info_path, idx_path) = xtream_get_info_file_paths(&storage_path, XtreamCluster::Series)?;

    if info_path.exists() && idx_path.exists() {
        {
            let _file_lock = config.file_locks.read_lock(&info_path).await.map_err(|err| {
                error!("Could not lock document {:?}: {}", info_path, err);
                Error::new(ErrorKind::Other, format!("Document Reader error for target {target_name}"))
            }).ok()?;
            return match IndexedDocumentDirectAccess::read_indexed_item::<u32, String>(&info_path, &idx_path, &series_id) {
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

async fn xtream_get_vod_info_mapping(
    config: &Config,
    target_name: &str,
    vod_id: u32,
) -> Option<VirtualIdRecord> {
    xtream_get_info_mapping(config, target_name, vod_id).await
    //.filter(|id_record| !id_record.is_expired())
}

// Reads the vod info entry if exists
pub async fn xtream_load_vod_info(
    config: &Config,
    target_name: &str,
    vod_id: u32,
) -> Option<String> {

    // Check if the entry exists; if not, we don't need to look further.
    xtream_get_vod_info_mapping(config, target_name, vod_id).await.as_ref()?;
    // Entry exists, read db entry
    let target_storage_path = xtream_get_storage_path(config, target_name)?;

    let (info_path, idx_path) = xtream_get_info_file_paths(&target_storage_path, XtreamCluster::Video)?;

    if info_path.exists() && idx_path.exists() {
        {
            let _file_lock = config.file_locks.read_lock(&info_path).await.map_err(|err| {
                error!("Could not lock document {:?}: {}", info_path, err);
                Error::new(ErrorKind::Other, format!("Document Reader error for target {target_name}"))
            }).ok()?;
            return match IndexedDocumentDirectAccess::read_indexed_item::<u32, String>(&info_path, &idx_path, &vod_id) {
                Ok(content) => Some(content),
                Err(_err) => {
                    // this is not an error, it means the info is not indexed
                    // error!("Failed to read vod info for id {vod_id} for {target_name}: {}",err);
                    None
                }
            };
        }
    }
    None
}

pub async fn write_and_get_xtream_vod_info<P>(
    config: &Config,
    target: &ConfigTarget,
    pli: &P,
    content: &str,
) -> Result<String, Error> where
    P: PlaylistEntry,
{
    let mut doc = serde_json::from_str::<Map<String, Value>>(content).map_err(|_| Error::new(ErrorKind::Other, "Failed to parse JSON content"))?;

    // wen need to update the movie data with virtual ids.
    if let Some(Value::Object(movie_data)) = doc.get_mut(TAG_MOVIE_DATA) {
        let stream_id = pli.get_virtual_id();
        let category_id = pli.get_category_id().unwrap_or(0);
        movie_data.insert(
            TAG_STREAM_ID.to_string(),
            Value::Number(serde_json::value::Number::from(stream_id)),
        );
        movie_data.insert(
            TAG_CATEGORY_ID.to_string(),
            Value::Number(serde_json::value::Number::from(category_id)),
        );
        let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
        if options.skip_video_direct_source {
            movie_data.insert(TAG_DIRECT_SOURCE.to_string(), Value::String(String::new()));
        } else {
            movie_data.insert(
                TAG_DIRECT_SOURCE.to_string(),
                Value::String(pli.get_provider_url().to_string()),
            );
        }
    }
    let result = serde_json::to_string(&doc).map_err(|_| Error::new(ErrorKind::Other, "Failed to serialize vod info"))?;
    xtream_write_vod_info(config, target.name.as_str(), pli.get_virtual_id(), &result).await.ok();

    Ok(result)
}

pub async fn write_and_get_xtream_series_info<P>(
    config: &Config,
    target: &ConfigTarget,
    pli_series_info: &P,
    content: &str,
) -> Result<String, Error> where
    P: PlaylistEntry,
{
    let mut doc = serde_json::from_str::<Value>(content).map_err(|_| Error::new(ErrorKind::Other, "Failed to parse JSON content"))?;

    let target_path = get_target_storage_path(config, target.name.as_str()).ok_or_else(|| Error::new(ErrorKind::Other, format!("Could not find path for target {}", target.name)))?;
    let episodes = doc.get_mut("episodes").and_then(Value::as_object_mut).ok_or_else(|| Error::new(ErrorKind::Other, "No episodes found in content"))?;

    let virtual_id = pli_series_info.get_virtual_id();
    {
        let target_id_mapping_file = get_target_id_mapping_file(&target_path);
        let _file_lock = config.file_locks.write_lock(&target_id_mapping_file).await.map_err(|err| Error::new(ErrorKind::Other, format!("Could not load id mapping for target {} err:{err}", target.name)))?;
        let mut target_id_mapping = TargetIdMapping::new(&target_id_mapping_file);
        let options = XtreamMappingOptions::from_target_options(target.options.as_ref());

        let provider_url = pli_series_info.get_provider_url();
        for episode_list in episodes.values_mut().filter_map(Value::as_array_mut) {
            for episode in episode_list.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(provider_id) = episode.get("id").and_then(Value::as_str).and_then(|id| id.parse::<u32>().ok())
                {
                    let uuid = hash_string(&format!("{provider_url}/{provider_id}"));
                    let episode_virtual_id = target_id_mapping.insert_entry(
                        uuid,
                        provider_id,
                        PlaylistItemType::SeriesEpisode,
                        virtual_id,
                    );
                    episode.insert(
                        "id".to_string(),
                        Value::String(episode_virtual_id.to_string()),
                    );
                }
                if options.skip_series_direct_source {
                    episode.insert(TAG_DIRECT_SOURCE.to_string(), Value::String(String::new()));
                }
            }
        }

        drop(target_id_mapping);
    }
    let result = serde_json::to_string(&doc).map_err(|_| Error::new(ErrorKind::Other, "Failed to serialize updated series info"))?;
    xtream_write_series_info(config, target.name.as_str(), virtual_id, &result).await.ok();

    Ok(result)
}

pub async fn xtream_get_input_info(
    cfg: &Config,
    input: &ConfigInput,
    provider_id: u32,
    cluster: XtreamCluster,
) -> Option<String> {
    if let Ok(Some((info_path, idx_path))) = get_input_storage_path(input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, cluster))
    {
        if let Ok(_file_lock) = cfg.file_locks.read_lock(&info_path).await {
            if let Ok(content) = IndexedDocumentDirectAccess::read_indexed_item::<u32, String>(&info_path, &idx_path, &provider_id) {
                return Some(content);
            }
        }
    }
    None
}

pub async fn xtream_update_input_info_file(
    cfg: &Config,
    input: &ConfigInput,
    wal_file: &mut File,
    wal_path: &Path,
    cluster: XtreamCluster,
) -> Result<(), M3uFilterError> {
    match get_input_storage_path(input, &cfg.working_dir).map(|storage_path| xtream_get_info_file_paths(&storage_path, cluster)) {
        Ok(Some((info_path, idx_path))) => {
            match cfg.file_locks.write_lock(&info_path).await {
                Ok(_file_lock) => {
                    wal_file.seek(SeekFrom::Start(0)).map_err(|err| notify_err!(format!("Could not read {cluster} info {err}")))?;
                    let mut reader = BufReader::new(wal_file);
                    match IndexedDocumentWriter::<u32>::new_append(info_path, idx_path) {
                        Ok(mut writer) => {
                            let mut provider_id_bytes = [0u8; 4];
                            let mut length_bytes = [0u8; 4];
                            loop {
                                if reader.read_exact(&mut provider_id_bytes).is_err() {
                                    break; // End of file
                                }
                                let provider_id = u32::from_le_bytes(provider_id_bytes);
                                reader.read_exact(&mut length_bytes).map_err(|err| notify_err!(format!("Could not read temporary {cluster} info {err}")))?;
                                let length = u32::from_le_bytes(length_bytes) as usize;
                                let mut buffer = vec![0u8; length];
                                reader.read_exact(&mut buffer).map_err(|err| notify_err!(format!("Could not read temporary {cluster} info {err}")))?;
                                if let Ok(content) = String::from_utf8(buffer) {
                                    let _ = writer.write_doc(provider_id, &content);
                                }
                            }
                            writer.store().map_err(|err| notify_err!(format!("Could not store {cluster} info {err}")))?;
                            drop(reader);
                            if let Err(err) = fs::remove_file(wal_path) {
                                error!("Failed to delete WAL file for {cluster} {err}");
                            }
                            Ok(())
                        }
                        Err(err) => Err(notify_err!(format!("Could not create create indexed document writer for {cluster} info {err}"))),
                    }
                }
                Err(err) => Err(info_err!(format!("{err}"))),
            }
        }
        Ok(None) => Err(notify_err!(format!("Could not create storage path for input {}", &input.name.as_ref().map_or("?", |v| v)))),
        Err(err) => Err(notify_err!(format!("Could not create storage path for input {err}"))),
    }
}

pub async fn xtream_update_input_vod_record_from_wal_file(
    cfg: &Config,
    input: &ConfigInput,
    wal_file: &mut File,
    wal_path: &Path,
) -> Result<(), M3uFilterError> {
    let record_path = get_input_storage_path(input, &cfg.working_dir).map(|storage_path| xtream_get_record_file_path(&storage_path, PlaylistItemType::Video))
        .map_err(|err| notify_err!(format!("Error accessing storage path: {err}")))
        .and_then(|opt| opt.ok_or_else(|| notify_err!(format!("Error accessing storage path for input: {}", input.name.clone().unwrap_or_else(|| input.id.to_string())))))?;

    match cfg.file_locks.write_lock(&record_path).await {
        Ok(_file_lock) => {
            wal_file.seek(SeekFrom::Start(0)).map_err(|err| notify_err!(format!("Could not read vod wal info {err}")))?;
            let mut reader = BufReader::new(wal_file);
            let mut provider_id_bytes = [0u8; 4];
            let mut tmdb_id_bytes = [0u8; 4];
            let mut ts_bytes = [0u8; 8];
            let mut tree_record_index: BPlusTree<u32, InputVodInfoRecord> = BPlusTree::load(&record_path).unwrap_or_else(|_| BPlusTree::new());
            loop {
                if reader.read_exact(&mut provider_id_bytes).is_err() {
                    break; // End of file
                }
                let provider_id = u32::from_le_bytes(provider_id_bytes);
                if reader.read_exact(&mut tmdb_id_bytes).is_err() {
                    break; // End of file
                }
                let tmdb_id = u32::from_le_bytes(tmdb_id_bytes);
                if reader.read_exact(&mut ts_bytes).is_err() {
                    break; // End of file
                }
                let ts = u64::from_le_bytes(ts_bytes);
                tree_record_index.insert(provider_id, InputVodInfoRecord { tmdb_id, ts });
            }
            tree_record_index.store(&record_path).map_err(|err| notify_err!(format!("Could not store vod record info {err}")))?;
            drop(reader);
            if let Err(err) = fs::remove_file(wal_path) {
                error!("Failed to delete record WAL file for vod {err}");
            }
            Ok(())
        }
        Err(err) => Err(info_err!(format!("{err}"))),
    }
}

pub async fn xtream_update_input_series_record_from_wal_file(
    cfg: &Config,
    input: &ConfigInput,
    wal_file: &mut File,
    wal_path: &Path,
) -> Result<(), M3uFilterError> {
    let record_path = get_input_storage_path(input, &cfg.working_dir).map(|storage_path| xtream_get_record_file_path(&storage_path, PlaylistItemType::SeriesInfo))
        .map_err(|err| notify_err!(format!("Error accessing storage path: {err}")))
        .and_then(|opt| opt.ok_or_else(|| notify_err!(format!("Error accessing storage path for input: {}", input.name.clone().unwrap_or_else(|| input.id.to_string())))))?;
    match cfg.file_locks.write_lock(&record_path).await {
        Ok(_file_lock) => {
            wal_file.seek(SeekFrom::Start(0)).map_err(|err| notify_err!(format!("Could not read series wal info {err}")))?;
            let mut reader = BufReader::new(wal_file);
            let mut provider_id_bytes = [0u8; 4];
            let mut ts_bytes = [0u8; 8];
            let mut tree_record_index: BPlusTree<u32, u64> = BPlusTree::load(&record_path).unwrap_or_else(|_| BPlusTree::new());
            loop {
                if reader.read_exact(&mut provider_id_bytes).is_err() {
                    break; // End of file
                }
                let provider_id = u32::from_le_bytes(provider_id_bytes);
                if reader.read_exact(&mut ts_bytes).is_err() {
                    break; // End of file
                }
                let ts = u64::from_le_bytes(ts_bytes);
                tree_record_index.insert(provider_id, ts);
            }
            tree_record_index.store(&record_path).map_err(|err| notify_err!(format!("Could not store series record info {err}")))?;
            drop(reader);
            if let Err(err) = fs::remove_file(wal_path) {
                error!("Failed to delete record WAL file for series {err}");
            }
            Ok(())
        }

        Err(err) => Err(info_err!(format!("{err}"))),
    }
}

pub async fn xtream_update_input_series_episodes_record_from_wal_file(
    cfg: &Config,
    input: &ConfigInput,
    wal_file: &mut File,
    wal_path: &Path,
) -> Result<(), M3uFilterError> {
    let record_path = get_input_storage_path(input, &cfg.working_dir).map(|storage_path| xtream_get_record_file_path(&storage_path, PlaylistItemType::SeriesEpisode))
        .map_err(|err| notify_err!(format!("Error accessing storage path: {err}")))
        .and_then(|opt| opt.ok_or_else(|| notify_err!(format!("Error accessing storage path for input: {}", input.name.clone().unwrap_or_else(|| input.id.to_string())))))?;
    match cfg.file_locks.write_lock(&record_path).await {
        Ok(_file_lock) => {
            wal_file.seek(SeekFrom::Start(0)).map_err(|err| notify_err!(format!("Could not read series episode wal info {err}")))?;
            let mut reader = BufReader::new(wal_file);
            let mut provider_id_bytes = [0u8; 4];
            let mut tmdb_id_bytes = [0u8; 4];
            let mut tree_record_index: BPlusTree<u32, u32> = BPlusTree::load(&record_path).unwrap_or_else(|_| BPlusTree::new());
            loop {
                if reader.read_exact(&mut provider_id_bytes).is_err() {
                    break; // End of file
                }
                let provider_id = u32::from_le_bytes(provider_id_bytes);
                if reader.read_exact(&mut tmdb_id_bytes).is_err() {
                    break; // End of file
                }
                let tmdb_id = u32::from_le_bytes(tmdb_id_bytes);
                tree_record_index.insert(provider_id, tmdb_id);
            }
            tree_record_index.store(&record_path).map_err(|err| notify_err!(format!("Could not store series episode record info {err}")))?;
            drop(reader);
            if let Err(err) = fs::remove_file(wal_path) {
                error!("Failed to delete record WAL file for series episode {err}");
            }
            Ok(())
        }

        Err(err) => Err(info_err!(format!("{err}"))),
    }
}
