use polodb_core::bson::{Bson, doc};
use polodb_core::Database;
use serde_json::Value;
use crate::config::{Config, ConfigTarget};
use crate::messaging::send_message;
use crate::model_m3u::{PlaylistGroup, XtreamCluster};
use crate::utils;

pub(crate) fn save_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    //let mut new_playlist = playlist.to_owned();
    let mut failed = false;
    if let Some(path) = utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(format!("{}.db", &target.name)))) {
        let db = Database::open_file(path).unwrap();
        let cat_live_col = db.collection("cat_live");
        let cat_series_col = db.collection("cat_series");
        let cat_vod_col = db.collection("cat_vod");
        let live_col = db.collection("live");
        let series_col = db.collection("series");
        let vod_col = db.collection("vod");
        for plg in playlist {
            let cat_collection = match &plg.xtream_cluster {
                XtreamCluster::LIVE => &cat_live_col,
                XtreamCluster::SERIES => &cat_series_col,
                XtreamCluster::VIDEO => &cat_vod_col,
            };
            match cat_collection.insert_one(
                doc! {
                    "category_id": plg.id,
                    "category_name": plg.title.clone(),
                    "parent_id": 0
                }
            ) {
                Ok(_) => {}
                Err(e) => {
                    println!("failed to write to collection {:?}", e);
                    failed = true;
                }
            };

            for pli in &plg.channels {
                let header = &pli.header.borrow();
                let collection = match header.xtream_cluster {
                    XtreamCluster::LIVE => &live_col,
                    XtreamCluster::SERIES => &series_col,
                    XtreamCluster::VIDEO => &vod_col,
                };

                let mut document = doc! {
                    "category_id": format!("{}", plg.id),
                    "category_ids": [plg.id],
                    "custom_sid": "",
                    "direct_source": header.source.clone(),
                    "name": header.name.clone(),
                    "title": header.title.clone(),
                    //"num": channel_num,
                    "stream_icon": header.logo.clone(),
                    "stream_id": header.id.clone(),
                    "thumbnail": header.logo_small.clone(),
                };

                if let Some(add_props) = &header.additional_properties {
                    for (field_name, field_value) in add_props {
                        document.insert(field_name, match field_value {
                            Value::Null => Bson::Null,
                            Value::Bool(value) => Bson::Boolean(value.clone()),
                            Value::Number(value) => {
                                if value.is_f64() {
                                    Bson::Double(value.as_f64().unwrap())
                                } else {
                                    Bson::Int64(value.as_i64().unwrap())
                                }
                            }
                            Value::String(value) => Bson::String(value.clone()),
                            Value::Array(_) => Bson::Null,
                            Value::Object(_) => Bson::Null,
                        });
                    }
                }

                match collection.insert_one(document) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("failed to write to collection {:?}", e);
                        failed = true;
                    }
                };
            }
        }
    } else {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Persisting playlist failed: {}.db", &target.name)));
    }

    if failed {
        send_message(format!("Something went wrong persisting target to db {}", &target.name).as_str());
    }
    Ok(())
}