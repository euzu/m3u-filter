use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream_factory::STREAM_QUEUE_SIZE;
use crate::api::model::stream_error::StreamError;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::sanitize_sensitive_info;
use parking_lot::{FairMutex};
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

const MIN_STREAM_QUEUE_SIZE: usize = 128;

///
/// Wraps a `ReceiverStream` as Stream<Item = Result<Bytes, `StreamError`>>
///
struct BroadcastStreamWrapper {
    stream: BroadcastStream<Bytes>,
}

impl Stream for BroadcastStreamWrapper {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(bytes))),
            Poll::Ready(Some(Err(_))) | Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}

fn convert_stream(stream: BroadcastStream<Bytes>) -> BoxStream<'static, Result<Bytes, StreamError>> {
    Box::pin(BroadcastStreamWrapper { stream })
}

/// Represents the state of a shared provider URL.
///
/// - `headers`: The initial connection headers used during the setup of the shared stream.
struct SharedStreamState {
    headers: Vec<(String, String)>,
    sender: tokio::sync::broadcast::Sender<Bytes>,
}

impl SharedStreamState {
    fn new(headers: Vec<(String, String)>,
           buf_size: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(buf_size);
        Self {
            headers,
            sender,
        }
    }

    fn subscribe(&self) -> BoxStream<'static, Result<Bytes, StreamError>> {
        let rx = self.sender.subscribe();
        convert_stream(BroadcastStream::new(rx)).boxed()
        // .map_err(StreamError::ReceiverError).boxed()
    }

    fn broadcast<S, E>(&self, stream_url: &str, bytes_stream: S, shared_streams: Arc<SharedStreamManager>)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
    {
        let mut source_stream = Box::pin(bytes_stream);
        let streaming_url = stream_url.to_string();
        let sender = self.sender.clone();
        // Spawn a task to forward items from the source stream to the broadcast channel
        actix_rt::spawn(async move {
            let sleep_duration = Duration::from_millis(20);
            loop  {
                match source_stream.next().await {
                    Some(Ok(data)) => {
                        if sender.receiver_count() == 0 {
                            debug_if_enabled!("No active subscribers. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
                            break;
                        }
                        let _ = sender.send(data);
                    }
                    None | Some(Err(_)) => {
                        break;
                    }
                }
                actix_web::rt::time::sleep(sleep_duration).await;
            }
            shared_streams.unregister(&streaming_url);
        });
    }
}

type SharedStreamRegister = Arc<FairMutex<HashMap<String, SharedStreamState>>>;

pub struct SharedStreamManager {
    shared_streams: SharedStreamRegister,
}

impl SharedStreamManager {
    pub(crate) fn new() -> Self {
        Self {
            shared_streams: Arc::new(FairMutex::new(HashMap::new())),
        }
    }

    pub fn get_shared_state_headers(&self, stream_url: &str) -> Option<Vec<(String, String)>> {
        self.shared_streams.lock().get(stream_url).map(|s| s.headers.clone())
    }

    fn unregister(&self, stream_url: &str) {
        self.shared_streams.lock().remove(stream_url);
    }

    pub(crate) fn subscribe<S, E>(
        app_state: &AppState,
        stream_url: &str,
        bytes_stream: S,
        use_buffer: bool,
        headers: Vec<(String, String)>, )
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
    {
        let buf_size = if use_buffer { STREAM_QUEUE_SIZE } else { MIN_STREAM_QUEUE_SIZE };
        let shared_state = SharedStreamState::new(headers, buf_size);
        shared_state.broadcast(stream_url, bytes_stream, Arc::clone(&app_state.shared_stream_manager));
        app_state
            .shared_stream_manager
            .shared_streams
            .lock()
            .insert(stream_url.to_string(), shared_state);
        debug_if_enabled!("Created shared provider stream {}", sanitize_sensitive_info(stream_url));
    }

    /// Creates a broadcast notify stream for the given URL if a shared stream exists.
    pub fn subscribe_shared_stream(
        app_state: &AppState,
        stream_url: &str,
    ) -> Option<BoxStream<'static, Result<Bytes, StreamError>>> {
        if let Some(shared_stream) =  app_state
            .shared_stream_manager
            .shared_streams
            .lock()
            .get(stream_url) {
            debug_if_enabled!("Responding existing shared client stream {}", sanitize_sensitive_info(stream_url));
            Some(shared_stream.subscribe())
        } else {
            None
        }
    }
}