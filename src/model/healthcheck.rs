use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Healthcheck {
    pub status: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time: Option<String>,
    pub server_time: String,
    pub memory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    pub active_clients: usize,
    pub active_connections: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider_connections: Option<HashMap<String, u16>>,
}