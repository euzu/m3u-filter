use std::cell::RefCell;
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use crate::model::api_proxy::ProxyUserCredentials;
use crate::model::config::{ConfigInput, ConfigTargetOptions};
use crate::model::xmltv::TVGuide;
use crate::model::xtream::{xtream_playlistitem_to_document, XtreamMappingOptions};
use crate::processing::m3u_parser::extract_id_from_url;
use crate::repository::storage::hash_string;
use crate::utils::json_utils::{get_string_from_serde_value, get_u64_from_serde_value};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html

pub trait PlaylistEntry {
    fn get_virtual_id(&self) -> u32;
    fn get_provider_id(&self) -> Option<u32>;
    fn get_category_id(&self) -> Option<u32>;
    fn get_provider_url(&self) -> Rc<String>;
}

#[derive(Debug, Clone)]
pub struct FetchedPlaylist<'a> { // Contains playlist for one input
    pub input: &'a ConfigInput,
    pub playlistgroups: Vec<PlaylistGroup>,
    pub epg: Option<TVGuide>,
}

impl FetchedPlaylist<'_> {
    pub fn update_playlist(&mut self, plg: &PlaylistGroup) {
        for grp in &mut self.playlistgroups {
            if grp.id == plg.id {
                plg.channels.iter().for_each(|item| grp.channels.push(item.clone()));
                return;
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum XtreamCluster {
    #[default]
    Live = 1,
    Video = 2,
    Series = 3,
}

impl XtreamCluster {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Live => "Live",
            Self::Video => "Video",
            Self::Series => "Series",
        }
    }
    pub const fn as_stream_type(&self) -> &str {
        match self {
            Self::Live => "live",
            Self::Video => "movie",
            Self::Series => "series",
        }
    }
}

impl Display for XtreamCluster {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<PlaylistItemType> for XtreamCluster {
    type Error = String;
    fn try_from(item_type: PlaylistItemType) -> Result<Self, Self::Error> {
        match item_type {
            PlaylistItemType::Live => Ok(Self::Live),
            PlaylistItemType::Video => Ok(Self::Video),
            PlaylistItemType::Series => Ok(Self::Series),
            _ => Err(format!("Cant convert {item_type}")),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum PlaylistItemType {
    #[default]
    Live = 1,
    Video = 2,
    Series = 3, //  xtream series description
    SeriesInfo = 4, //  xtream series info fetched for series description
    Catchup = 5,
    LiveUnknown = 6, // No Provider id
    LiveHls = 7, // m3u8 entry
}

impl From<XtreamCluster> for PlaylistItemType {
    fn from(xtream_cluster: XtreamCluster) -> Self {
        match xtream_cluster {
            XtreamCluster::Live => Self::Live,
            XtreamCluster::Video => Self::Video,
            XtreamCluster::Series => Self::SeriesInfo,
        }
    }
}

impl PlaylistItemType {
    const LIVE: &'static str = "live";
    const VIDEO: &'static str = "video";
    const SERIES: &'static str = "series";
    const SERIES_INFO: &'static str = "series-info";
    const CATCHUP: &'static str = "catchup";
}

impl Display for PlaylistItemType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Live | Self::LiveHls | Self::LiveUnknown => Self::LIVE,
            Self::Video => Self::VIDEO,
            Self::Series => Self::SERIES,
            Self::SeriesInfo => Self::SERIES_INFO,
            Self::Catchup => Self::CATCHUP,
        })
    }
}

pub trait FieldGetAccessor {
    fn get_field(&self, field: &str) -> Option<Rc<String>>;
}
pub trait FieldSetAccessor {
    fn set_field(&mut self, field: &str, value: &str) -> bool;
}

pub type UUIDType = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaylistItemHeader {
    pub uuid: Rc<UUIDType>, // calculated
    pub id: Rc<String>, // provider id
    pub virtual_id: u32, // virtual id
    pub name: Rc<String>,
    pub chno: Rc<String>,
    pub logo: Rc<String>,
    pub logo_small: Rc<String>,
    pub group: Rc<String>,
    pub title: Rc<String>,
    pub parent_code: Rc<String>,
    pub audio_track: Rc<String>,
    pub time_shift: Rc<String>,
    pub rec: Rc<String>,
    pub url: Rc<String>,
    pub epg_channel_id: Option<Rc<String>>,
    pub xtream_cluster: XtreamCluster,
    pub additional_properties: Option<Value>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub item_type: PlaylistItemType,
    #[serde(default)]
    pub category_id: u32,
    #[serde(default)]
    pub input_id: u16,
}

impl PlaylistItemHeader {
    pub fn gen_uuid(&mut self) {
        self.uuid = Rc::new(hash_string(&self.url));
    }
    pub const fn get_uuid(&self) -> &Rc<UUIDType> {
        &self.uuid
    }

    pub fn get_provider_id(&mut self) -> Option<u32> {
        match self.id.parse::<u32>() {
            Ok(id) => Some(id),
            Err(_) => match extract_id_from_url(&self.url) {
                Some(id) => match id.parse::<u32>() {
                    Ok(newid) => {
                        self.id = Rc::new(newid.to_string());
                        Some(newid)
                    }
                    Err(_) => None,
                },
                None => None,
            }
        }
    }

    pub fn get_additional_property(&self, field: &str) -> Option<&Value> {
        self.additional_properties.as_ref().and_then(|v| match v {
            Value::Object(map) => {
                map.get(field)
            }
            _ => None,
        })
    }
    // pub fn get_additional_property_as_u32(&self, field: &str) -> Option<u32> {
    //     match self.get_additional_property(field) {
    //         Some(value) => get_u32_from_serde_value(value),
    //         None => None
    //     }
    // }
    pub fn get_additional_property_as_u64(&self, field: &str) -> Option<u64> {
        match self.get_additional_property(field) {
            Some(value) => get_u64_from_serde_value(value),
            None => None
        }
    }

    pub fn get_additional_property_as_str(&self, field: &str) -> Option<String> {
        match self.get_additional_property(field) {
            Some(value) => get_string_from_serde_value(value),
            None => None
        }
    }
}

macro_rules! to_m3u_non_empty_fields {
    ($header:expr, $line:expr, $(($prop:ident, $field:expr)),*;) => {
        $(
           if !$header.$prop.is_empty() {
                $line = format!("{} {}=\"{}\"", $line, $field, $header.$prop);
            }
         )*
    };
}

macro_rules! to_m3u_resource_non_empty_fields {
    ($header:expr, $url:expr, $line:expr, $(($prop:ident, $field:expr)),*;) => {
        $(
           if !$header.$prop.is_empty() {
                $line = format!("{} {}=\"{}/{}\"", $line, $field, $url, stringify!($prop));
            }
         )*
    };
}

macro_rules! generate_field_accessor_impl_for_playlist_item_header {
    ($($prop:ident),*;) => {
        impl FieldGetAccessor for PlaylistItemHeader {
            fn get_field(&self, field: &str) -> Option<Rc<String>> {
                match field {
                    $(
                        stringify!($prop) => Some(self.$prop.clone()),
                    )*
                    "epg_channel_id" | "epg_id" => self.epg_channel_id.clone(),
                    _ => None,
                }
            }
         }
         impl FieldSetAccessor for PlaylistItemHeader {
            fn set_field(&mut self, field: &str, value: &str) -> bool {
                let val = String::from(value);
                match field {
                    $(
                        stringify!($prop) => {
                            self.$prop = Rc::new(val);
                            true
                        }
                    )*
                    "epg_channel_id" | "epg_id" => {
                        self.epg_channel_id = Some(Rc::new(value.to_owned()));
                        true
                    }
                    _ => false,
                }
            }
        }
    }
}

generate_field_accessor_impl_for_playlist_item_header!(id, /*virtual_id,*/ name, chno, logo, logo_small, group, title, parent_code, audio_track, time_shift, rec, url;);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M3uPlaylistItem {
    pub virtual_id: u32,
    pub provider_id: Rc<String>,
    pub name: Rc<String>,
    pub chno: Rc<String>,
    pub logo: Rc<String>,
    pub logo_small: Rc<String>,
    pub group: Rc<String>,
    pub title: Rc<String>,
    pub parent_code: Rc<String>,
    pub audio_track: Rc<String>,
    pub time_shift: Rc<String>,
    pub rec: Rc<String>,
    pub url: Rc<String>,
    pub epg_channel_id: Option<Rc<String>>,
    pub input_id: u16,
    pub item_type: PlaylistItemType,
}

impl M3uPlaylistItem {
    pub fn to_m3u(&self, target_options: Option<&ConfigTargetOptions>, rewrite_urls: Option<&(String, String)>) -> String {
        let (stream_url, resource_url) = rewrite_urls
            .map_or_else(|| (self.url.as_str(), None), |(su, ru)| (su.as_str(), Some(ru.as_str())));

        let options = target_options.as_ref();
        let ignore_logo = options.is_some_and(|o| o.ignore_logo);
        let mut line = format!("#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\" group-title=\"{}\"",
                               self.epg_channel_id.as_ref().map_or("", |o| o.as_ref()),
                               self.name, self.group);

        if !ignore_logo {
            match resource_url {
                None => {
                    to_m3u_non_empty_fields!(self, line, (logo, "tvg-logo"), (logo_small, "tvg-logo-small"););
                }
                Some(res_url) => {
                    to_m3u_resource_non_empty_fields!(self, res_url, line, (logo, "tvg-logo"), (logo_small, "tvg-logo-small"););
                }
            }
        }

        to_m3u_non_empty_fields!(self, line,
            (chno, "tvg-chno"),
            (parent_code, "parent-code"),
            (audio_track, "audio-track"),
            (time_shift, "timeshift"),
            (rec, "tvg-rec"););

        format!("{},{}\n{}", line, self.title, stream_url)
    }
}

impl PlaylistEntry for M3uPlaylistItem {
    #[inline]
    fn get_virtual_id(&self) -> u32 {
        self.virtual_id
    }

    fn get_provider_id(&self) -> Option<u32> {
        match self.provider_id.parse::<u32>() {
            Ok(id) => Some(id),
            Err(_) => match extract_id_from_url(&self.url) {
                Some(id) => match id.parse::<u32>() {
                    Ok(newid) => {
                        Some(newid)
                    }
                    Err(_) => None,
                },
                None => None,
            }
        }
    }
    #[inline]
    fn get_category_id(&self) -> Option<u32> {
        None
    }
    #[inline]
    fn get_provider_url(&self) -> Rc<String> {
        Rc::clone(&self.url)
    }
}

macro_rules! generate_field_accessor_impl_for_m3u_playlist_item {
    ($($prop:ident),*;) => {
        impl FieldGetAccessor for M3uPlaylistItem {
            fn get_field(&self, field: &str) -> Option<Rc<String>> {
                match field {
                    $(
                        stringify!($prop) => Some(self.$prop.clone()),
                    )*
                     "epg_channel_id" | "epg_id" => self.epg_channel_id.clone(),
                    _ => None,
                }
            }
        }
    }
}

generate_field_accessor_impl_for_m3u_playlist_item!(provider_id, name, chno, logo, logo_small, group, title, parent_code, audio_track, time_shift, rec, url;);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtreamPlaylistItem {
    pub virtual_id: u32,
    pub provider_id: u32,
    pub name: Rc<String>,
    pub logo: Rc<String>,
    pub logo_small: Rc<String>,
    pub group: Rc<String>,
    pub title: Rc<String>,
    pub parent_code: Rc<String>,
    pub rec: Rc<String>,
    pub url: Rc<String>,
    pub epg_channel_id: Option<Rc<String>>,
    pub xtream_cluster: XtreamCluster,
    pub additional_properties: Option<String>,
    pub item_type: PlaylistItemType,
    pub category_id: u32,
    pub input_id: u16,
}

impl XtreamPlaylistItem {
    pub fn to_doc(&self, url: &str, options: &XtreamMappingOptions, user: &ProxyUserCredentials) -> Value {
        xtream_playlistitem_to_document(self, url, options, user)
    }
}

impl PlaylistEntry for XtreamPlaylistItem {
    #[inline]
    fn get_virtual_id(&self) -> u32 {
        self.virtual_id
    }
    #[inline]
    fn get_provider_id(&self) -> Option<u32> {
        Some(self.provider_id)
    }
    #[inline]
    fn get_category_id(&self) -> Option<u32> {
        None
    }
    #[inline]
    fn get_provider_url(&self) -> Rc<String> {
        Rc::clone(&self.url)
    }
}

fn get_backdrop_path_value(field: &str, value: Option<&Value>) -> Option<Rc<String>> {
    match value {
        Some(Value::String(url)) => Some(Rc::new(url.clone())),
        Some(Value::Array(values)) => {
            match values.as_slice() {
                [single] => Some(Rc::new(single.to_string())),
                multiple if !multiple.is_empty() => {
                    if let Some(index) = field.rfind('_') {
                        if let Ok(bd_index) = field[index + 1..].parse::<usize>() {
                            if let Some(selected) = multiple.get(bd_index) {
                                return Some(Rc::new(selected.to_string()));
                            }
                        }
                    }
                    Some(Rc::new(multiple[0].to_string()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}


macro_rules! generate_field_accessor_impl_for_xtream_playlist_item {
    ($($prop:ident),*;) => {
        impl FieldGetAccessor for XtreamPlaylistItem {
            fn get_field(&self, field: &str) -> Option<Rc<String>> {
                match field {
                    $(
                        stringify!($prop) => Some(self.$prop.clone()),
                    )*
                     "epg_channel_id" | "epg_id" => self.epg_channel_id.clone(),
                    _ => {
                       if field.starts_with("bakdrop_path") || field == "cover" {
                            let props = self.additional_properties.as_ref().and_then(|add_props| serde_json::from_str::<Map<String, Value>>(add_props).ok());
                            return match props {
                                Some(doc) => {
                                    return if field == "cover" {
                                       doc.get("cover").and_then(|value| value.as_str().map(|s| Rc::new(s.to_string())))
                                    } else {
                                       get_backdrop_path_value(field, doc.get("backdrop_path"))
                                    }
                                }
                                _=> None,
                            }
                        }
                        None
                    },
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub header: RefCell<PlaylistItemHeader>,
}

generate_field_accessor_impl_for_xtream_playlist_item!(name, logo, logo_small, group,title, parent_code, rec, url;);

impl PlaylistItem {
    pub fn to_m3u(&self) -> M3uPlaylistItem {
        let header = self.header.borrow();
        M3uPlaylistItem {
            virtual_id: header.virtual_id,
            provider_id: Rc::clone(&header.id),
            name: Rc::clone(if header.item_type == PlaylistItemType::Series { &header.title } else { &header.name }),
            chno: Rc::clone(&header.chno),
            logo: Rc::clone(&header.logo),
            logo_small: Rc::clone(&header.logo_small),
            group: Rc::clone(&header.group),
            title: Rc::clone(&header.title),
            parent_code: Rc::clone(&header.parent_code),
            audio_track: Rc::clone(&header.audio_track),
            time_shift: Rc::clone(&header.time_shift),
            rec: Rc::clone(&header.rec),
            url: Rc::clone(&header.url),
            epg_channel_id: header.epg_channel_id.clone(),
            input_id: header.input_id,
            item_type: header.item_type,
        }
    }

    pub fn to_xtream(&self) -> XtreamPlaylistItem {
        let header = self.header.borrow();
        let provider_id = header.id.parse::<u32>().unwrap_or_default();
        XtreamPlaylistItem {
            virtual_id: header.virtual_id,
            provider_id,
            name: Rc::clone(if header.item_type == PlaylistItemType::Series { &header.title } else { &header.name }),
            logo: Rc::clone(&header.logo),
            logo_small: Rc::clone(&header.logo_small),
            group: Rc::clone(&header.group),
            title: Rc::clone(&header.title),
            parent_code: Rc::clone(&header.parent_code),
            rec: Rc::clone(&header.rec),
            url: Rc::clone(&header.url),
            epg_channel_id: header.epg_channel_id.clone(),
            xtream_cluster: header.xtream_cluster,
            additional_properties: header.additional_properties.as_ref().and_then(|props| serde_json::to_string(props).ok()),
            item_type: header.item_type,
            category_id: header.category_id,
            input_id: header.input_id,
        }
    }
}

impl PlaylistEntry for PlaylistItem {
    #[inline]
    fn get_virtual_id(&self) -> u32 {
        self.header.borrow().virtual_id
    }

    fn get_provider_id(&self) -> Option<u32> {
        let header = self.header.borrow();
        match header.id.parse::<u32>() {
            Ok(id) => Some(id),
            Err(_) => match extract_id_from_url(&header.url) {
                Some(id) => match id.parse::<u32>() {
                    Ok(newid) => {
                        Some(newid)
                    }
                    Err(_) => None,
                },
                None => None,
            }
        }
    }

    #[inline]
    fn get_category_id(&self) -> Option<u32> {
        None
    }
    #[inline]
    fn get_provider_url(&self) -> Rc<String> {
        Rc::clone(&self.header.borrow().url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistGroup {
    pub id: u32,
    pub title: Rc<String>,
    pub channels: Vec<PlaylistItem>,
    #[serde(skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
}

impl PlaylistGroup {
    #[inline]
    pub fn on_load(&mut self) {
        self.channels.iter().for_each(|pl| pl.header.borrow_mut().gen_uuid());
    }
}