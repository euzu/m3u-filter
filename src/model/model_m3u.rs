use std::cell::RefCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::model::config::ConfigOptions;

// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html


#[derive(Debug, Clone)]
pub(crate) enum XtreamCluster {
    LIVE = 1,
    VIDEO = 2,
    SERIES = 3
}

pub(crate) fn default_stream_cluster() -> XtreamCluster { XtreamCluster::LIVE }

pub(crate) trait FieldAccessor {
    fn get_field(&self, field: &str) -> Option<&String>;
    fn set_field(&mut self, field: &str, value: &str) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItemHeader {
    pub id: String,
    pub name: String,
    pub logo: String,
    pub logo_small: String,
    pub group: String,
    pub title: String,
    pub parent_code: String,
    pub audio_track: String,
    pub time_shift: String,
    pub rec: String,
    pub source: String,
    #[serde(default = "default_stream_cluster", skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
    #[serde(skip_serializing, skip_deserializing)]
    pub additional_properties: Option<Vec<(String, Value)>>,
}

impl FieldAccessor for PlaylistItemHeader {
    fn get_field(&self, field: &str) -> Option<&String> {
        match field {
            "id" => Some(&self.id),
            "name" => Some(&self.name),
            "logo" => Some(&self.logo),
            "logo_small" => Some(&self.logo_small),
            "group" => Some(&self.group),
            "title" => Some(&self.title),
            "parent_code" => Some(&self.parent_code),
            "audio_track" => Some(&self.audio_track),
            "time_shift" => Some(&self.time_shift),
            "rec" => Some(&self.rec),
            "source" => Some(&self.source),
            _ => None
        }
    }

    fn set_field(&mut self, field: &str, value: &str) -> bool {
        let val = String::from(value);
        match field {
            "id" => { self.id = val; true},
            "name" =>  { self.name = val; true }
            "logo" =>  { self.logo = val; true }
            "logo_small" =>  { self.logo_small = val; true }
            "group" =>  { self.group = val; true }
            "title" =>  { self.title = val; true }
            "parent_code" =>  { self.parent_code = val; true }
            "audio_track" =>  { self.audio_track = val; true }
            "time_shift" =>  { self.time_shift = val; true }
            "rec" =>  { self.rec = val; true }
            "source" =>  { self.source = val; true }
            _ => false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistItem {
    pub header: RefCell<PlaylistItemHeader>,
    pub url: String,
}

impl PlaylistItem {
    pub fn to_m3u(&self, options: &Option<ConfigOptions>) -> String {
        let header = self.header.borrow();
        let ignore_logo = options.as_ref().map_or(false, |o| o.ignore_logo);
        let mut line = format!("#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\" group-title=\"{}\"", header.id, header.name, header.group);

        // line = format!("{} tvg-chno=\"{}\"", line, header.chno);

        if !ignore_logo {
            if !header.logo.is_empty() {
                line = format!("{} tvg-logo=\"{}\"", line, header.logo);
            }
            if !header.logo_small.is_empty() {
                line = format!("{} tvg-logo-small=\"{}\"", line, header.logo_small);
            }
        }
        if !header.parent_code.is_empty() {
            line = format!("{} parent-code=\"{}\"", line, header.parent_code);
        }
        if !header.audio_track.is_empty() {
            line = format!("{} audio-track=\"{}\"", line, header.audio_track);
        }
        if !header.time_shift.is_empty() {
            line = format!("{} timeschift=\"{}\"", line, header.time_shift);
        }
        if !header.rec.is_empty() {
            line = format!("{} rec=\"{}\"", line, header.rec);
        }
        format!("{},{}\n{}", line, header.title, self.url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistGroup {
    pub id: i32,
    pub title: String,
    pub channels: Vec<PlaylistItem>,
    #[serde(default = "default_stream_cluster", skip_serializing, skip_deserializing)]
    pub xtream_cluster: XtreamCluster,
}
