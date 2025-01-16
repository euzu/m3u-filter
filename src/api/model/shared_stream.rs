use crate::api::model::app_state::AppState;
use crate::api::model::provider_stream_factory::STREAM_QUEUE_SIZE;
use crate::debug_if_enabled;
use crate::utils::request_utils::mask_sensitive_info;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use log::error;
use crate::api::model::broadcast_stream::BroadcastStream;
use crate::api::model::stream_error::StreamError;

const MIN_STREAM_QUEUE_SIZE: usize = 1024;

pub struct SharedStream {
    // pub sender: flume::Sender<Bytes>,
    pub receiver: async_broadcast::Receiver<Bytes>,
}

impl SharedStream {

    pub(crate)fn get_receiver(&self) -> BoxStream<'static, Result<Bytes, StreamError>> {
         BroadcastStream::new(self.receiver.clone()).boxed()
    }

    /// Creates a shared stream and stores it in the shared state.
    pub(crate) async fn register<S, E>(
        app_state: &AppState,
        stream_url: &str,
        bytes_stream: S,
        use_buffer: bool,
    ) where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
    {
        // Create a broadcast channel for the shared stream
        let (tx, rx) = async_broadcast::broadcast(if use_buffer { STREAM_QUEUE_SIZE } else { MIN_STREAM_QUEUE_SIZE });
        let sender = tx.clone();

        // Insert the shared stream into the shared state
        app_state
            .shared_streams
            .lock()
            .await
            .insert(
                stream_url.to_string(),
                SharedStream {
                    // sender: sender.clone(),
                    receiver: rx
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
                        match sender.broadcast(data).await {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Broadcast channel send {err}");
                            }
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
