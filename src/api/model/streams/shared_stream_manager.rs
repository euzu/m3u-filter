use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream_factory::STREAM_QUEUE_SIZE;
use crate::api::model::stream_error::StreamError;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::sanitize_sensitive_info;
use parking_lot::{RwLock};
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Sender};

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use log::{trace};
use tokio::sync::{mpsc};
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::wrappers::ReceiverStream;

///
/// Wraps a `ReceiverStream` as Stream<Item = Result<Bytes, `StreamError`>>
///
struct ReceiverStreamWrapper<S> {
    stream: S,
}

impl<S> Stream for ReceiverStreamWrapper<S>
where
    S: Stream<Item=Bytes> + Unpin,
{
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(Some(bytes)) => Poll::Ready(Some(Ok(bytes))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn convert_stream(stream: BoxStream<Bytes>) -> BoxStream<Result<Bytes, StreamError>> {
    Box::pin(ReceiverStreamWrapper { stream }.boxed())
}


/// Represents the state of a shared provider URL.
///
/// - `headers`: The initial connection headers used during the setup of the shared stream.
struct SharedStreamState {
    headers: Vec<(String, String)>,
    buf_size: usize,
    subscribers: Arc<RwLock<Vec<Sender<Bytes>>>>,
}

impl SharedStreamState {
    fn new(headers: Vec<(String, String)>,
           buf_size: usize) -> Self {
        Self {
            headers,
            buf_size,
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn subscribe(&self) -> BoxStream<'static, Result<Bytes, StreamError>> {
        let (tx, rx) = mpsc::channel(self.buf_size);
        self.subscribers.write().push(tx);
        convert_stream(ReceiverStream::new(rx).boxed())
    }

    fn broadcast<S, E>(&self, stream_url: &str, bytes_stream: S, shared_streams: Arc<SharedStreamManager>)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
        E: std::fmt::Debug
    {
        let buf_size = self.buf_size;
        let sleep_duration = Duration::from_millis(10);
        let mut source_stream = Box::pin(bytes_stream);
        let subscriber = Arc::clone(&self.subscribers);
        let streaming_url = stream_url.to_string();

        //Spawn a task to forward items from the source stream to the broadcast channel
        actix_rt::spawn(async move {
            while let Some(item) = source_stream.next().await {
                if let Ok(data) = item {

                    if subscriber.read().is_empty() {
                        debug_if_enabled!("No active subscribers. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
                        break;
                    }

                    while !subscriber.read().iter().any(|sender| sender.capacity() == buf_size) {
                        actix_web::rt::time::sleep(sleep_duration).await;
                    }

                    let mut subs = subscriber.write();
                    if subs.is_empty() {
                        debug_if_enabled!("No active subscribers. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
                        // Cleanup for removing unused shared streams
                        shared_streams.unregister(&streaming_url);
                        break;
                    }
                    (*subs).retain(|sender| {
                        match sender.try_send(data.clone()) {
                            Ok(()) => true,
                            Err(TrySendError::Closed(_)) => false,
                            Err(err) => {
                                trace!("broadcast send error {err}");
                                true
                            }
                        }
                    });
                }
                actix_web::rt::time::sleep(sleep_duration).await;
            }
            debug_if_enabled!("Shared stream exhausted. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
            shared_streams.unregister(&streaming_url);
        });
    }
}

type SharedStreamRegister = RwLock<HashMap<String, SharedStreamState>>;

pub struct SharedStreamManager {
    shared_streams: SharedStreamRegister,
}

impl SharedStreamManager {
    pub(crate) fn new() -> Self {
        Self {
            shared_streams: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_shared_state_headers(&self, stream_url: &str) -> Option<Vec<(String, String)>> {
        self.shared_streams.read().get(stream_url).map(|s| s.headers.clone())
    }

    fn unregister(&self, stream_url: &str) {
        self.shared_streams.write().remove(stream_url);
    }

    fn subscribe_stream(&self, stream_url: &str) -> Option<BoxStream<'static, Result<Bytes, StreamError>>> {
        let stream_data = self.shared_streams.read().get(stream_url)?.subscribe();
        Some(stream_data)
    }

    fn register(&self, stream_url: &str, shared_state: SharedStreamState) {
        self.shared_streams.write().insert(stream_url.to_string(), shared_state);
    }

    pub(crate) fn subscribe<S, E>(
        app_state: &AppState,
        stream_url: &str,
        bytes_stream: S,
        headers: Vec<(String, String)>,
        buffer_size: usize,)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
        E: std::fmt::Debug
    {
        let buf_size = std::cmp::max(buffer_size, STREAM_QUEUE_SIZE);
        let shared_state = SharedStreamState::new(headers, buf_size);
        shared_state.broadcast(stream_url, bytes_stream, Arc::clone(&app_state.shared_stream_manager));
        app_state.shared_stream_manager.register(stream_url, shared_state);
        debug_if_enabled!("Created shared provider stream {}", sanitize_sensitive_info(stream_url));
    }

    /// Creates a broadcast notify stream for the given URL if a shared stream exists.
    pub fn subscribe_shared_stream(
        app_state: &AppState,
        stream_url: &str,
    ) -> Option<BoxStream<'static, Result<Bytes, StreamError>>> {
        debug_if_enabled!("Responding existing shared client stream {}", sanitize_sensitive_info(stream_url));
        app_state.shared_stream_manager.subscribe_stream(stream_url)
    }
}