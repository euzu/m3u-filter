use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

pub struct SharedStream {
    pub data_stream: Arc<tokio::sync::broadcast::Sender<Bytes>>,
    pub header: HashMap<String, Vec<u8>>,
}

