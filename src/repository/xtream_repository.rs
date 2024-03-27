use std::cell::Ref;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Error, Read, Seek, SeekFrom, Write};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use log::{error};
use serde::Serialize;
use serde_json::{json, Map, Value};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::{create_m3u_filter_error_result};
use crate::api::api_model::AppState;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::utils::file_utils;

type IndexTree = BTreeMap<i32, (u32, u16)>;


pub(crate) static COL_CAT_LIVE: &str = "cat_live";
pub(crate) static COL_CAT_SERIES: &str = "cat_series";
pub(crate) static COL_CAT_VOD: &str = "cat_vod";
pub(crate) static COL_LIVE: &str = "live";
pub(crate) static COL_SERIES: &str = "series";
pub(crate) static COL_VOD: &str = "vod";

const LIVE_STREAM_FIELDS: &[&str] = &[];

const VIDEO_STREAM_FIELDS: &[&str] = &[
    "release_date", "cast",
    "director", "episode_run_time", "genre",
    "stream_type", "title", "year", "youtube_trailer",
    "plot", "rating_5based", "stream_icon", "container_extension"
];

const SERIES_STREAM_FIELDS: &[&str] = &[
    "backdrop_path", "cast", "cover", "director", "episode_run_time", "genre",
    "last_modified", "name", "plot", "rating_5based",
    "stream_type", "title", "year", "youtube_trailer",
];


pub(crate) fn get_xtream_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}

pub(crate) fn get_xtream_epg_file_path(path: &Path) -> PathBuf {
    path.join("epg.xml")
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

fn write_to_file<T>(file: &Path, value: &T) -> Result<(), Error>
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
            .write(true)
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

pub(crate) fn write_xtream_playlist(target: &ConfigTarget, cfg: &Config, playlist: &[PlaylistGroup]) -> Result<(), M3uFilterError> {
    if let Some(path) = get_xtream_storage_path(cfg, &target.name) {
        if fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save, can't create directory {}", &path.to_str().unwrap());
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg));
        }

        let (skip_live_direct_source, skip_video_direct_source) = target.options.as_ref()
            .map_or((false, false), |o| (o.xtream_skip_live_direct_source, o.xtream_skip_video_direct_source));

        let mut cat_live_col = vec![];
        let mut cat_series_col = vec![];
        let mut cat_vod_col = vec![];
        let mut live_col = vec![];
        let mut series_col = vec![];
        let mut vod_col = vec![];

        let mut vod_map = HashMap::<i32, String>::new();
        let mut series_map = HashMap::<i32, String>::new();

        let mut channel_num: i32 = 0;
        let mut errors = Vec::new();
        for plg in playlist {
            if !&plg.channels.is_empty() {
                match &plg.xtream_cluster {
                    XtreamCluster::Live => &mut cat_live_col,
                    XtreamCluster::Series => &mut cat_series_col,
                    XtreamCluster::Video => &mut cat_vod_col,
                }.push(
                    json!({
                    "category_id": format!("{}", &plg.id),
                    "category_name": plg.title.clone(),
                    "parent_id": 0
                }));

                for pli in &plg.channels {
                    let header = &pli.header.borrow();
                    if let Ok(stream_id) = header.id.parse::<i32>() {
                        if header.item_type == PlaylistItemType::Series {
                            // we skip resolved series, because this is only necessary when writing m3u files
                            continue;
                        }
                        channel_num += 1;
                        let mut document = serde_json::Map::from_iter([
                            ("category_id".to_string(), Value::String(format!("{}", &plg.id))),
                            ("category_ids".to_string(), Value::Array(Vec::from([Value::Number(serde_json::Number::from(plg.id.to_owned()))]))),
                            ("name".to_string(), Value::String(header.name.as_ref().clone())),
                            ("num".to_string(), Value::Number(serde_json::Number::from(channel_num))),
                            ("title".to_string(), Value::String(header.title.as_ref().clone())),
                            ("stream_icon".to_string(), Value::String(header.logo.as_ref().clone())),
                        ]);

                        let stream_id_value = Value::Number(serde_json::Number::from(stream_id));
                        match header.xtream_cluster {
                            XtreamCluster::Live => {
                                document.insert("stream_id".to_string(), stream_id_value);
                                if skip_live_direct_source {
                                    document.insert("direct_source".to_string(), Value::String("".to_string()));
                                } else {
                                    document.insert("direct_source".to_string(), Value::String(header.url.as_ref().clone()));
                                }
                                document.insert("thumbnail".to_string(), Value::String(header.logo_small.as_ref().clone()));
                                document.insert("custom_sid".to_string(), Value::String("".to_string()));
                                document.insert("epg_channel_id".to_string(), match &header.epg_channel_id {
                                    None => Value::Null,
                                    Some(epg_id) => Value::String(epg_id.as_ref().clone())
                                });
                            }
                            XtreamCluster::Video => {
                                document.insert("stream_id".to_string(), stream_id_value);
                                if skip_video_direct_source {
                                    document.insert("direct_source".to_string(), Value::String("".to_string()));
                                } else {
                                    document.insert("direct_source".to_string(), Value::String(header.url.as_ref().clone()));
                                }
                                document.insert("custom_sid".to_string(), Value::String("".to_string()));
                            }
                            XtreamCluster::Series => {
                                document.insert("series_id".to_string(), stream_id_value);
                            }
                        };

                        if let Some(add_props) = &header.additional_properties {
                            for (field_name, field_value) in add_props {
                                document.insert(field_name.to_string(), field_value.to_owned());
                            }
                        }

                        match header.xtream_cluster {
                            XtreamCluster::Live => {
                                append_mandatory_fields(&mut document, LIVE_STREAM_FIELDS);
                            }
                            XtreamCluster::Video => {
                                append_mandatory_fields(&mut document, VIDEO_STREAM_FIELDS);
                            }
                            XtreamCluster::Series => {
                                append_prepared_series_properties(header, &mut document);
                                append_mandatory_fields(&mut document, SERIES_STREAM_FIELDS);
                                append_release_date(&mut document);
                            }
                        };

                        match header.xtream_cluster {
                            XtreamCluster::Live => {}
                            XtreamCluster::Series => {
                                series_map.insert(stream_id, serde_json::to_string(&document).unwrap());
                            }
                            XtreamCluster::Video => {
                                vod_map.insert(stream_id, serde_json::to_string(&document).unwrap());
                            }
                        }

                        match header.xtream_cluster {
                            XtreamCluster::Live => &mut live_col,
                            XtreamCluster::Series => &mut series_col,
                            XtreamCluster::Video => &mut vod_col,
                        }.push(Value::Object(document));
                    } else {
                        errors.push(format!("Channel does not have an id: {}", &header.title));
                    }
                }
            }
        }

        for (col_path, data) in [
            (get_collection_path(&path, COL_CAT_LIVE), &cat_live_col),
            (get_collection_path(&path, COL_CAT_VOD), &cat_vod_col),
            (get_collection_path(&path, COL_CAT_SERIES), &cat_series_col),
            (get_collection_path(&path, COL_LIVE), &live_col),
            (get_collection_path(&path, COL_VOD), &vod_col),
            (get_collection_path(&path, COL_SERIES), &series_col)] {
            match write_to_file(&col_path, data) {
                Ok(()) => {}
                Err(err) => {
                    errors.push(format!("Persisting collection failed: {}: {}", &col_path.to_str().unwrap(), err));
                }
            }
        }
        if !errors.is_empty() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "{}", errors.join("\n"));
        }
    } else {
        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Persisting playlist failed: {}.db", &target.name);
    }

    Ok(())
}

fn append_prepared_series_properties(header: &Ref<PlaylistItemHeader>, document: &mut Map<String, Value>) {
    if let Some(add_props) = &header.additional_properties {
        match add_props.iter().find(|(key, _)| key.eq("rating")) {
            Some((_, value)) => {
                document.insert("rating".to_string(), match value {
                    Value::Number(val) => Value::String(format!("{:.0}", val.as_f64().unwrap())),
                    Value::String(val) => Value::String(val.to_string()),
                    _ => Value::String("0".to_string()),
                });
            }
            None => {
                document.insert("rating".to_string(), Value::String("0".to_string()));
            }
        }
    }
}

fn append_release_date(document: &mut Map<String, Value>) {
    // Do we really need releaseDate ?
    let has_release_date_1 = document.contains_key("release_date");
    let has_release_date_2 = document.contains_key("releaseDate");
    if !(has_release_date_1 && has_release_date_2) {
        let release_date = if has_release_date_1 {
            document.get("release_date")
        } else if has_release_date_2 {
            document.get("releaseDate")
        } else {
            None
        }.map_or_else(|| Value::Null, |v| v.clone());
        if !&has_release_date_1 {
            document.insert("release_date".to_string(), release_date.clone());
        }
        if !&has_release_date_2 {
            document.insert("releaseDate".to_string(), release_date.clone());
        }
    }
}

fn append_mandatory_fields(document: &mut Map<String, Value>, fields: &[&str]) {
    for &field in fields {
        if !document.contains_key(field) {
            document.insert(field.to_string(), Value::Null);
        }
    }
}

pub(crate) fn xtream_get_collection_path(cfg: &Config, target_name: &str, collection_name: &str) -> Result<(Option<PathBuf>, Option<String>), Error> {
    if let Some(path) = get_xtream_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok((Some(col_path), None));
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find collection: {}/{}", target_name, collection_name)))
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

fn write_index(path: &PathBuf, index: &IndexTree) -> std::io::Result<()> {
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
                            let mut decomp: Vec<u8> = Vec::new();
                            let _ = lzma_rs::lzma_decompress(&mut bytes.as_slice(), &mut decomp);
                            drop(shared_lock);
                            return Ok(String::from_utf8(decomp).unwrap());
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