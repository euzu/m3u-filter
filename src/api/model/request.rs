use serde::de::{self, Deserializer, Unexpected};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, Serialize, Default)]
#[repr(u8)]
pub enum PlaylistRequestType {
    #[default]
    Input = 1,
    Target = 2,
    Xtream = 3,
    M3U = 4
}

impl<'de> serde::Deserialize<'de> for PlaylistRequestType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            1 => Ok(PlaylistRequestType::Input),
            2 => Ok(PlaylistRequestType::Target),
            3 => Ok(PlaylistRequestType::Xtream),
            4 => Ok(PlaylistRequestType::M3U),
            _ => Err(de::Error::invalid_value(
                Unexpected::Unsigned(value.into()),
                &"expected 1 (Input), 2 (Target), 3 (Xtream) or 4 (M3U)",
            )),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlaylistRequest {
    pub rtype: PlaylistRequestType,
    pub username: Option<String>,
    pub password: Option<String>,
    pub url: Option<String>,
    #[serde(alias="sourceId")]
    pub source_id: Option<u16>,
    #[serde(alias="sourceName")]
    pub source_name: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct UserApiRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub series_id: String,
    #[serde(default)]
    pub vod_id: String,
    #[serde(default)]
    pub stream_id: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub limit: String,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default, alias = "type")]
    pub content_type: String,
}