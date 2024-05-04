use std::fmt::Display;
use crate::model::config::InputType;

#[derive(Debug, Clone)]
pub(crate) struct PlaylistStats {
    pub group_count: usize,
    pub channel_count: usize,
}

impl Display for PlaylistStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,  "{}", format_args!("{{\"groups\": {}, \"channels\": {}}}", self.group_count, self.channel_count))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InputStats {
    pub name: String,
    pub input_type: InputType,
    pub error_count: usize,
    pub raw_stats: PlaylistStats,
    pub processed_stats: PlaylistStats,
}

impl Display for InputStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = format!("{{\"name\": {}, \"type\": {}, \"errors\": {}, \"raw\": {}, \"processed\": {}}}",
                          self.name, self.input_type, self.error_count,
                          self.raw_stats, self.processed_stats);
        write!(f, "{}", str)
    }
}