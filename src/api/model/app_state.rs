use std::sync::{Arc};
use parking_lot::{Mutex, RwLock};
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::download::DownloadQueue;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::model::config::{Config};
use crate::tools::lru_cache::LRUResourceCache;

pub struct AppState {
    pub config: Arc<Config>,
    pub downloads: Arc<DownloadQueue>,
    pub shared_stream_manager: Arc<Mutex<SharedStreamManager>>,
    pub http_client: Arc<reqwest::Client>,
    pub cache: Arc<Option<Mutex<LRUResourceCache>>>,
    pub active_users: Arc<RwLock<ActiveUserManager>>,
}

impl AppState {
    pub fn get_active_connections_for_user(&self, username: &str) -> u32 {
        self.active_users.read().user_connections(username)
    }
}
