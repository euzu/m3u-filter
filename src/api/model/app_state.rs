use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use async_std::sync::{Mutex};
use crate::api::model::download::DownloadQueue;
use crate::api::model::streams::shared_stream_manager::SharedStreamManager;
use crate::model::config::{Config};
use crate::tools::lru_cache::LRUResourceCache;

pub struct AppState {
    pub config: Arc<Config>,
    pub downloads: Arc<DownloadQueue>,
    pub shared_stream_manager: Arc<Mutex<SharedStreamManager>>,
    pub active_clients: Arc<AtomicUsize>,
    pub http_client: Arc<reqwest::Client>,
    pub cache: Arc<Option<Mutex<LRUResourceCache>>>
}
