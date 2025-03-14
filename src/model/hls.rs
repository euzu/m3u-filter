use std::collections::HashMap;
use tokio::time::Instant;
use crate::model::config::TargetType;

#[derive(Clone)]
pub struct HlsEntry {
    pub ts: Instant,
    pub token: String,
    pub target_type: TargetType,
    pub input_name: String,
    pub virtual_id: u32,
    pub chunk: u32,
    pub chunks: HashMap<u32, String>,
}

impl HlsEntry {
    pub fn get_chunk_url(&self, chunk: u32) -> Option<&String> {
       self.chunks.get(&chunk)
    }
}