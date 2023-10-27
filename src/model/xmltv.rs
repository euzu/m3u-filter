use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use crate::model::model_config::default_as_empty_str;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Text {
    #[serde(rename = "$value")]
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Channel {
    #[serde(rename = "@id", default="default_as_empty_str")]
    id: String,
    #[serde(rename = "display-name")]
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<Icon>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Icon {
    #[serde(rename = "@src", default="default_as_empty_str")]
    src: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Programme {
    #[serde(rename = "@start", default="default_as_empty_str")]
    start: String,
    #[serde(rename = "@stop", default="default_as_empty_str")]
    stop: String,
    #[serde(rename = "@channel", default="default_as_empty_str")]
    channel: String,
    title: Text,
    desc: Text,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none", rename="sub-title")]
    sub_title: Option<Text>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "tv")]
pub(crate) struct TVGuide {
    #[serde(rename = "channel")]
    pub channels: Vec<Channel>,
    #[serde(rename = "programme")]
    pub programs: Vec<Programme>,
    #[serde(skip_serializing_if = "Option::is_none", rename="@date")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias="generator-info-url", rename="@source-info-url")]
    pub source_info_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias="generator-info-name", rename="@source-info-name")]
    pub source_info_name: Option<String>,
}

impl TVGuide {
    pub(crate) fn clear(&mut self, channel_ids: &HashSet<String>) {
        if !channel_ids.is_empty() {
            self.channels = self.channels.drain(..).filter(|chan| channel_ids.contains(&chan.id)).collect();
        }
    }
}

impl TVGuide {
    pub fn prepare(&mut self) {
        self.channels = self.channels.drain(..).filter(|chan| !chan.id.trim().is_empty()).collect();
    }
}
