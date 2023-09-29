use actix_web::web;
use serde::{Deserialize, Serialize};
use crate::config::{Config, ProcessTargets};

pub(crate) struct AppState {
    pub config: Config,
    pub targets: ProcessTargets,
    pub verbose: bool,
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