use std::collections::{BTreeMap, HashMap};
use std::{fs, io};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use log::{error};
use serde::Serialize;
use serde_json::{json, Value};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::{create_m3u_filter_error_result};
use crate::api::api_model::AppState;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::xtream::{XtreamMappingOptions};
use crate::repository::repository_utils::IndexRecord;
use crate::utils::file_utils;
use crate::utils::file_utils::create_file_tuple;
use crate::utils::json_utils::iter_json_array;

type IndexTree = BTreeMap<i32, (u32, u16)>;

pub(crate) static COL_CAT_LIVE: &str = "cat_live";
pub(crate) static COL_CAT_SERIES: &str = "cat_series";
pub(crate) static COL_CAT_VOD: &str = "cat_vod";
pub(crate) static COL_LIVE: &str = "live";
pub(crate) static COL_SERIES: &str = "series";
pub(crate) static COL_VOD: &str = "vod";

pub(crate) fn get_xtream_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}

pub(crate) fn get_xtream_epg_file_path(path: &Path) -> PathBuf {
    path.join("epg.xml")
}

pub(crate) fn get_xtream_file_paths(storage_path: &Path, cluster: &XtreamCluster) -> (PathBuf, PathBuf) {
    let xtream_path = storage_path.join(format!("{}.db", match cluster {
        XtreamCluster::Live => COL_LIVE,
        XtreamCluster::Video => COL_VOD,
        XtreamCluster::Series => COL_SERIES
    }));
    let extension = xtream_path.extension().map(|ext| format!("{}_", ext.to_str().unwrap_or(""))).unwrap_or("".to_owned());
    let index_path = xtream_path.with_extension(format!("{}idx", &extension));
    (xtream_path, index_path)
}

pub(crate) fn get_xtream_id_index_file_path(storage_path: &Path) -> PathBuf {
    storage_path.join("stream_id.db")
}

fn get_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}.json", collection))
}

fn get_info_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}_info.db", collection))
}

fn get_info_idx_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}_info.idx", collection))
}

fn get_info_collection_and_idx_path(path: &Path, cluster: &XtreamCluster) -> (PathBuf, PathBuf) {
    let collection = match cluster {
        XtreamCluster::Live => COL_LIVE,
        XtreamCluster::Video => COL_VOD,
        XtreamCluster::Series => COL_SERIES,
    };
    (get_info_collection_path(path, collection), get_info_idx_path(path, collection))
}

fn write_xtream_info(app_state: &AppState, target_name: &str, stream_id: i32, cluster: &XtreamCluster,
                     content: &str, index_tree: &mut IndexTree) -> Result<(), Error> {
    if let Some(path) = get_xtream_storage_path(&app_state.config, target_name) {
        let (col_path, idx_path) = get_info_collection_and_idx_path(&path, cluster);
        let mut comp: Vec<u8> = Vec::new();
        lzma_rs::lzma_compress(&mut BufReader::new(content.as_bytes()), &mut comp)?;
        let size = comp.len();
        match OpenOptions::new()
            .create(true)
            //.write(true)
            .append(true)
            .open(col_path) {
            Ok(mut file) => {
                let offset = file.metadata().unwrap().len();
                file.write_all(comp.as_slice())?;
                file.flush()?;
                index_tree.insert(stream_id, (offset as u32, size as u16));
                write_index(&idx_path, index_tree)?;
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    Ok(())
}

fn ensure_xtream_storage_path(cfg: &Config, target_name: &str) -> Result<PathBuf, M3uFilterError> {
    if let Some(path) = get_xtream_storage_path(cfg, target_name) {
        if fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save xtream data, can't create directory {}", &path.to_str().unwrap());
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg));
        }
        Ok(path)
    } else {
        let msg = format!("Failed to save xtream data, can't create directory for target {target_name}");
        Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg))
    }
}

fn write_documents_to_file<T>(file: &Path, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize {
    match File::create(file) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, value)?;
            match writer.flush() {
                Ok(_) => Ok(()),
                Err(e) => Err(e)
            }
        }
        Err(e) => Err(e)
    }
}

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write xtream playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

fn write_playlist_to_file(storage_path: &Path, stream_id: &mut u32, cluster: &XtreamCluster, playlist: &mut [PlaylistItem]) -> Result<(), M3uFilterError> {
    let (xtream_path, idx_path) = get_xtream_file_paths(storage_path, cluster);
    match create_file_tuple(&xtream_path, &idx_path) {
        Ok((mut main_file, mut idx_file)) => {
            let mut idx_offset: u32 = 0;
            for pli in playlist.iter_mut() {
                pli.header.borrow_mut().stream_id = Rc::new(stream_id.to_string());
                if let Ok(encoded) = bincode::serialize(&pli) {
                    match file_utils::check_write(main_file.write_all(&encoded)) {
                        Ok(_) => {
                            let bytes_written = encoded.len() as u16;
                            let combined_bytes = IndexRecord::new(idx_offset, bytes_written).to_bytes();
                            if let Err(err) = file_utils::check_write(idx_file.write_all(&combined_bytes)) {
                                return cant_write_result!(&idx_path, err);
                            }
                            idx_offset += bytes_written as u32;
                            *stream_id += 1;
                        }
                        Err(err) => {
                            return cant_write_result!(&xtream_path, err);
                        }
                    }
                }
            }
        }
        Err(err) => return cant_write_result!(&xtream_path, err),
    }
    Ok(())
}

fn save_stream_id_cluster_mapping(storage_path: &Path, id_data: &mut Vec<(XtreamCluster, u32)>) -> Result<(), Error> {
    let stream_id_path = get_xtream_id_index_file_path(storage_path);
    id_data.sort_by(|(_, a), (_, b)| b.cmp(a));
    let encoded: Vec<u8> = bincode::serialize(id_data).unwrap();
    fs::write(stream_id_path, encoded)
}

fn load_stream_id_cluster_mapping(storage_path: &Path) -> Option<Vec<(XtreamCluster, u32)>> {
    let path = get_xtream_id_index_file_path(storage_path);
    match fs::read(path) {
        Ok(encoded) => {
            let decoded: Vec<(XtreamCluster, u32)> = bincode::deserialize(&encoded[..]).unwrap();
            Some(decoded)
        }
        Err(_) => None,
    }
}

fn write_playlists_to_file(storage_path: &Path, collections: Vec<(XtreamCluster, &mut [PlaylistItem])>) -> Result<(), M3uFilterError> {
    let mut id_list: Vec<(XtreamCluster, u32)> = vec![];
    let mut stream_id: u32 = 1;
    for (cluster, playlist) in collections {
        id_list.push((cluster.clone(), stream_id));
        write_playlist_to_file(storage_path, &mut stream_id, &cluster, playlist)?;
    }
    match save_stream_id_cluster_mapping(storage_path, &mut id_list) {
        Ok(_) => Ok(()),
        Err(err) => Err(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("failed to write xtream playlist: {} - {}", storage_path.to_str().unwrap() , err)))
    }
}

pub(crate) fn write_xtream_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut [PlaylistGroup]) -> Result<(), M3uFilterError> {
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
                        let col = if header.item_type != PlaylistItemType::Series {
                            if let Ok(_) = header.id.parse::<i32>() {
                                header.category_id = *cat_id;
                                Some(match pli.header.borrow().xtream_cluster {
                                    XtreamCluster::Live => &mut live_col,
                                    XtreamCluster::Series => &mut series_col,
                                    XtreamCluster::Video => &mut vod_col,
                                })
                            } else {
                                errors.push(format!("Channel does not have an id: {}", pli.header.borrow().title.as_str()));
                                None
                            }
                        } else { None };
                        drop(header);
                        if let Some(pl) =  col {
                            pl.push(pli);
                        }
                    }
                }
            }

            for (col_path, data) in [
                (get_collection_path(&path, COL_CAT_LIVE), &cat_live_col),
                (get_collection_path(&path, COL_CAT_VOD), &cat_vod_col),
                (get_collection_path(&path, COL_CAT_SERIES), &cat_series_col)] {
                match write_documents_to_file(&col_path, data) {
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
                    errors.push(format!("Persisting collection failed:{}", err));
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
                for entry in iter_json_array::<Value, BufReader<File>>(reader).flatten() {
                    if let Some(item) = entry.as_object() {
                        if let Some(category_id_value) = item.get("category_id") {
                            if let Some(category_id) = category_id_value.as_str() {
                                if let Some(category_name_value) = item.get("category_name") {
                                    if let Some(category_name) = category_name_value.as_str() {
                                        if let Ok(cat_id) = category_id.to_string().parse::<u32>() {
                                            result.insert(category_name.to_string(), cat_id);
                                            max_id = max_id.max(cat_id);
                                        }
                                    }
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

pub(crate) fn xtream_get_collection_path(cfg: &Config, target_name: &str, collection_name: &str) -> Result<(Option<PathBuf>, Option<String>), Error> {
    if let Some(path) = get_xtream_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok((Some(col_path), None));
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Cant find collection: {}/{}", target_name, collection_name)))
}

fn load_index(path: &Path) -> Option<IndexTree> {
    match fs::read(path) {
        Ok(encoded) => {
            let decoded: IndexTree = bincode::deserialize(&encoded[..]).unwrap();
            Some(decoded)
        }
        Err(_) => None,
    }
}

fn write_index(path: &PathBuf, index: &IndexTree) -> io::Result<()> {
    let encoded = bincode::serialize(index).unwrap();
    fs::write(path, encoded)
}

fn seek_read(
    reader: &mut (impl Read + Seek),
    offset: u64,
    amount_to_read: u16,
) -> Result<Vec<u8>, Error> {
    // A buffer filled with as many zeros as we'll read with read_exact
    let mut buf = vec![0u8; amount_to_read as usize];
    reader.seek(SeekFrom::Start(offset))?;
    reader.read_exact(&mut buf)?;
    Ok(buf)
}


pub(crate) async fn xtream_get_stored_stream_info(
    app_state: &AppState, target_name: &str, stream_id: i32,
    cluster: &XtreamCluster, target_input: &ConfigInput) -> Result<String, ()> {
    let cache_info = target_input.options.as_ref()
        .map(|o| o.xtream_info_cache).unwrap_or(false);
    if cache_info {
        if let Some(path) = get_xtream_storage_path(&app_state.config, target_name) {
            let (col_path, idx_path) = get_info_collection_and_idx_path(&path, cluster);
            let lock = app_state.shared_locks.get_lock(target_name);
            let shared_lock = lock.read().unwrap();
            if idx_path.exists() && col_path.exists() {
                let index_tree = load_index(&idx_path);
                if let Some(idx_map) = &index_tree {
                    if let Some((offset, size)) = idx_map.get(&stream_id) {
                        let mut reader = BufReader::new(File::open(&col_path).unwrap());
                        if let Ok(bytes) = seek_read(&mut reader, *offset as u64, *size) {
                            let mut decompressed: Vec<u8> = Vec::new();
                            let _ = lzma_rs::lzma_decompress(&mut bytes.as_slice(), &mut decompressed);
                            drop(shared_lock);
                            return Ok(String::from_utf8(decompressed).unwrap());
                        }
                    }
                }
            }
            drop(shared_lock);
        }
    }
    Err(())
}

pub(crate) async fn xtream_persist_stream_info(
    app_state: &AppState, target_name: &str, stream_id: i32,
    cluster: &XtreamCluster, target_input: &ConfigInput, content: &str) {
    let cache_info = target_input.options.as_ref()
        .map(|o| o.xtream_info_cache).unwrap_or(false);
    if cache_info {
        if let Some(path) = get_xtream_storage_path(&app_state.config, target_name) {
            let lock = app_state.shared_locks.get_lock(target_name);
            let shared_lock = lock.write().unwrap();
            let mut index_tree = {
                let (col_path, idx_path) = get_info_collection_and_idx_path(&path, cluster);
                if idx_path.exists() && col_path.exists() {
                    load_index(&idx_path).unwrap_or_default()
                } else {
                    IndexTree::new()
                }
            };
            match write_xtream_info(app_state, target_name, stream_id, cluster, content,
                                    &mut index_tree) {
                Ok(_) => {}
                Err(err) => { error!("{}", err.to_string()); }
            }
            drop(shared_lock);
        }
    }
}

pub(crate) fn get_xtream_item_for_stream_id(stream_id: u32, config: &Config, target: &ConfigTarget) -> Result<PlaylistItem, Error> {
    if let Some(storage_path) = get_xtream_storage_path(config, target.name.as_str()) {
        if let Some(mapping) = load_stream_id_cluster_mapping(&storage_path) {
            if let Some((cluster, cluster_index)) = mapping.iter().find(|(_, c)| stream_id > *c) {
                let (xtream_path, idx_path) = get_xtream_file_paths(&storage_path, cluster);
                if xtream_path.exists() && idx_path.exists() {
                    let offset: u64 = IndexRecord::get_index_offset(stream_id - cluster_index) as u64;
                    let mut idx_file = File::open(idx_path)?;
                    let mut xtream_file = File::open(xtream_path)?;
                    let index_record = IndexRecord::from_file(&mut idx_file, offset)?;
                    xtream_file.seek(SeekFrom::Start(index_record.index as u64))?;
                    let mut buffer: Vec<u8> = vec![0; index_record.size as usize];
                    xtream_file.read_exact(&mut buffer)?;
                    if let Ok(pli) = bincode::deserialize::<PlaylistItem>(&buffer[..]) {
                        return Ok(pli);
                    }
                }
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to read xtream item for stream-id {}", stream_id)))
}

pub(crate) fn load_rewrite_xtream_playlist(cluster: &XtreamCluster, config: &Config, target: &ConfigTarget) -> Result<String, Error> {
    if let Some(storage_path) = get_xtream_storage_path(config, target.name.as_str()) {
        let (xtream_path, idx_path) = get_xtream_file_paths(&storage_path, cluster);
        if xtream_path.exists() && idx_path.exists() {
            match std::fs::read(&xtream_path) {
                Ok(encoded_xtream) => {
                    match std::fs::read(&idx_path) {
                       Ok(encoded_idx) => {
                           let mut cursor = 0;
                           let size = encoded_idx.len();
                           let options = XtreamMappingOptions::from_target_options(target.options.as_ref());
                           let mut result = vec![];
                           while cursor < size {
                               let tuple = IndexRecord::from_bytes(&encoded_idx, &mut cursor);
                               let start_offset = tuple.index as usize;
                               let end_offset = start_offset + tuple.size as usize;
                               if let Ok(pli) = bincode::deserialize::<PlaylistItem>(&encoded_xtream[start_offset..end_offset]) {
                                    result.push(pli.to_xtream(&options));
                               } else {
                                   error!("Could not deserialize item {}", &xtream_path.to_str().unwrap());
                               }
                           }
                           return Ok(serde_json::to_string(&result).unwrap());
                       }
                        Err(err) => error!("Could not open file {}: {}", &idx_path.to_str().unwrap(), err),
                    }
                }
                Err(err) => error!("Could not open file {}: {}", &xtream_path.to_str().unwrap(), err),
            }
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to find xtream storage for target {}", &target.name)))
}

