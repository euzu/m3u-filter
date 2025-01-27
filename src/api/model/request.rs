use actix_web::web;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlaylistRequest {
    pub url: Option<String>,
    pub input_name: Option<String>,
}

impl From<web::Json<Self>> for PlaylistRequest {
    fn from(req: web::Json<Self>) -> Self {
        req.clone()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct UserApiRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub series_id: String,
    #[serde(default)]
    pub vod_id: String,
    #[serde(default)]
    pub stream_id: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub limit: String,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub duration: String,
}