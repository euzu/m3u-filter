use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use actix_web::web;
use serde::{Deserialize, Serialize};
use crate::model::config::{Config, ConfigOptions, ConfigRename, ConfigSort, InputType, ProcessTargets, TargetOutput, VideoConfig};
use crate::model::model_config::{default_as_empty_str, ProcessingOrder};

pub(crate) struct AppState {
    pub config: Arc<Config>,
    pub targets: Arc<ProcessTargets>,
    pub downloads: Arc<Mutex<HashMap<String, u64>>>
}

#[derive(Serialize)]
pub(crate) struct XtreamUserInfo {
    pub active_cons: u16, // 0
    pub allowed_output_formats: Vec<String>, //["ts"],
    pub auth: u16, // 0 | 1
    pub created_at: i64, //1623429679,
    pub exp_date: i64,  //1628755200,
    pub is_trial: u16, // 0 | 1
    pub max_connections: u16,
    pub message: String,
    pub password: String,
    pub username: String,
    pub status: String, // "Active"
}

#[derive(Serialize)]
pub(crate) struct XtreamServerInfo {
    pub url: String,
    pub port:u16,
    pub https_port:u16,
    pub server_protocol: String, // http, https
    pub rtmp_port:u16,
    pub timezone:String,
    pub timestamp_now:i64,
    pub time_now: String, //"2021-06-28 17:07:37"
}

#[derive(Serialize)]
pub(crate) struct XtreamAuthorizationResponse {
    pub user_info: XtreamUserInfo,
    pub server_info: XtreamServerInfo,
}


#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct UserApiRequest {
    #[serde(default = "default_as_empty_str")]
    pub username: String,
    #[serde(default = "default_as_empty_str")]
    pub password: String,
    #[serde(default = "default_as_empty_str")]
    pub token: String,
    #[serde(default = "default_as_empty_str")]
    pub action: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerInputConfig {
    pub id: u16,
    pub input_type: InputType,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub persist: Option<String>,
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerTargetConfig {
    pub id: u16,
    pub enabled: bool,
    pub name: String,
    pub options: Option<ConfigOptions>,
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type")]
    pub output: Vec<TargetOutput>,
    pub rename: Option<Vec<ConfigRename>>,
    pub mapping: Option<Vec<String>>,
    pub processing_order: ProcessingOrder,
    pub watch: Option<Vec<String>>,
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerSourceConfig {
    pub inputs: Vec<ServerInputConfig>,
    pub targets: Vec<ServerTargetConfig>
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerConfig {
    pub sources: Vec<ServerSourceConfig>,
    pub video: Option<VideoConfig>,
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct PlaylistRequest {
    pub url: Option<String>,
    pub input_id: Option<u16>,
}

impl From<web::Json<PlaylistRequest>> for PlaylistRequest {
    fn from(req: web::Json<PlaylistRequest>) -> Self {
        req.clone()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct FileDownloadRequest {
    pub url: String,
    pub filename: String,
}

impl From<web::Json<FileDownloadRequest>> for FileDownloadRequest {
    fn from(req: web::Json<FileDownloadRequest>) -> Self {
        req.clone()
    }
}

