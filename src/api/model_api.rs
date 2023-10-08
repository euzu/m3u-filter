use actix_web::web;
use serde::{Deserialize, Serialize};
use crate::model::config::{Config, ProcessTargets};

pub(crate) struct AppState {
    pub config: Config,
    pub targets: ProcessTargets,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct PlaylistRequest {
   pub url: String,
}

impl From<web::Json<PlaylistRequest>> for PlaylistRequest {
    fn from(req: web::Json<PlaylistRequest>) -> Self {
        PlaylistRequest {
            url: String::from(&req.url),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ServerConfig {
    pub sources: Vec<String>,
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

