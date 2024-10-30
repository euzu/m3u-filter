use std::cell::RefCell;
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{ConfigInput, ConfigTarget};
use crate::model::xmltv::TVGuide;
use crate::model::xtream::{xtream_playlistitem_to_document, XtreamMappingOptions};
use crate::processing::m3u_parser::extract_id_from_url;
use crate::repository::storage::hash_string;
use crate::utils::default_utils::{default_as_false, default_as_zero_u16, default_as_zero_u32};

// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html

#[derive(Debug, Clone)]
pub(crate) struct FetchedPlaylist<'a> { // Contains playlist for one input
    pub input: &'a ConfigInput,
    pub playlistgroups: Vec<PlaylistGroup>,
    pub epg: Option<TVGuide>,
}

impl FetchedPlaylist<'_> {
    pub(crate) fn update_playlist(&mut self, plg: &PlaylistGroup) {
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
pub(crate) enum XtreamCluster {
    #[default]
    Live = 1,
    Video = 2,
    Series = 3,
}

impl XtreamCluster {

    pub fn as_str(&self) -> &str {
        match self {
            XtreamCluster::Live => "Live",
            XtreamCluster::Video => "Video",
            XtreamCluster::Series => "Series",
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
            PlaylistItemType::Live => Ok(XtreamCluster::Live),
            PlaylistItemType::Video => Ok(XtreamCluster::Video),
            PlaylistItemType::Series => Ok(XtreamCluster::Series),
            _ => Err(format!("Cant convert {item_type}")),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub(crate) enum PlaylistItemType {
    #[default]
    Live = 1,
    Video = 2,
    Series = 3, //  xtream series description
    SeriesInfo = 4, //  xtream series info fetched for series description
    SeriesEpisode = 5, // from SeriesInfo parsed episodes
    Catchup = 6,
}

impl From<XtreamCluster> for PlaylistItemType {
    fn from(xtream_cluster: XtreamCluster) -> Self {
        match xtream_cluster {
            XtreamCluster::Live => PlaylistItemType::Live,
            XtreamCluster::Video => PlaylistItemType::Video,
            XtreamCluster::Series => PlaylistItemType::SeriesInfo,
        }
    }
}

impl Display for PlaylistItemType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            PlaylistItemType::Live => "live",
            PlaylistItemType::Video => "video",
            PlaylistItemType::Series => "series",
            PlaylistItemType::SeriesInfo => "series-info",
            PlaylistItemType::SeriesEpisode => "series-episode",
            PlaylistItemType::Catchup => "catchup"
        })
    }
}

pub(crate) trait FieldAccessor {
    fn get_field(&self, field: &str) -> Option<Rc<String>>;
    fn set_field(&mut self, field: &str, value: &str) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PlaylistItemHeader {
    pub uuid: Rc<[u8; 32]>, // calculated
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
    #[serde(skip_serializing, skip_deserializing)]
    pub item_type: PlaylistItemType,
    #[serde(default = "default_as_false", skip_serializing, skip_deserializing)]
    pub series_fetched: bool, // only used for series_info
    #[serde(default = "default_as_zero_u32")]
    pub category_id: u32,
    #[serde(default = "default_as_zero_u16")]
    pub input_id: u16,
}

impl PlaylistItemHeader {
    pub(crate) fn gen_uuid(&mut self) {
        self.uuid = Rc::new(hash_string(&self.url));
    }
    pub(crate) fn get_uuid(&self) -> &Rc<[u8; 32]> {
        &self.uuid
    }

    pub(crate) fn get_provider_id(&mut self) -> Option<u32> {
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


macro_rules! generate_field_accessor_impl_for_playlist_item_header {
    ($($prop:ident),*;) => {
        impl FieldAccessor for PlaylistItemHeader {
            fn get_field(&self, field: &str) -> Option<Rc<String>> {
                match field {
                    $(
                        stringify!($prop) => Some(self.$prop.clone()),
                    )*
                    "epg_channel_id" | "epg_id" => self.epg_channel_id.clone(),
                    _ => None,
                }
            }

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
pub(crate) struct M3uPlaylistItem {
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
}

impl M3uPlaylistItem {
    pub fn to_m3u(&self, target: &ConfigTarget, url: Option<&str>) -> String {
        let options = target.options.as_ref();
        let ignore_logo = options.map_or(false, |o| o.ignore_logo);
        let mut line = format!("#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\" group-title=\"{}\"",
                               self.epg_channel_id.as_ref().map_or("", |o| o.as_ref()),
                               self.name, self.group);

        if !ignore_logo {
            to_m3u_non_empty_fields!(self, line, (logo, "tvg-logo"), (logo_small, "tvg-logo-small"););
        }

        to_m3u_non_empty_fields!(self, line,
            (chno, "tvg-chno"),
            (parent_code, "parent-code"),
            (audio_track, "audio-track"),
            (time_shift, "timeshift"),
            (rec, "tvg-rec"););

        format!("{},{}\n{}", line, self.title, if url.is_none() { self.url.as_str() } else { url.unwrap() })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct XtreamPlaylistItem {
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
    pub series_fetched: bool, // only used for series_info
    pub category_id: u32,
    pub input_id: u16,
}

impl XtreamPlaylistItem {
    pub fn to_doc(&self, options: &XtreamMappingOptions) -> Value {
        xtream_playlistitem_to_document(self, options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItem {
    pub header: RefCell<PlaylistItemHeader>,
}

impl PlaylistItem {
    pub fn to_m3u(&self) -> M3uPlaylistItem {
        let header = self.header.borrow();
        M3uPlaylistItem {
            virtual_id: header.virtual_id,
            provider_id: Rc::clone(&header.id),
            name: Rc::clone(&header.name),
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
        }
    }

    pub fn to_xtream(&self) -> Result<XtreamPlaylistItem, M3uFilterError> {
        let header = self.header.borrow();
        match header.id.parse::<u32>() {
            Ok(provider_id) => {
                Ok(XtreamPlaylistItem {
                    virtual_id: header.virtual_id,
                    provider_id,
                    name: Rc::clone(&header.name),
                    logo: Rc::clone(&header.logo),
                    logo_small: Rc::clone(&header.logo_small),
                    group: Rc::clone(&header.group),
                    title: Rc::clone(&header.title),
                    parent_code: Rc::clone(&header.parent_code),
                    rec: Rc::clone(&header.rec),
                    url: Rc::clone(&header.url),
                    epg_channel_id: header.epg_channel_id.clone(),
                    xtream_cluster: header.xtream_cluster,
                    additional_properties: match &header.additional_properties {
                        None => None,
                        Some(props) => match serde_json::to_string(props) {
                            Ok(val) => Some(val),
                            Err(_) => None
                        }
                    },
                    item_type: header.item_type,
                    series_fetched: header.series_fetched,
                    category_id: header.category_id,
                    input_id: header.input_id,
                })
            }
            Err(_) => {
                Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("cant parse provider stream id: {}", header.id)))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistGroup {
    pub id: u32,
    pub title: Rc<String>,
    pub channels: Vec<PlaylistItem>,
    #[serde(skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
}

impl PlaylistGroup {
    pub(crate) fn on_load(&mut self) {
        self.channels.iter().for_each(|pl| pl.header.borrow_mut().gen_uuid());
    }
}