use tokio::sync::{Mutex};
use std::sync::Arc;
use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::download::DownloadQueue;
use crate::api::model::hls_cache::HlsCache;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::model::config::{Config};
use crate::model::hdhomerun_config::HdHomeRunDeviceConfig;
use crate::tools::lru_cache::LRUResourceCache;


#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http_client: Arc<reqwest::Client>,
    pub downloads: Arc<DownloadQueue>,
    pub cache: Arc<Option<Mutex<LRUResourceCache>>>,
    pub hls_cache: Arc<HlsCache>,
    pub shared_stream_manager: Arc<SharedStreamManager>,
    pub active_users: Arc<ActiveUserManager>,
    pub active_provider: Arc<ActiveProviderManager>,
}

impl AppState {
    pub async fn get_active_connections_for_user(&self, username: &str) -> u32 {
        self.active_users.user_connections(username).await
    }
}

#[derive(Clone)]
pub struct HdHomerunAppState {
    pub app_state: Arc<AppState>,
    pub device: Arc<HdHomeRunDeviceConfig>,
}
