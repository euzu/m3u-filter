use std::cell::Ref;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Error, Write};
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

const LIVE_STREAM_FIELDS: &[&str] = &[
    "epg_channel_id"
];

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

// fn value_to_bson(value: &Value) -> Bson {
//     match value {
//         Value::Null => Bson::Null,
//         Value::Bool(value) => Bson::Boolean(value.clone()),
//         Value::Number(value) => {
//             if value.is_f64() {
//                 Bson::Double(value.as_f64().unwrap())
//             } else {
//                 Bson::Int64(value.as_i64().unwrap())
//             }
//         }
//         Value::String(value) => Bson::String(value.clone()),
//         Value::Array(value) => Bson::Array(value.iter().map(value_to_bson).collect()),
//         Value::Object(value) => {
//             let mut document = Document::new();
//             for (key, val) in value {
//                 document.insert(key, value_to_bson(val));
//             }
//             Bson::Document(document)
//         }
//     }
// }

fn write_to_file<T>(file: &PathBuf, value: &T) -> Result<(), Error>
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

fn get_storage_path(cfg: &Config, target_name: &str) -> Option<PathBuf> {
    utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(' ', "_"))))
}

fn get_collection_path(path: &Path, collection: &str) -> PathBuf {
    path.join(format!("{}.json", collection))
}

pub(crate) fn xtream_save_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut [PlaylistGroup]) -> Result<(), M3uFilterError> {
    if let Some(path) = get_storage_path(cfg, &target.name) {
        if fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save, can't create directory {}", &path.to_str().unwrap());
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, msg));
        }

        let mut cat_live_col = vec![];
        let mut cat_series_col = vec![];
        let mut cat_vod_col = vec![];
        let mut live_col = vec![];
        let mut series_col = vec![];
        let mut vod_col = vec![];

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
                        ("name".to_string(), Value::String(header.name.clone())),
                        ("num".to_string(), Value::Number(serde_json::Number::from(channel_num))),
                        ("title".to_string(), Value::String(header.title.clone())),
                        ("stream_icon".to_string(), Value::String(header.logo.clone())),
                    ]);

                    let stream_id = Value::Number(serde_json::Number::from(header.id.parse::<i32>().unwrap()));
                    match header.xtream_cluster {
                        XtreamCluster::Live => {
                            document.insert("stream_id".to_string(), stream_id);
                            document.insert("direct_source".to_string(), Value::String(header.source.clone()));
                            document.insert("thumbnail".to_string(), Value::String(header.logo_small.clone()));
                            document.insert("custom_sid".to_string(), Value::String("".to_string()));
                        }
                        XtreamCluster::Video => {
                            document.insert("stream_id".to_string(), stream_id);
                            document.insert("direct_source".to_string(), Value::String(header.source.clone()));
                            document.insert("custom_sid".to_string(), Value::String("".to_string()));
                        }
                        XtreamCluster::Series => {
                            document.insert("series_id".to_string(), stream_id);
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
                        XtreamCluster::Live => &mut live_col,
                        XtreamCluster::Series => &mut series_col,
                        XtreamCluster::Video => &mut vod_col,
                    }.push(Value::Object(document));
                }
            }
        }

        let mut errors = Vec::new();
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

pub(crate) fn xtream_get_all(cfg: &Config, target_name: &str, collection_name: &str) -> Result<PathBuf, Error> {
    if let Some(path) = get_storage_path(cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok(col_path);
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find collection: {}/{}", target_name, collection_name)))
}
