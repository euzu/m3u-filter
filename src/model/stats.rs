use std::fmt::Display;
use serde::{Serialize, Serializer};
use crate::model::config::InputType;

pub fn format_elapsed_time(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds} secs")
    } else {
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        format!("{minutes}:{seconds} mins")
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn serialize_elapsed_time<S>(secs: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let formatted = format_elapsed_time(*secs);
    serializer.serialize_str(&formatted)
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistStats {
    #[serde(rename = "groups")]
    pub group_count: usize,
    #[serde(rename = "channels")]
    pub channel_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputStats {
    pub name: String,
    #[serde(rename = "type")]
    pub input_type: InputType,
    #[serde(rename = "errors")]
    pub error_count: usize,
    #[serde(rename = "raw")]
    pub raw_stats: PlaylistStats,
    #[serde(rename = "processed")]
    pub processed_stats: PlaylistStats,
    #[serde(rename = "took", serialize_with = "serialize_elapsed_time")]
    pub secs_took: u64,
}

impl Display for InputStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string(&self) {
            Ok(json_str) => write!(f, "{json_str}"),
            Err(_) => Err(std::fmt::Error),
        }
    }
}