use std::cell::{RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::DeserializeOwned;
use serde_json::Value;
use crate::model::model_m3u::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, XtreamCluster};
use crate::model::model_config::{default_as_empty_str};
use log::{error};

fn default_as_empty_list() -> Vec<PlaylistItem> { vec![] }

fn deserialize_number_from_string<'de, D, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
{
    // we define a local enum type inside of the function
    // because it is untagged, serde will deserialize as the first variant
    // that it can
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MaybeNumber<U> {
        // if it can be parsed as Option<T>, it will be
        Value(Option<U>),
        // otherwise try parsing as a string
        NumberString(String),
    }

    // deserialize into local enum
    let value: MaybeNumber<T> = Deserialize::deserialize(deserializer)?;
    match value {
        // if parsed as T or None, return that
        MaybeNumber::Value(value) => Ok(value),

        // (if it is any other string)
        MaybeNumber::NumberString(string) => {
            match serde_json::from_str::<T>(string.as_str()) {
                Ok(val) => Ok(Some(val)),
                Err(_) => Ok(None)
            }
        }
    }
}

fn value_to_string_array(value: &Vec<Value>) -> Option<Vec<String>> {
    Some(value.iter().map(|i| value_to_string(i)).filter(|i| i.is_some()).map(|i| i.unwrap()).collect())
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.to_string()),
        _ => None,
    }
}

fn deserialize_as_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(Some(s.clone())),
        _ => Ok(None),
    }
}

fn deserialize_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(s.clone()),
        _ => Ok(value.to_string()),
    }
}

fn deserialize_as_string_array<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(|v| match v {
        Value::Array(value) => value_to_string_array(&value),
        _ => None,
    })
}

#[derive(Deserialize)]
struct XtreamCategory {
    #[serde(deserialize_with = "deserialize_as_string")]
    pub category_id: String,
    #[serde(deserialize_with = "deserialize_as_string")]
    pub category_name: String,
    //pub parent_id: i32,
    #[serde(default = "default_as_empty_list")]
    pub channels: Vec<PlaylistItem>,
}

impl XtreamCategory {
    fn add(&mut self, item: PlaylistItem) {
        self.channels.push(item);
    }
}

#[derive(Serialize, Deserialize)]
struct XtreamStream {
    #[serde(default, deserialize_with = "deserialize_as_string")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_as_string")]
    pub category_id: String,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub stream_id: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub series_id: Option<i32>,
    #[serde(default = "default_as_empty_str", deserialize_with = "deserialize_as_string")]
    pub stream_icon: String,
    #[serde(default = "default_as_empty_str", deserialize_with = "deserialize_as_string")]
    pub direct_source: String,

    // optional attributes
    #[serde(default, deserialize_with = "deserialize_as_string_array")]
    backdrop_path: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    added: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    cast: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    container_extension: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    cover: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    director: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    episode_run_time: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    genre: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    last_modified: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    plot: Option<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    rating: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    rating_5based: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    release_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    stream_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    year: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    youtube_trailer: Option<String>,
    #[serde(default, deserialize_with = "deserialize_as_option_string")]
    epg_channel_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    tv_archive: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    tv_archive_duration: Option<i32>,
}

macro_rules! add_str_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::String(v.to_string()))));
    }
}
macro_rules! add_i64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from(i64::from(*v))))));
    }
}

macro_rules! add_f64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from_f64(f64::from(*v)).unwrap()))));
    }
}

impl XtreamStream {
    pub(crate) fn get_stream_id(&self) -> String {
        self.stream_id.map_or_else(|| self.series_id.map_or_else(|| String::from(""), |seid| format!("{}", seid)), |sid| format!("{}", sid))
    }

    pub(crate) fn get_additional_properties(&self) -> Option<Vec<(String, Value)>> {
        let mut result = vec![];
        if let Some(bdpath) = self.backdrop_path.as_ref() {
            if !bdpath.is_empty() {
                result.push((String::from("backdrop_path"), Value::Array(Vec::from([Value::String(String::from(bdpath.get(0).unwrap()))]))));
            }
        }
        add_str_property_if_exists!(result, self.added, "added");
        add_str_property_if_exists!(result, self.cast, "cast");
        add_str_property_if_exists!(result, self.container_extension, "container_extension");
        add_str_property_if_exists!(result, self.cover, "cover");
        add_str_property_if_exists!(result, self.director, "director");
        add_str_property_if_exists!(result, self.episode_run_time, "episode_run_time");
        add_str_property_if_exists!(result, self.genre, "genre");
        add_str_property_if_exists!(result, self.last_modified, "last_modified");
        add_str_property_if_exists!(result, self.plot, "plot");
        add_f64_property_if_exists!(result, self.rating, "rating");
        add_f64_property_if_exists!(result, self.rating_5based, "rating_5based");
        add_str_property_if_exists!(result, self.release_date, "release_date");
        add_str_property_if_exists!(result, self.stream_type, "stream_type");
        add_str_property_if_exists!(result, self.title, "title");
        add_str_property_if_exists!(result, self.year, "year");
        add_str_property_if_exists!(result, self.youtube_trailer, "youtube_trailer");
        add_str_property_if_exists!(result, self.epg_channel_id, "epg_channel_id");
        add_i64_property_if_exists!(result, self.tv_archive, "tv_archive");
        add_i64_property_if_exists!(result, self.tv_archive_duration, "tv_archive_duration");
        if result.is_empty() { None } else { Some(result) }
    }
}

fn process_category(category: Option<serde_json::Value>) -> Vec<XtreamCategory> {
    match category {
        Some(value) => {
            match serde_json::from_value::<Vec<XtreamCategory>>(value) {
                Ok(category_list) => category_list,
                Err(err) => {
                    error!("Failed to process categories {}", &err);
                    vec![]
                }
            }
        }
        None => vec![]
    }
}


fn process_streams(xtream_cluster: &XtreamCluster, streams: Option<serde_json::Value>) -> Vec<XtreamStream> {
    match streams {
        Some(value) => {
            match serde_json::from_value::<Vec<XtreamStream>>(value) {
                Ok(stream_list) => stream_list,
                Err(err) => {
                    error!("Failed to process streams {:?}: {}", xtream_cluster, &err);
                    vec![]
                }
            }
        }
        None => vec![]
    }
}

pub(crate) fn parse_xtream(cat_id_cnt: &AtomicI32, xtream_cluster: &XtreamCluster, category: Option<serde_json::Value>, streams: Option<serde_json::Value>, stream_base_url: &String) -> Vec<PlaylistGroup> {
    let mut categories = process_category(category);
    if !categories.is_empty() {
        let streams = process_streams(xtream_cluster, streams);
        if !streams.is_empty() {
            let mut group_map = HashMap::<String, RefCell<XtreamCategory>>::new();
            while let Some(category) = categories.pop() {
                group_map.insert(String::from(&category.category_id), RefCell::new(category));
            }

            for stream in streams {
                if let Some(group) = group_map.get(stream.category_id.as_str()) {
                    let mut grp = group.borrow_mut();
                    let title = String::from(&grp.category_name);
                    let item = PlaylistItem {
                        header: RefCell::new(PlaylistItemHeader {
                            id: stream.get_stream_id(),
                            name: String::from(&stream.name),
                            logo: String::from(&stream.stream_icon),
                            logo_small: "".to_string(),
                            group: title,
                            title: String::from(&stream.name),
                            parent_code: "".to_string(),
                            audio_track: "".to_string(),
                            time_shift: "".to_string(),
                            rec: "".to_string(),
                            source: String::from(&stream.direct_source),
                            xtream_cluster: xtream_cluster.clone(),
                            additional_properties: stream.get_additional_properties(),
                        }),
                        url: format!("{}/{}", stream_base_url, stream.get_stream_id()),
                    };
                    grp.add(item);
                }
            }

            let mut result = vec![];
            for category in group_map.values() {
                let cat = category.borrow();
                cat_id_cnt.fetch_add(1, Ordering::Relaxed);
                let group = PlaylistGroup {
                    id: cat_id_cnt.load(Ordering::Relaxed),
                    xtream_cluster: xtream_cluster.clone(),
                    title: String::from(&cat.category_name),
                    channels: cat.channels.clone(),
                };
                result.push(group);
            }

            return result;
        }
    }
    vec![]
}