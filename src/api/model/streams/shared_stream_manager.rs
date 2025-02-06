use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream_factory::STREAM_QUEUE_SIZE;
use crate::api::model::stream_error::StreamError;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::sanitize_sensitive_info;
use parking_lot::FairMutex;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Sender};
use tokio_stream::wrappers::ReceiverStream;

use std::pin::Pin;
use std::task::{Context, Poll};

const MIN_STREAM_QUEUE_SIZE: usize = 1024;

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
/// - `subscribers`: A list of clients that have subscribed to the shared stream.
struct SharedStreamState {
    headers: Vec<(String, String)>,
    buf_size: usize,
    subscribers: Arc<FairMutex<Vec<Sender<Bytes>>>>,
}

impl SharedStreamState {
    fn new(headers: Vec<(String, String)>,
           buf_size: usize) -> Self {
        Self {
            headers,
            buf_size,
            subscribers: Arc::new(FairMutex::new(Vec::new())),
        }
    }

    fn subscribe(&self) -> BoxStream<'static, Result<Bytes, StreamError>> {
        let (tx, rx) = mpsc::channel(self.buf_size);
        self.subscribers.lock().push(tx);
        convert_stream(ReceiverStream::new(rx).boxed())
    }

    fn broadcast<S, E>(&self, stream_url: &str, bytes_stream: S, shared_streams: Arc<FairMutex<SharedStreamManager>>)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static,
    {
        let mut source_stream = Box::pin(bytes_stream);
        let subscriber = Arc::clone(&self.subscribers);
        let streaming_url = stream_url.to_string();
        // Spawn a task to forward items from the source stream to the broadcast channel
        actix_rt::spawn(async move {
            while let Some(item) = source_stream.next().await {
                if let Ok(data) = item {
                    let mut subs = subscriber.lock();
                    if subs.len() > 0 {
                        (*subs).retain(|sender| {
                            match sender.try_send(data.clone()) {
                                Err(TrySendError::Closed(_)) => false,
                                Ok(()) | Err(_) => true,
                                // Err(TrySendError::Full(_)) => false, // Drop slow consumers
                            }
                        });
                    } else {
                        debug_if_enabled!("No active subscribers. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
                        // Cleanup for removing unused shared streams
                        shared_streams.lock().unregister(&streaming_url);
                        return;
                    }
                }
            }
            shared_streams.lock().unregister(&streaming_url);
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
            .lock()
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
            .lock()
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