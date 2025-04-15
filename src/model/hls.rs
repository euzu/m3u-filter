use std::collections::HashMap;
use tokio::time::Instant;
use crate::model::config::TargetType;

#[derive(Clone, Debug)]
pub struct HlsEntry {
    pub ts: Instant,
    pub token: u32,
    pub target_type: TargetType,
    pub input_id: u16,
    pub virtual_id: u32,
    pub chunk: u32,
    pub chunks: HashMap<u32, String>,
}

impl HlsEntry {
    pub fn get_chunk_url(&self, chunk: u32) -> Option<&String> {
       self.chunks.get(&chunk)
    }
}