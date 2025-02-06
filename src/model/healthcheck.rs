#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Healthcheck {
    pub status: String,
    pub version: String,
    pub time: String,
    pub mem: String,
    pub active_clients: usize,
    pub active_connections: usize,
}