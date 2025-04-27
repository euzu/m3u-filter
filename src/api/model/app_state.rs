use tokio::sync::{Mutex};
use std::sync::Arc;
use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::download::DownloadQueue;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::model::api_proxy::UserConnectionPermission;
use crate::model::config::{Config};
use crate::model::hdhomerun_config::HdHomeRunDeviceConfig;
use crate::tools::lru_cache::LRUResourceCache;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http_client: Arc<reqwest::Client>,
    pub downloads: Arc<DownloadQueue>,
    pub cache: Arc<Option<Mutex<LRUResourceCache>>>,
    pub shared_stream_manager: Arc<SharedStreamManager>,
    pub active_users: Arc<ActiveUserManager>,
    pub active_provider: Arc<ActiveProviderManager>,
}

impl AppState {
    /// Returns the number of active connections for the specified user.
    ///
    /// # Examples
    ///
    /// ```
    /// let count = app_state.get_active_connections_for_user("alice").await;
    /// assert_eq!(count, 0); // if user "alice" has no active connections
    /// ```
    pub async fn get_active_connections_for_user(&self, username: &str) -> u32 {
        self.active_users.user_connections(username).await
    }

    /// Determines whether a user is permitted to establish a new connection based on their current active connections and the specified maximum.
    ///
    /// # Examples
    ///
    /// ```
    /// let permission = app_state.get_connection_permission("alice", 3).await;
    /// match permission {
    ///     UserConnectionPermission::Allowed => println!("Connection permitted"),
    ///     UserConnectionPermission::Denied => println!("Connection denied"),
    /// }
    /// ```
    pub async fn get_connection_permission(&self, username: &str, max_connections: u32) -> UserConnectionPermission {
    pub async fn get_connection_permission(&self, username: &str, max_connections: u32) -> UserConnectionPermission {
        self.active_users.connection_permission(username, max_connections).await
    }
}

#[derive(Clone)]
pub struct HdHomerunAppState {
    pub app_state: Arc<AppState>,
    pub device: Arc<HdHomeRunDeviceConfig>,
}
