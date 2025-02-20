#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PlaylistCategories {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vod: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<String>>,
}