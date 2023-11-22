use std::cell::Ref;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Error, Read, Seek, SeekFrom, Write};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use serde::Serialize;
use serde_json::{json, Map, Value};
use crate::model::config::{Config, ConfigTarget};
use crate::model::model_m3u::{PlaylistGroup, PlaylistItemHeader, XtreamCluster};
use crate::{create_m3u_filter_error_result, utils};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};

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

fn get_collection_and_idx_path(path: &Path, cluster: &XtreamCluster) -> (PathBuf, PathBuf) {
    let collection = match cluster {
        XtreamCluster::Live => COL_LIVE,
        XtreamCluster::Video => COL_VOD,
        XtreamCluster::Series => COL_SERIES,
    };
    (get_collection_path(path, collection), get_idx_path(path, collection))
}

fn write_to_file_width_idx(path: &Path, values: &[(i32, Value)], cluster: &XtreamCluster) -> Result<(), Error> {
    let (file, file_idx) = get_collection_and_idx_path(path, cluster);
    match File::create(file) {
        Ok(file) => {
            let mut index = BTreeMap::<i32, (u32, u16)>::new();
            let mut writer = BufWriter::new(file);
            writer.write_all("[".as_bytes())?;
            let mut offset = 1;
            let value_cnt = values.len();
            let mut value_idx = 0;
            for (stream_id, data) in values {
                let content = serde_json::to_string(data).unwrap();
                let bytes = content.as_bytes();
                let size = bytes.len();
                index.insert(*stream_id, (offset as u32, size as u16));
                offset += size;
                let _ = writer.write_all(bytes);
                value_idx += 1;
                if value_idx < value_cnt {
                    writer.write_all(",".as_bytes())?;
                    offset += 1;
                }
            }
            writer.write_all("]".as_bytes())?;
            match writer.flush() {
                Ok(_) => {
                    let encoded: Vec<u8> = bincode::serialize(&index).unwrap();
                    let _ = fs::write(file_idx, encoded);
                    Ok(())
                }
                Err(e) => Err(e)
            }
        }
        Err(e) => Err(e)
    }
}


pub(crate) fn get_xtream_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}

fn get_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}.json", collection))
}

fn get_idx_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}.idx", collection))
}


pub(crate) fn get_xtream_epg_file_path(path: &Path) -> PathBuf {
    path.join("epg.xml")
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
                    channel_num += 1;
                    let header = &pli.header.borrow();
                    let mut document = serde_json::Map::from_iter([
                        ("category_id".to_string(), Value::String(format!("{}", &plg.id))),
                        ("category_ids".to_string(), Value::Array(Vec::from([Value::Number(serde_json::Number::from(plg.id.to_owned()))]))),
                        ("name".to_string(), Value::String(header.name.as_ref().clone())),
                        ("num".to_string(), Value::Number(serde_json::Number::from(channel_num))),
                        ("title".to_string(), Value::String(header.title.as_ref().clone())),
                        ("stream_icon".to_string(), Value::String(header.logo.as_ref().clone())),
                    ]);

                    let stream_id = header.id.parse::<i32>().unwrap();
                    let stream_id_value = Value::Number(serde_json::Number::from(stream_id));
                    match header.xtream_cluster {
                        XtreamCluster::Live => {
                            document.insert("stream_id".to_string(), stream_id_value);
                            if !skip_live_direct_source {
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
                            if !skip_video_direct_source {
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
                    }.push((stream_id, Value::Object(document)));
                }
            }
        }

        let mut errors = Vec::new();
        for (col_path, data) in [
            (get_collection_path(&path, COL_CAT_LIVE), &cat_live_col),
            (get_collection_path(&path, COL_CAT_VOD), &cat_vod_col),
            (get_collection_path(&path, COL_CAT_SERIES), &cat_series_col)] {
            match write_to_file(&col_path, data) {
                Ok(()) => {}
                Err(err) => {
                    errors.push(format!("Persisting collection failed: {}: {}", &col_path.to_str().unwrap(), err));
                }
            }
        }
        for (data, cluster) in [
            (&live_col, XtreamCluster::Live),
            (&vod_col, XtreamCluster::Video),
            (&series_col, XtreamCluster::Series)] {
            match write_to_file_width_idx(&path, data, &cluster) {
                Ok(()) => {}
                Err(err) => {
                    errors.push(format!("Persisting collection failed: {}: {}", cluster, err));
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

pub(crate) fn xtream_get_all(cfg: &Config, target_name: &str, collection_name: &str) -> Result<(Option<PathBuf>, Option<String>), Error> {
    if let Some(path) = get_xtream_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok((Some(col_path), None));
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find collection: {}/{}", target_name, collection_name)))
}

fn load_map(path: &Path) -> Option<BTreeMap<i32, (u32, u16)>> {
    match std::fs::read(path) {
        Ok(encoded) => {
            let decoded: BTreeMap<i32, (u32, u16)> = bincode::deserialize(&encoded[..]).unwrap();
            Some(decoded)
        }
        Err(_) => None,
    }
}

fn seek_read(
    reader: &mut (impl Read + Seek),
    offset: u32,
    amount_to_read: u16,
) -> Result<Vec<u8>, Error> {
    // A buffer filled with as many zeros as we'll read with read_exact
    let mut buf = vec![0; amount_to_read as usize];
    reader.seek(SeekFrom::Start(offset as u64))?;
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn xtream_get_stream_info(cfg: &Config, target_name: &str, stream_id: i32, cluster: XtreamCluster) -> Result<String, Error> {
    if let Some(path) = get_xtream_storage_path(cfg, target_name) {
        let (col_path, idx_path) = get_collection_and_idx_path(&path, &cluster);
        if idx_path.exists() && col_path.exists() {
            if let Some(idx_map) = load_map(&idx_path) {
                if let Some((offset, size)) = idx_map.get(&stream_id) {
                    let mut reader = BufReader::new(File::open(&col_path).unwrap());
                    if let Ok(bytes) = seek_read(&mut reader, *offset, *size) {
                        return Ok(String::from_utf8(bytes).unwrap());
                    }
                }
            }
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find stream with id: {}/{}/{}", target_name, &cluster, stream_id)))
}

pub(crate) fn xtream_get_series_info(cfg: &Config, target_name: &str, stream_id: i32) -> Result<String, Error> {
    /*
    {
        "episodes": {
        "": [
        {
            "added": string,
            "container_extension": string,
            "custom_sid": string,
            "direct_source": string,
            "episode_num": int,
            "id": string,
            "info": {
                "bitrate": int,
                "duration": string,
                "duration_secs": int,
                "movie_image": string,
                "name": string,
                "plot": string,
                "rating": float,
                "releasedate": string,
                "audio": FFMPEGStreamInfo,
                "video": FFMPEGStreamInfo
            }
            "season": int,
            "title": string
         }
        ]
    },
        "info": {
            "backdrop_path: [string],
            "cast":  string,
            "category_id": string,
            "cover":  string,
            "director":  string,
            "episode_run_time":  string,
            "genre":  string,
            "last_modified": string,
            "name":  string,
            "num":  int,
            "plot":  string,
            "rating, string,
            "rating_5based": float,
            "releaseDate": string,
            "series_id": int,
            "stream_type": string,
            "youtube_trailer": string,
    }
    }
    "seasons": []
}
        */
    // TODO restructure
    xtream_get_stream_info(cfg, target_name, stream_id, XtreamCluster::Series)
}

pub(crate) fn xtream_get_vod_info(cfg: &Config, target_name: &str, stream_id: i32) -> Result<String, Error> {
    /*
    {
    "info": {
        "backdrop_path": [string],
        "bitrate": FlexInt,
        "cast": string,
        "director": string,
        "duration": string,
        "duration_secs": FlexInt,
        "genre": string,
        "movie_image": string,
        "plot": string,
        "rating": FlexFloat,
        "releasedate": string,
        "tmdb_id": int,
        "youtube_trailer": string,
        "audio": FFMPEGStreamInfo,
        "video": FFMPEGStreamInfo,
	} `json:"info"`
	"movie_data": {
		"added": string,
		"category_id": string,
		"container_extension": string,
		"custom_sid": string,
		"direct_source": string,
		"name": string,
		"stream_id": int
	}
	}
     */
  // TODO restructure
    xtream_get_stream_info(cfg, target_name, stream_id, XtreamCluster::Video)
}
