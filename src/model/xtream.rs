use std::collections::HashMap;
use std::iter::FromIterator;
use std::rc::Rc;

use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::ConfigTargetOptions;
use crate::model::playlist::{PlaylistEntry, PlaylistItem, XtreamCluster, XtreamPlaylistItem};
use crate::utils::json_utils::{opt_string_or_number_u32, string_default_on_null, string_or_number_f64, string_or_number_u32};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

const PROP_BACKDROP_PATH: &str = "backdrop_path";
const PROP_COVER: &str = "cover";

const LIVE_STREAM_FIELDS: &[&str] = &[];

const VIDEO_STREAM_FIELDS: &[&str] = &[
    "release_date", "cast",
    "director", "episode_run_time", "genre",
    "stream_type", "title", "year", "youtube_trailer",
    "plot", "rating_5based", "stream_icon", "container_extension"
];

const SERIES_STREAM_FIELDS: &[&str] = &[
    PROP_BACKDROP_PATH, "cast", PROP_COVER, "director", "episode_run_time", "genre",
    "last_modified", "name", "plot", "rating_5based",
    "stream_type", "title", "year", "youtube_trailer",
];


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
            serde_json::from_str::<T>(string.as_str()).map_or_else(|_| Ok(None), |val| Ok(Some(val)))
        }
    }
}

fn value_to_string_array(value: &[Value]) -> Vec<String> {
    value.iter().filter_map(value_to_string).collect()
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
        Value::Null => Ok(Rc::new(String::new())),
        _ => Ok(Rc::new(value.to_string())),
    }
}

fn deserialize_as_string_array<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(|v| match v {
        Value::String(value) => Some(vec![value]),
        Value::Array(value) => Some(value_to_string_array(&value)),
        _ => None,
    })
}

#[derive(Deserialize, Default)]
pub struct XtreamCategory {
    #[serde(deserialize_with = "deserialize_as_rc_string")]
    pub category_id: Rc<String>,
    #[serde(deserialize_with = "deserialize_as_rc_string")]
    pub category_name: Rc<String>,
    //pub parent_id: i32,
    #[serde(default)]
    pub channels: Vec<PlaylistItem>,
}

impl XtreamCategory {
    pub fn add(&mut self, item: PlaylistItem) {
        self.channels.push(item);
    }
}

#[derive(Serialize, Deserialize)]
pub struct XtreamStream {
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub name: Rc<String>,
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub category_id: Rc<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub stream_id: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub series_id: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub stream_icon: Rc<String>,
    #[serde(default, deserialize_with = "deserialize_as_rc_string")]
    pub direct_source: Rc<String>,

    // optional attributes
    #[serde(default, deserialize_with = "deserialize_as_option_rc_string")]
    pub custom_sid: Option<Rc<String>>,
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
        $vec.insert(String::from($prop_name), Value::String($prop.to_string()));
    }
}
macro_rules! add_rc_str_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.insert(String::from($prop_name), Value::String(v.to_string())));
    }
}

macro_rules! add_opt_i64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.insert(String::from($prop_name), Value::Number(serde_json::value::Number::from(i64::from(*v)))));
    }
}

macro_rules! add_opt_f64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $prop.as_ref().map(|v| $vec.insert(String::from($prop_name), Value::Number(serde_json::value::Number::from_f64(f64::from(*v)).unwrap())));
    }
}

macro_rules! add_f64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $vec.insert(String::from($prop_name), Value::Number(serde_json::value::Number::from_f64(f64::from($prop)).unwrap()));
    }
}

macro_rules! add_i64_property_if_exists {
    ($vec:expr, $prop:expr, $prop_name:expr) => {
       $vec.insert(String::from($prop_name), Value::Number(serde_json::value::Number::from(i64::from($prop))));
    }
}

macro_rules! add_to_doc_str_property_if_not_exists {
    ($document:expr, $prop_name:expr, $prop_value:expr) => {
          match $document.get($prop_name) {
            None => {
                $document.insert(String::from($prop_name), $prop_value);
            }
            Some(value) => { if Value::is_null(value) {
                $document.insert(String::from($prop_name), $prop_value);
            }}
          }
    }
}


impl XtreamStream {
    pub fn get_stream_id(&self) -> u32 {
        self.stream_id.unwrap_or_else(|| self.series_id.unwrap_or(0))
    }

    pub fn get_additional_properties(&self) -> Option<Value> {
        let mut result = Map::new();
        if let Some(bdpath) = self.backdrop_path.as_ref() {
            if !bdpath.is_empty() {
                result.insert(String::from("backdrop_path"), Value::Array(Vec::from([Value::String(String::from(bdpath.first().unwrap()))])));
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
        add_rc_str_property_if_exists!(result, self.epg_channel_id, "epg_channel_id");
        add_opt_i64_property_if_exists!(result, self.tv_archive, "tv_archive");
        add_opt_i64_property_if_exists!(result, self.tv_archive_duration, "tv_archive_duration");
        if result.is_empty() { None } else { Some(Value::Object(result)) }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtreamSeriesInfoSeason {
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub air_date: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub episode_count: u32,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub id: u32,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub name: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub overview: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub season_number: u32,
    #[serde(default, deserialize_with = "string_or_number_f64")]
    pub vote_average: f64,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub cover: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub cover_big: String,

}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub struct XtreamSeriesInfoInfo {
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub(crate) name: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    cover: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    plot: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    cast: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    director: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    genre: String,
    #[serde(
        default,
        alias = "release_date",
        alias = "releaseDate",
        alias = "releasedate",
        deserialize_with = "string_default_on_null"
    )]
    release_date: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    last_modified: String,
    #[serde(default, deserialize_with = "string_or_number_f64")]
    rating: f64,
    #[serde(default, deserialize_with = "string_or_number_f64")]
    rating_5based: f64,
    #[serde(default, deserialize_with = "deserialize_as_string_array")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default, deserialize_with = "string_default_on_null")]
    youtube_trailer: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    episode_run_time: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    category_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XtreamSeriesInfoEpisodeInfo {
    #[serde(default, deserialize_with = "opt_string_or_number_u32")]
    pub tmdb_id: Option<u32>,
    #[serde(
        default,
        alias = "release_date",
        alias = "releaseDate",
        alias = "releasedate",
        deserialize_with = "string_default_on_null"
    )]
    pub releasedate: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub plot: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub duration_secs: u32,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub duration: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub movie_image: String,
    #[serde(default)]
    pub video: Value,
    #[serde(default)]
    pub audio: Value,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub bitrate: u32,
    #[serde(default, deserialize_with = "string_or_number_f64")]
    pub rating: f64,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub season: u32,
}

// Used for serde_json deserialization, can not be used with bincode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtreamSeriesInfoEpisode {
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub id: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub episode_num: u32,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub title: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub container_extension: String,
    #[serde(default)]
    pub info: Option<XtreamSeriesInfoEpisodeInfo>,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub custom_sid: String,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub added: String,
    #[serde(default, deserialize_with = "string_or_number_u32")]
    pub season: u32,
    #[serde(default, deserialize_with = "string_default_on_null")]
    pub direct_source: String,
}

impl XtreamSeriesInfoEpisode {
    pub fn get_id(&self) -> u32 {
        self.id.parse::<u32>().unwrap_or(0)
    }
}

//bincode does not support deserialize_with. We use this struct for db
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtreamSeriesEpisode {
    pub id: u32,
    pub episode_num: u32,
    pub title: String,
    pub container_extension: String,
    pub custom_sid: String,
    pub added: String,
    pub season: u32,
    pub tmdb_id: u32,
    pub direct_source: String,
}

impl XtreamSeriesEpisode {
    pub fn from(info_episode: &XtreamSeriesInfoEpisode) -> Self {
        Self {
            id: info_episode.get_id(),
            episode_num: info_episode.episode_num,
            title: info_episode.title.to_string(),
            container_extension: info_episode.container_extension.to_string(),
            custom_sid: info_episode.custom_sid.to_string(),
            added: info_episode.added.to_string(),
            season: info_episode.season,
            tmdb_id: info_episode.info.as_ref().and_then(|info| info.tmdb_id).unwrap_or(0),
            direct_source: info_episode.direct_source.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtreamSeriesInfo {
    #[serde(default)]
    pub seasons: Option<Vec<XtreamSeriesInfoSeason>>,
    #[serde(default)]
    pub info: Option<XtreamSeriesInfoInfo>,
    #[serde(default)]
    pub episodes: Option<HashMap<String, Vec<XtreamSeriesInfoEpisode>>>,
}


impl XtreamSeriesInfoEpisode {
    pub fn get_additional_properties(&self, series_info: &XtreamSeriesInfo) -> Option<Value> {
        let mut result = Map::new();
        let info = series_info.info.as_ref();
        let bdpath = info.and_then(|i| i.backdrop_path.as_ref());
        let bdpath_is_set = bdpath.as_ref().is_some_and(|bdpath| !bdpath.is_empty());
        if bdpath_is_set {
            result.insert(String::from("backdrop_path"), Value::Array(Vec::from([Value::String(String::from(bdpath.as_ref().unwrap().first()?))])));
        }
        add_str_property_if_exists!(result, info.map_or("", |i| i.name.as_str()), "series_name");
        add_str_property_if_exists!(result, info.map_or("", |i| i.release_date.as_str()), "series_release_date");
        add_str_property_if_exists!(result, self.added.as_str(), "added");
        add_str_property_if_exists!(result, info.map_or("", |i| i.cast.as_str()), "cast");
        add_str_property_if_exists!(result, self.container_extension.as_str(), "container_extension");
        add_str_property_if_exists!(result, self.info.as_ref().map_or("", |info| info.movie_image.as_str()), "cover");
        add_str_property_if_exists!(result, info.map_or("", |i| i.director.as_str()), "director");
        add_str_property_if_exists!(result, info.map_or("", |i| i.episode_run_time.as_str()), "episode_run_time");
        add_str_property_if_exists!(result, info.map_or("", |i| i.last_modified.as_str()), "last_modified");
        add_str_property_if_exists!(result, self.info.as_ref().map_or("", |info| info.plot.as_str()), "plot");
        add_f64_property_if_exists!(result, info.map_or(0_f64, |i| i.rating), "rating");
        add_f64_property_if_exists!(result, info.map_or(0_f64, |i| i.rating_5based), "rating_5based");
        add_str_property_if_exists!(result, self.info.as_ref().map_or("", |info| info.releasedate.as_str()), "release_date");
        add_str_property_if_exists!(result, self.title, "title");
        add_i64_property_if_exists!(result, self.season, "season");
        add_i64_property_if_exists!(result, self.episode_num, "episode");
        add_opt_i64_property_if_exists!(result, self.info.as_ref().and_then(|info| info.tmdb_id), "tmdb_id");
        if result.is_empty() { None } else { Some(Value::Object(result)) }
    }
}

pub struct XtreamMappingOptions {
    pub skip_live_direct_source: bool,
    pub skip_video_direct_source: bool,
    pub skip_series_direct_source: bool,
}

impl XtreamMappingOptions {
    pub fn from_target_options(options: Option<&ConfigTargetOptions>) -> Self {
        let (skip_live_direct_source, skip_video_direct_source, skip_series_direct_source) = options
            .map_or((false, false, false), |o| (
                o.xtream_skip_live_direct_source,
                o.xtream_skip_video_direct_source,
                o.xtream_skip_series_direct_source));
        Self {
            skip_live_direct_source,
            skip_video_direct_source,
            skip_series_direct_source,
        }
    }
}

fn append_release_date(document: &mut serde_json::Map<String, Value>) {
    let release_date = document
        .get("release_date")
        .or_else(|| document.get("releaseDate"))
        .cloned()
        .unwrap_or(Value::Null);

    if !document.contains_key("release_date") {
        document.insert("release_date".to_string(), release_date.clone());
    }
    if !document.contains_key("releaseDate") {
        document.insert("releaseDate".to_string(), release_date);
    }
}

fn append_mandatory_fields(document: &mut serde_json::Map<String, Value>, fields: &[&str]) {
    for &field in fields {
        if !document.contains_key(field) {
            document.insert(field.to_string(), Value::Null);
        }
    }
}

fn append_prepared_series_properties(add_props: Option<&Map<String, Value>>, document: &mut Map<String, Value>) {
    if let Some(props) = add_props {
        match props.get("rating") {
            Some(value) => {
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

fn make_bdpath_resource_url(resource_url: &str, bd_path: &str, index: usize) -> String {
    if bd_path.starts_with("http") {
        format!("{resource_url}/bd_path_{index}")
    } else {
        bd_path.to_string()
    }
}

pub fn xtream_playlistitem_to_document(pli: &XtreamPlaylistItem, url: &str, options: &XtreamMappingOptions, user: &ProxyUserCredentials) -> serde_json::Value {
    let stream_id_value = Value::Number(serde_json::Number::from(pli.virtual_id));
    let (resource_url, logo, logo_small) = match user.proxy {
        ProxyType::Reverse => {
            let resource_url = format!("{url}/resource/{}/{}/{}/{}", pli.xtream_cluster.as_stream_type(), user.username, user.password, pli.get_virtual_id());
            let logo_url = if pli.logo.is_empty() { String::new() } else { format!("{resource_url}/logo") };
            let logo_small_url = if pli.logo_small.is_empty() { String::new() } else { format!("{resource_url}/logo_small") };
            (Some(resource_url), logo_url, logo_small_url)
        }
        ProxyType::Redirect => {
            (None, pli.logo.as_ref().clone(), pli.logo_small.as_ref().clone())
        }
    };
    let mut document = serde_json::Map::from_iter([
        ("category_id".to_string(), Value::String(format!("{}", &pli.category_id))),
        ("category_ids".to_string(), Value::Array(Vec::from([Value::Number(serde_json::Number::from(pli.category_id))]))),
        ("name".to_string(), Value::String(pli.name.as_ref().clone())),
        ("num".to_string(), stream_id_value.clone()),
        ("title".to_string(), Value::String(pli.title.as_ref().clone())),
        ("stream_icon".to_string(), Value::String(logo)),
    ]);

    match pli.xtream_cluster {
        XtreamCluster::Live => {
            document.insert("stream_id".to_string(), stream_id_value);
            if options.skip_live_direct_source {
                document.insert("direct_source".to_string(), Value::String(String::new()));
            } else {
                document.insert("direct_source".to_string(), Value::String(pli.url.as_ref().clone()));
            }
            document.insert("thumbnail".to_string(), Value::String(logo_small));
            document.insert("custom_sid".to_string(), Value::String(String::new()));
            document.insert("epg_channel_id".to_string(), pli.epg_channel_id.as_ref().map_or(Value::Null, |epg_id| Value::String(epg_id.as_ref().clone())));
        }
        XtreamCluster::Video => {
            document.insert("stream_id".to_string(), stream_id_value);
            if options.skip_video_direct_source {
                document.insert("direct_source".to_string(), Value::String(String::new()));
            } else {
                document.insert("direct_source".to_string(), Value::String(pli.url.as_ref().clone()));
            }
            document.insert("custom_sid".to_string(), Value::String(String::new()));
        }
        XtreamCluster::Series => {
            document.insert("series_id".to_string(), stream_id_value);
        }
    };

    let props = pli.additional_properties.as_ref().and_then(|add_props| serde_json::from_str::<Map<String, Value>>(add_props).ok());

    if let Some(ref add_props) = props {
        for (field_name, field_value) in add_props {
            document.insert(field_name.to_string(), field_value.to_owned());
        }
    }

    match pli.xtream_cluster {
        XtreamCluster::Live => {
            append_mandatory_fields(&mut document, LIVE_STREAM_FIELDS);
            add_to_doc_str_property_if_not_exists!(document, "stream_type", Value::String(String::from("live")));
            add_to_doc_str_property_if_not_exists!(document, "added", Value::String(chrono::Utc::now().timestamp().to_string()));
        }
        XtreamCluster::Video => {
            append_mandatory_fields(&mut document, VIDEO_STREAM_FIELDS);
            add_to_doc_str_property_if_not_exists!(document, "stream_type", Value::String(String::from("movie")));
            add_to_doc_str_property_if_not_exists!(document, "added", Value::String(chrono::Utc::now().timestamp().to_string()));
        }
        XtreamCluster::Series => {
            append_prepared_series_properties(props.as_ref(), &mut document);
            append_mandatory_fields(&mut document, SERIES_STREAM_FIELDS);
            append_release_date(&mut document);
        }
    };

    if let Some(rewrite_url) = resource_url {
        if let Some(bdpath) = document.get(PROP_BACKDROP_PATH) {
            match bdpath {
                Value::String(bd_path) => {
                    document.insert(PROP_BACKDROP_PATH.to_string(), Value::Array(vec![Value::String(make_bdpath_resource_url(rewrite_url.as_str(), bd_path, 0))]));
                }
                Value::Array(bd_path) => {
                    document.insert(PROP_BACKDROP_PATH.to_string(), Value::Array(
                        bd_path.iter()
                            .filter_map(|val| val.as_str())
                            .enumerate()
                            .map(|(index, value)| Value::String(make_bdpath_resource_url(rewrite_url.as_str(), value, index)))
                            .collect()));
                }
                _ => {}
            };
        }
        if let Some(Value::String(cover)) = document.get(PROP_COVER) {
            if cover.starts_with("http") {
                document.insert(PROP_COVER.to_string(), Value::String(format!("{}/cover", rewrite_url.as_str())));
            }
        }
    }

    Value::Object(document)
}
