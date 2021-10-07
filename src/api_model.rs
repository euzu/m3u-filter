use actix_web::web;
use serde::{Deserialize, Serialize};
use crate::config::Config;

pub(crate) struct AppState {
    pub config: Config,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct PlaylistRequest {
   pub(crate) url: String,
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