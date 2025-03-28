use tokio::sync::{Mutex};
use std::sync::Arc;
use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::download::DownloadQueue;
use crate::api::model::hls_cache::HlsCache;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::model::api_proxy::UserConnectionPermission;
use crate::model::config::{Config};
use crate::model::hdhomerun_config::HdHomeRunDeviceConfig;
use crate::tools::lru_cache::LRUResourceCache;
use crate::utils::default_utils::default_grace_period_millis;

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

    pub async fn get_connection_permission(&self, username: &str, max_connections: u32) -> UserConnectionPermission {
        let grace_period_millis = self.config.reverse_proxy.as_ref().and_then(|r| r.stream.as_ref()).map(|s| s.grace_period_millis).unwrap_or_else(default_grace_period_millis);
        self.active_users.connection_permission(username, max_connections, grace_period_millis > 0).await
    }
}

#[derive(Clone)]
pub struct HdHomerunAppState {
    pub app_state: Arc<AppState>,
    pub device: Arc<HdHomeRunDeviceConfig>,
}
