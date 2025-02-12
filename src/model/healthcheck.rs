#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
}