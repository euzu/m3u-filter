use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::config::{ConfigInput, ConfigOptions};
use crate::model::xmltv::TVGuide;

// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html

#[derive(Debug, Clone)]
pub(crate) struct FetchedPlaylist<'a> {
    pub input: &'a ConfigInput,
    pub playlist: Vec<PlaylistGroup>,
    pub epg: Option<TVGuide>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum XtreamCluster {
    Live = 1,
    Video = 2,
    Series = 3,
}

pub(crate) fn default_stream_cluster() -> XtreamCluster { XtreamCluster::Live }

pub(crate) trait FieldAccessor {
    fn get_field(&self, field: &str) -> Option<Rc<String>>;
    fn set_field(&mut self, field: &str, value: &str) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItemHeader {
    pub id: Rc<String>,
    pub name: Rc<String>,
    pub logo: Rc<String>,
    pub logo_small: Rc<String>,
    pub group: Rc<String>,
    pub title: Rc<String>,
    pub parent_code: Rc<String>,
    pub audio_track: Rc<String>,
    pub time_shift: Rc<String>,
    pub rec: Rc<String>,
    pub source: Rc<String>,
    // this is the source content not the url
    pub url: Rc<String>,
    pub epg_channel_id: Option<Rc<String>>,
    #[serde(default = "default_stream_cluster", skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
    #[serde(skip_serializing, skip_deserializing)]
    pub additional_properties: Option<Vec<(String, Value)>>,
}

macro_rules! update_fields {
    ($self:expr, $field:expr, $($prop:ident),*; $val:expr) => {
        match $field {
            $(
                stringify!($prop) => {
                    $self.$prop = Rc::new($val);
                    true
                },
            )*
            _ => false,
        }
    };
}

macro_rules! get_fields {
    ($self:expr, $field:expr, $($prop:ident),*;) => {
        match $field {
            $(
                stringify!($prop) => Some($self.$prop.clone()),
            )*
            _ => None,
        }
    };
}

impl FieldAccessor for PlaylistItemHeader {
    fn get_field(&self, field: &str) -> Option<Rc<String>> {
        get_fields!(self, field, id, name, logo, logo_small, group, title, parent_code, audio_track, time_shift, rec, source, url;)
    }

    fn set_field(&mut self, field: &str, value: &str) -> bool {
        let val = String::from(value);
        update_fields!(self, field, id, name, logo, logo_small, group, title, parent_code, audio_track, time_shift, rec, source, url; val)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItem {
    pub header: RefCell<PlaylistItemHeader>,
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


impl PlaylistItem {
    pub fn to_m3u(&self, options: &Option<ConfigOptions>) -> String {
        let header = self.header.borrow();
        let ignore_logo = options.as_ref().map_or(false, |o| o.ignore_logo);
        let mut line = format!("#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\" group-title=\"{}\"",
                               header.epg_channel_id.as_ref().map_or("", |o| o.as_ref()),
                               header.name, header.group);

        // line = format!("{} tvg-chno=\"{}\"", line, header.chno);

        if !ignore_logo {
            to_m3u_non_empty_fields!(header, line, (logo, "tvg-logo"), (logo_small, "tvg-logo-small"););
        }

        to_m3u_non_empty_fields!(header, line,
            (parent_code, "parent-code"),
            (audio_track, "audio-track"),
            (time_shift, "timeshift"),
            (rec, "tvg-rec"););

        format!("{},{}\n{}", line, header.title, header.url)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistGroup {
    pub id: i32,
    pub title: Rc<String>,
    pub channels: Vec<PlaylistItem>,
    #[serde(default = "default_stream_cluster", skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
}
