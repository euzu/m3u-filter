use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use async_std::prelude::{Stream, StreamExt};
use tokio::sync::broadcast;
use crate::api::model::app_state::AppState;
use crate::debug_if_enabled;
use crate::utils::request_utils::mask_sensitive_info;

pub struct SharedStream {
    pub data_stream: Arc<tokio::sync::broadcast::Sender<Bytes>>,
}

impl SharedStream {

    /// Creates a shared stream and stores it in the shared state.
    pub(crate) async fn register<S, E>(
        app_state: &AppState,
        stream_url: &str,
        bytes_stream: S,
    ) where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
    {
        // Create a broadcast channel for the shared stream
        let (tx, _) = broadcast::channel(100);
        let sender = Arc::new(tx);

        // Insert the shared stream into the shared state
        app_state
            .shared_streams
            .lock()
            .await
            .insert(
                stream_url.to_string(),
                SharedStream {
                    data_stream: sender.clone(),
                },
            );

        let shared_streams_map = Arc::clone(&app_state.shared_streams);
        let mut source_stream = Box::pin(bytes_stream);
        let streaming_url = stream_url.to_string();
        // Spawn a task to forward items from the source stream to the broadcast channel
        actix_rt::spawn(async move {
            while let Some(item) = source_stream.next().await {
                if let Ok(data) = item {
                    if sender.receiver_count() > 0 {
                        // if let Err(err) = sender.send(data) {
                        //     debug!("{err}")
                        // }
                        if sender.send(data).is_err() {
                            // ignore
                        }
                        actix_web::rt::time::sleep(Duration::from_millis(20)).await;
                    } else {
                        debug_if_enabled!("No active subscribers. Closing stream {}", mask_sensitive_info(&streaming_url));
                        // Cleanup for removing unused shared streams
                        let mut shared_streams = shared_streams_map.lock().await;
                        shared_streams.remove(&streaming_url);
                        return;
                    }
                }
            }
        });
    }
}
