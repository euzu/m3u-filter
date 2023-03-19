use std::cell::RefCell;
use std::collections::HashMap;
use serde::{Deserialize};
use crate::m3u::{PlaylistGroup, PlaylistItem, PlaylistItemHeader};
use crate::model::{default_as_empty_str};


pub fn default_as_empty_list() -> Vec<PlaylistItem> { vec![] }

#[derive(Deserialize)]
struct XtreamCategory {
    pub category_id: String,
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
    pub num: i32,
    pub name: String,
    //pub stream_type: String,
    pub stream_id: i32,
    pub stream_icon: String,
    //pub epg_channel_id: String,
    // pub added: String,
    pub category_id: String,
    //pub custom_sid: String,
    //pub tv_archive: i32,
    #[serde(default = "default_as_empty_str")]
    pub direct_source: String,
    //pub tv_archive_duration: i32,
}

fn decode_category(category: Option<serde_json::Value>) -> Vec<XtreamCategory> {
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


fn decode_streams(streams: Option<serde_json::Value>) -> Vec<XtreamStream> {
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

pub(crate) fn decode(category: Option<serde_json::Value>, streams: Option<serde_json::Value>, stream_base_url: &String) -> Vec<PlaylistGroup> {
    let mut categories = decode_category(category);
    if !categories.is_empty() {
        let streams = decode_streams(streams);
        if !streams.is_empty() {
            let mut group_map = HashMap::<String, RefCell<XtreamCategory>>::new();
            while let Some(category) = categories.pop() {
                group_map.insert(String::from(&category.category_id), RefCell::new(category));
            }

            for stream in streams {
                match  group_map.get(stream.category_id.as_str()) {
                    Some(group) => {
                        let mut grp = group.borrow_mut();
                        let title = String::from(&grp.category_name);
                        let item = PlaylistItem {
                            header: PlaylistItemHeader {
                                id: stream.stream_id.to_string(),
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
                                chno: stream.num.to_string(),
                            },
                            url: format!("{}/{}", stream_base_url, stream.stream_id),
                        };
                        grp.add(item);
                    }
                    None => {}
                }
            }

            let mut result = vec![];
            for category in group_map.values() {
                let cat  = category.borrow();
                let group = PlaylistGroup {
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