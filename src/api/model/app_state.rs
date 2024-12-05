use std::collections::HashMap;
use std::sync::Arc;
use async_std::sync::{Mutex};
use crate::api::model::download::DownloadQueue;
use crate::api::model::shared_stream::SharedStream;
use crate::model::config::{Config, ProcessTargets};

pub struct AppState {
    pub config: Arc<Config>,
    pub targets: Arc<ProcessTargets>,
    pub downloads: Arc<DownloadQueue>,
    pub shared_streams: Arc<Mutex<HashMap<String, SharedStream>>>,
}
