use crate::model::config::InputType;

#[derive(Debug, Clone)]
pub(crate) struct PlaylistStats {
    pub group_count: usize,
    pub channel_count: usize,
}

impl ToString for PlaylistStats {
    fn to_string(&self) -> String {
        format!("{{\"groups\": {}, \"channels\": {}}}", self.group_count, self.channel_count)
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

impl ToString for InputStats {
    fn to_string(&self) -> String {
        format!("{{\"name\": {}, \"type\": {}, \"errors\": {}, \"raw\": {}, \"processed\": {}}}",
                self.name, self.input_type.to_string(), self.error_count,
                self.raw_stats.to_string(), self.processed_stats.to_string())
    }
}