use std::fs;
use std::fs::File;
use std::io::{BufWriter, Error, Write};
use std::iter::FromIterator;
use std::path::PathBuf;
use serde::Serialize;
use serde_json::{json, Value};
use crate::model::config::{Config, ConfigTarget};
use crate::messaging::send_message;
use crate::model::model_m3u::{PlaylistGroup, XtreamCluster};
use crate::utils;
use log::{error};

pub(crate) static COL_CAT_LIVE: &str = "cat_live";
pub(crate) static COL_CAT_SERIES: &str = "cat_series";
pub(crate) static COL_CAT_VOD: &str = "cat_vod";
pub(crate) static COL_LIVE: &str = "live";
pub(crate) static COL_SERIES: &str = "series";
pub(crate) static COL_VOD: &str = "vod";

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
    utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(target_name.replace(" ", "_"))))
}

fn get_collection_path(path: &PathBuf, collection: &str) -> PathBuf {
    path.join(format!("{}.json", collection))
}

pub(crate) fn xtream_save_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    let mut failed = false;
    if let Some(path) = get_storage_path(&cfg, &target.name) {
        if fs::create_dir_all(&path).is_err() {
            let msg = format!("Failed to save, can't create directory {}", &path.into_os_string().into_string().unwrap());
            send_message(msg.as_str());
            return Err(std::io::Error::new(std::io::ErrorKind::Other, msg));
        }

        let mut cat_live_col = vec![];
        let mut cat_series_col = vec![];
        let mut cat_vod_col = vec![];
        let mut live_col = vec![];
        let mut series_col = vec![];
        let mut vod_col = vec![];

        let mut channel_num: i32 = 0;
        for plg in playlist {
            match &plg.xtream_cluster {
                XtreamCluster::LIVE => &mut cat_live_col,
                XtreamCluster::SERIES => &mut cat_series_col,
                XtreamCluster::VIDEO => &mut cat_vod_col,
            }.push(
                json!({
                    "category_id": plg.id,
                    "category_name": plg.title.clone(),
                    "parent_id": 0
                })
            );

            for pli in &plg.channels {
                channel_num += 1;
                let header = &pli.header.borrow();
                let mut document = serde_json::Map::from_iter([
                    ("category_id".to_string(), Value::String(format!("{}", &plg.id))),
                    ("category_ids".to_string(), Value::Array(Vec::from([Value::Number(serde_json::Number::from((&plg.id).to_owned()))]))),
                    ("direct_source".to_string(), Value::String(header.source.clone())),
                    ("name".to_string(), Value::String(header.name.clone())),
                    ("num".to_string(), Value::Number(serde_json::Number::from(channel_num.clone()))),
                    ("title".to_string(), Value::String(header.title.clone())),
                    ("stream_icon".to_string(), Value::String(header.logo.clone())),
                    ("thumbnail".to_string(), Value::String(header.logo_small.clone())),
                ]);

                match header.xtream_cluster {
                    XtreamCluster::SERIES =>
                        document.insert("series_id".to_string(), Value::String(header.id.clone())),
                    _ => document.insert("stream_id".to_string(), Value::Number(serde_json::Number::from(header.id.parse::<i32>().unwrap()))),
                };

                if let XtreamCluster::LIVE = header.xtream_cluster {
                    document.insert("custom_sid".to_string(), Value::String("".to_string())); // is this necessary ?
                };


                if let Some(add_props) = &header.additional_properties {
                    for (field_name, field_value) in add_props {
                        document.insert(field_name.to_string(), field_value.to_owned());
                    }
                }

                match header.xtream_cluster {
                    XtreamCluster::LIVE => &mut live_col,
                    XtreamCluster::SERIES => &mut series_col,
                    XtreamCluster::VIDEO => &mut vod_col,
                }.push(Value::Object(document));
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
                    failed = true;
                    error!("Persisting collection failed: {}: {}", &col_path.clone().into_os_string().into_string().unwrap(), err);
                }
            }
        }
    } else {
        return Err(Error::new(std::io::ErrorKind::Other, format!("Persisting playlist failed: {}.db", &target.name)));
    }

    if failed {
        send_message(format!("Something went wrong persisting target {}", &target.name).as_str());
    }
    Ok(())
}

pub(crate) fn xtream_get_all(cfg: &Config, target_name: &str, collection_name: &str) -> Result<PathBuf, Error> {
    if let Some(path) = get_storage_path(&cfg, target_name) {
        let col_path = get_collection_path(&path, collection_name);
        if col_path.exists() {
            return Ok(col_path);
        }
    }
    Err(Error::new(std::io::ErrorKind::Other, format!("Cant find collection: {}/{}", target_name, collection_name)))
}
