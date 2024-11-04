#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Healthcheck {
    pub status: String,
    pub version: String,
    pub time: String,
}