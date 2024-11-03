use std::fmt::Display;

use crate::model::config::InputType;

#[derive(Debug, Clone)]
pub(crate) struct PlaylistStats {
    pub group_count: usize,
    pub channel_count: usize,
}

impl Display for PlaylistStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_args!("{{\"groups\": {}, \"channels\": {}}}", self.group_count, self.channel_count))
    }
}

pub(crate) fn format_elapsed_time(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds} secs")
    } else {
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        format!("{minutes}:{seconds} mins")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InputStats {
    pub name: String,
    pub input_type: InputType,
    pub error_count: usize,
    pub raw_stats: PlaylistStats,
    pub processed_stats: PlaylistStats,
    pub secs_took: u64,
}

impl Display for InputStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let elapsed = format_elapsed_time(self.secs_took);
        let str = format!("{{\"name\": {}, \"type\": {}, \"errors\": {}, \"raw\": {}, \"processed\": {}, \"took\": {elapsed}}}",
                          self.name, self.input_type, self.error_count,
                          self.raw_stats, self.processed_stats);
        write!(f, "{str}")
    }
}