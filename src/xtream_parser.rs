use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use crate::model_m3u::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, XtreamCluster};
use crate::model_config::{default_as_empty_str};

fn null_to_default<'de, D, T>(d: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: Default + Deserialize<'de>,
{
    let opt = Option::deserialize(d)?;
    let val = opt.unwrap_or_default();
    Ok(val)
}

fn default_as_empty_list() -> Vec<PlaylistItem> { vec![] }

#[derive(Deserialize)]
struct XtreamCategory {
    #[serde(deserialize_with = "null_to_default")]
    pub category_id: String,
    #[serde(deserialize_with = "null_to_default")]
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

#[derive(Deserialize)]
struct XtreamStream {
    #[serde(deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(deserialize_with = "null_to_default")]
    pub category_id: String,
    #[serde(deserialize_with = "null_to_default")]
    pub stream_id: Option<i32>,
    #[serde(deserialize_with = "null_to_default")]
    pub series_id: Option<i32>,
    #[serde(default = "default_as_empty_str", deserialize_with = "null_to_default")]
    pub stream_icon: String,
    #[serde(default = "default_as_empty_str", deserialize_with = "null_to_default")]
    pub direct_source: String,

    // optional attributes
    #[serde(deserialize_with = "null_to_default")]
    added: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    cast: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    container_extension: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    director: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    episode_run_time: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    genre: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    plot: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    rating: Option<f32>,
    #[serde(deserialize_with = "null_to_default")]
    rating_5based: Option<f32>,
    release_date: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    stream_type: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    title: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    year: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    youtube_trailer: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    epg_channel_id: Option<String>,
    #[serde(deserialize_with = "null_to_default")]
    tv_archive: Option<i32>,
    #[serde(deserialize_with = "null_to_default")]
    tv_archive_duration: Option<i32>,
}

impl XtreamStream {
    pub(crate) fn get_stream_id(&self) -> String {
        self.stream_id.map_or_else(|| self.series_id.map_or_else(|| String::from(""), |seid| format!("{}", seid)), |sid| format!("{}", sid))
    }

    pub(crate) fn get_additional_properties(&self) -> Option<Vec<(String, Value)>> {
        let mut result = vec![];
        self.added.as_ref().map(|v| result.push((String::from("added"), Value::String(v.to_string()))));
        self.cast.as_ref().map(|v| result.push((String::from("cast"), Value::String(v.to_string()))));
        self.container_extension.as_ref().map(|v| result.push((String::from("container_extension"), Value::String(v.to_string()))));
        self.director.as_ref().map(|v| result.push((String::from("director"), Value::String(v.to_string()))));
        self.episode_run_time.as_ref().map(|v| result.push((String::from("episode_run_time"), Value::String(v.to_string()))));
        self.genre.as_ref().map(|v| result.push((String::from("genre"), Value::String(v.to_string()))));
        self.plot.as_ref().map(|v| result.push((String::from("plot"), Value::String(v.to_string()))));
        self.rating.as_ref().map(|v| result.push((String::from("rating"), Value::Number(serde_json::value::Number::from_f64(f64::from(*v)).unwrap()))));
        self.rating_5based.as_ref().map(|v| result.push((String::from("rating_5based"), Value::Number(serde_json::value::Number::from_f64(f64::from(*v)).unwrap()))));
        self.release_date.as_ref().map(|v| result.push((String::from("release_date"), Value::String(v.to_string()))));
        self.stream_type.as_ref().map(|v| result.push((String::from("stream_type"), Value::String(v.to_string()))));
        self.title.as_ref().map(|v| result.push((String::from("title"), Value::String(v.to_string()))));
        self.year.as_ref().map(|v| result.push((String::from("year"), Value::String(v.to_string()))));
        self.youtube_trailer.as_ref().map(|v| result.push((String::from("youtube_trailer"), Value::String(v.to_string()))));
        self.epg_channel_id.as_ref().map(|v| result.push((String::from("epg_channel_id"), Value::String(v.to_string()))));
        self.tv_archive.as_ref().map(|v| result.push((String::from("tv_archive"), Value::Number(serde_json::value::Number::from(i64::from(*v))))));
        self.tv_archive_duration.as_ref().map(|v| result.push((String::from("tv_archive_duration"), Value::Number(serde_json::value::Number::from(i64::from(*v))))));
        if result.is_empty() { None } else { Some(result) }
    }
}

fn process_category(category: Option<serde_json::Value>) -> Vec<XtreamCategory> {
    match category {
        Some(value) => {
            match serde_json::from_value::<Vec<XtreamCategory>>(value) {
                Ok(category_list) => category_list,
                Err(err) => {
                    println!("Failed to process categories {}", &err);
                    vec![]
                }
            }
        }
        None => vec![]
    }
}


fn process_streams(streams: Option<serde_json::Value>) -> Vec<XtreamStream> {
    match streams {
        Some(value) => {
            match serde_json::from_value::<Vec<XtreamStream>>(value) {
                Ok(stream_list) => stream_list,
                Err(err) => {
                    println!("Failed to process streams {}", &err);
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
        let streams = process_streams(streams);
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