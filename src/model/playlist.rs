use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::config::{ConfigInput, ConfigTarget};
use crate::model::xmltv::TVGuide;
use crate::model::xtream::{xtream_playlistitem_to_document, XtreamMappingOptions};
use crate::utils::default_utils::{default_as_false, default_as_zero_u16, default_as_zero_u32, default_playlist_item_type, default_stream_cluster};

// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html

#[derive(Debug, Clone)]
pub(crate) struct FetchedPlaylist<'a> {
    pub input: &'a ConfigInput,
    pub playlist: Vec<PlaylistGroup>,
    pub epg: Option<TVGuide>,
}

impl FetchedPlaylist<'_> {
    pub(crate) fn update_playlist(&mut self, plg: &PlaylistGroup) {
        for grp in &mut self.playlist {
            if grp.id == plg.id {
                plg.channels.iter().for_each(|item| grp.channels.push(item.clone()));
                return;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum XtreamCluster {
    Live = 1,
    Video = 2,
    Series = 3,
}

impl Display for XtreamCluster {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            XtreamCluster::Live => "live",
            XtreamCluster::Video => "movie",
            XtreamCluster::Series => "series",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum PlaylistItemType {
    Live = 1,
    Movie = 2,
    Series = 3,
    SeriesInfo = 4,
}

pub(crate) trait FieldAccessor {
    fn get_field(&self, field: &str) -> Option<Rc<String>>;
    fn set_field(&mut self, field: &str, value: &str) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItemHeader {
    // stream_id is a custom field for processing
    pub stream_id: Rc<String>,
    pub id: Rc<String>,
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
    #[serde(default = "default_stream_cluster")]
    pub xtream_cluster: XtreamCluster,
    pub additional_properties: Option<Value>,
    #[serde(default = "default_playlist_item_type", skip_serializing, skip_deserializing)]
    pub item_type: PlaylistItemType,
    #[serde(default = "default_as_false", skip_serializing, skip_deserializing)]
    pub series_fetched: bool, // only used for series_info
    #[serde(default = "default_as_zero_u32")]
    pub category_id: u32,
    #[serde(default = "default_as_zero_u16")]
    pub input_id: u16,
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

// !! should be in sync with MAPPER_ATTRIBUTES  except stream_id !!
generate_field_accessor_impl_for_playlist_item_header!(id, stream_id, name, chno, logo, logo_small, group, title, parent_code, audio_track, time_shift, rec, url;);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct M3uPlaylistItem {
    pub stream_id: Rc<String>,
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
    pub stream_id: u32,
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
            stream_id: Rc::clone(&header.stream_id),
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

    pub fn to_xtream(&self) -> XtreamPlaylistItem {
        let header = self.header.borrow();
        XtreamPlaylistItem {
            stream_id: header.stream_id.parse::<u32>().unwrap(),
            provider_id: header.id.parse::<u32>().unwrap(),
            name: Rc::clone(&header.name),
            logo: Rc::clone(&header.logo),
            logo_small: Rc::clone(&header.logo_small),
            group: Rc::clone(&header.group),
            title: Rc::clone(&header.title),
            parent_code: Rc::clone(&header.parent_code),
            rec: Rc::clone(&header.rec),
            url: Rc::clone(&header.url),
            epg_channel_id: header.epg_channel_id.clone(),
            xtream_cluster: header.xtream_cluster.clone(),
            additional_properties: match &header.additional_properties {
                None => None,
                Some(props) => match serde_json::to_string(props) {
                    Ok(val) => Some(val),
                    Err(_) => None
                }
            },
            item_type: header.item_type.clone(),
            series_fetched: header.series_fetched,
            category_id: header.category_id,
            input_id: header.input_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistGroup {
    pub id: u32,
    pub title: Rc<String>,
    pub channels: Vec<PlaylistItem>,
    #[serde(default = "default_stream_cluster", skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
}

