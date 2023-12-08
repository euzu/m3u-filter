use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Deserializer, Serialize};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::model::model_config::{default_as_empty_rc_str};
use crate::model::model_playlist::{PlaylistItem};

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

fn value_to_string_array(value: &[Value]) -> Option<Vec<String>> {
    Some(value.iter().filter_map(value_to_string).collect())
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.to_string()),
        _ => None,
    }
}

fn deserialize_as_option_rc_string<'de, D>(deserializer: D) -> Result<Option<Rc<String>>, D::Error>
    where
        D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(Some(Rc::new(s.to_owned()))),
        Value::Number(s) => Ok(Some(Rc::new(s.to_string()))),
        _ => Ok(None),
    }
}

fn deserialize_as_rc_string<'de, D>(deserializer: D) -> Result<Rc<String>, D::Error>
    where
        D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(Rc::new(s.to_owned())),
        _ => Ok(Rc::new(value.to_string())),
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
pub(crate) struct XtreamCategory {
    #[serde(deserialize_with = "deserialize_as_rc_string")]
    pub category_id: Rc<String>,
    #[serde(deserialize_with = "deserialize_as_rc_string")]
    pub category_name: Rc<String>,
    //pub parent_id: i32,
    #[serde(default = "default_as_empty_list")]
    pub channels: Vec<PlaylistItem>,
}

impl XtreamCategory {
    pub(crate) fn add(&mut self, item: PlaylistItem) {
        self.channels.push(item);
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct XtreamStream {
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub name: Rc<String>,
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub category_id: Rc<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub stream_id: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub series_id: Option<i32>,
    #[serde(default = "default_as_empty_rc_str", deserialize_with = "deserialize_as_rc_string")]
    pub stream_icon: Rc<String>,
    #[serde(default = "default_as_empty_rc_str", deserialize_with = "deserialize_as_rc_string")]
    pub direct_source: Rc<String>,

    // optional attributes
    #[serde(default, deserialize_with = "deserialize_as_string_array")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub added: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub cast: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub container_extension: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub cover: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub director: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub episode_run_time: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub genre: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub last_modified: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub plot: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub rating: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub rating_5based: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub release_date: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub stream_type: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub title: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub year: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub youtube_trailer: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub epg_channel_id: Option<Rc<String>>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub tv_archive: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub tv_archive_duration: Option<i32>,
}

macro_rules! add_str_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
        $vec.push((String::from($prop_name), Value::String($prop.to_string())));
    }
}
macro_rules! add_rc_str_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::String(v.to_string()))));
    }
}
macro_rules! add_opt_i64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from(i64::from(*v))))));
    }
}

macro_rules! add_opt_f64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from_f64(f64::from(*v)).unwrap()))));
    }
}

macro_rules! add_f64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from_f64(f64::from($prop)).unwrap())));
    }
}

macro_rules! add_i64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $vec.push((String::from($prop_name), Value::Number(serde_json::value::Number::from(i64::from($prop)))));
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
        add_rc_str_property_if_exists!(result, self.added, "added");
        add_rc_str_property_if_exists!(result, self.cast, "cast");
        add_rc_str_property_if_exists!(result, self.container_extension, "container_extension");
        add_rc_str_property_if_exists!(result, self.cover, "cover");
        add_rc_str_property_if_exists!(result, self.director, "director");
        add_rc_str_property_if_exists!(result, self.episode_run_time, "episode_run_time");
        add_rc_str_property_if_exists!(result, self.genre, "genre");
        add_rc_str_property_if_exists!(result, self.last_modified, "last_modified");
        add_rc_str_property_if_exists!(result, self.plot, "plot");
        add_opt_f64_property_if_exists!(result, self.rating, "rating");
        add_opt_f64_property_if_exists!(result, self.rating_5based, "rating_5based");
        add_rc_str_property_if_exists!(result, self.release_date, "release_date");
        add_rc_str_property_if_exists!(result, self.stream_type, "stream_type");
        add_rc_str_property_if_exists!(result, self.title, "title");
        add_rc_str_property_if_exists!(result, self.year, "year");
        add_rc_str_property_if_exists!(result, self.youtube_trailer, "youtube_trailer");
        //add_str_property_if_exists!(result, self.epg_channel_id, "epg_channel_id");
        add_opt_i64_property_if_exists!(result, self.tv_archive, "tv_archive");
        add_opt_i64_property_if_exists!(result, self.tv_archive_duration, "tv_archive_duration");
        if result.is_empty() { None } else { Some(result) }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct XtreamSeriesInfoSeason {
    pub air_date: String,
    pub episode_count: u32,
    pub id: u32,
    pub name: String,
    pub overview: String,
    pub season_number: u32,
    pub vote_average: f64,
    pub cover: String,
    pub cover_big: String,

}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub(crate) struct XtreamSeriesInfoInfo {
    name: String,
    cover: String,
    plot: String,
    cast: String,
    director: String,
    genre: String,
    releaseDate: String,
    last_modified: String,
    rating: String,
    rating_5based: f64,
    backdrop_path: Vec<String>,
    youtube_trailer: String,
    episode_run_time: String,
    category_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct XtreamSeriesInfoEpisodeInfo {
    pub tmdb_id: u32,
    pub releasedate: String,
    pub plot: String,
    pub duration_secs: u32,
    pub duration: String,
    pub movie_image: String,
    // "video": [],
    // "audio": [],
    pub bitrate: u32,
    pub rating: f64,
    pub season: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct XtreamSeriesInfoEpisode {
    pub id: String,
    pub episode_num: u32,
    pub title: String,
    pub container_extension: String,
    pub info: XtreamSeriesInfoEpisodeInfo,
    pub custom_sid: String,
    pub added: String,
    pub season: u32,
    pub direct_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct XtreamSeriesInfo {
    pub seasons: Vec<XtreamSeriesInfoSeason>,
    pub info: XtreamSeriesInfoInfo,
    pub episodes: HashMap<String, Vec<XtreamSeriesInfoEpisode>>,
}


impl XtreamSeriesInfoEpisode {
    pub(crate) fn get_additional_properties(&self, series_info: &XtreamSeriesInfo) -> Option<Vec<(String, Value)>> {
        let mut result = vec![];
        let bdpath = &series_info.info.backdrop_path;
        if !bdpath.is_empty() {
            result.push((String::from("backdrop_path"), Value::Array(Vec::from([Value::String(String::from(bdpath.get(0).unwrap()))]))));
        }
        add_str_property_if_exists!(result, self.added.as_str(), "added");
        add_str_property_if_exists!(result, series_info.info.cast.as_str(), "cast");
        add_str_property_if_exists!(result, self.container_extension.as_str(), "container_extension");
        add_str_property_if_exists!(result, self.info.movie_image, "cover");
        add_str_property_if_exists!(result, series_info.info.director, "director");
        add_str_property_if_exists!(result, series_info.info.episode_run_time, "episode_run_time");
        add_str_property_if_exists!(result, series_info.info.last_modified, "last_modified");
        add_str_property_if_exists!(result, self.info.plot, "plot");
        add_str_property_if_exists!(result, series_info.info.rating, "rating");
        add_f64_property_if_exists!(result, series_info.info.rating_5based, "rating_5based");
        add_str_property_if_exists!(result, self.info.releasedate, "release_date");
        add_str_property_if_exists!(result, self.title, "title");
        add_i64_property_if_exists!(result, self.season, "season");
        add_str_property_if_exists!(result, series_info.info.youtube_trailer, "youtube_trailer");
        if result.is_empty() { None } else { Some(result) }
    }
}