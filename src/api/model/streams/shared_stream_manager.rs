use crate::api::model::app_state::AppState;
use crate::api::model::streams::provider_stream_factory::STREAM_QUEUE_SIZE;
use crate::api::model::stream_error::StreamError;
use crate::utils::debug_if_enabled;
use crate::utils::network::request::sanitize_sensitive_info;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::RwLock;
use tokio::sync::mpsc::{Sender};

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use log::{trace};
use tokio::sync::{mpsc};
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::wrappers::ReceiverStream;
use crate::api::model::stream::BoxedProviderStream;

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


// impl<S> Drop for ReceiverStreamWrapper<S>
// {
//     fn drop(&mut self) {
//         println!("receiver_dropped");
//     }
// }

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

    async fn subscribe(&self) -> BoxedProviderStream {
        let (tx, rx) = mpsc::channel(self.buf_size);
        self.subscribers.write().await.push(tx);
        convert_stream(ReceiverStream::new(rx).boxed())
    }

    fn broadcast<S, E>(&self, stream_url: &str, bytes_stream: S, shared_streams: Arc<SharedStreamManager>)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static + std::marker::Send,
        E: std::fmt::Debug + std::marker::Send
    {
        let mut source_stream = Box::pin(bytes_stream);
        let subscriber = Arc::clone(&self.subscribers);
        let streaming_url = stream_url.to_string();

        let mut tick = tokio::time::interval(Duration::from_millis(5));

        //Spawn a task to forward items from the source stream to the broadcast channel
        tokio::spawn(async move {
            while let Some(item) = source_stream.next().await {
                if let Ok(data) = item {
                    if subscriber.read().await.is_empty() {
                        debug_if_enabled!("No active subscribers. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
                        // Cleanup for removing unused shared streams
                        shared_streams.unregister(&streaming_url).await;
                        break;
                    }

                    let start_time = Instant::now();
                    loop {
                        if subscriber.read().await.iter().any(|sender| sender.capacity() > 0) {
                            break;
                        }
                        if start_time.elapsed().as_secs() > 5 {
                            break;
                        }
                        tick.tick().await;
                    }

                    let mut subs =  subscriber.write().await;
                    // TODO use drain_filter when stable
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
                tick.tick().await;
            }
            debug_if_enabled!("Shared stream exhausted. Closing shared provider stream {}", sanitize_sensitive_info(&streaming_url));
            shared_streams.unregister(&streaming_url).await;
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

    pub async fn get_shared_state_headers(&self, stream_url: &str) -> Option<Vec<(String, String)>> {
        self.shared_streams.read().await.get(stream_url).map(|s| s.headers.clone())
    }

    async fn unregister(&self, stream_url: &str) {
        let _ = self.shared_streams.write().await.remove(stream_url);
    }

    async fn subscribe_stream(&self, stream_url: &str) -> Option<BoxedProviderStream> {
        let stream_data = self.shared_streams.read().await.get(stream_url)?.subscribe().await;
        Some(stream_data)
    }

    async fn register(&self, stream_url: &str, shared_state: SharedStreamState) {
        let _= self.shared_streams.write().await.insert(stream_url.to_string(), shared_state);
    }

    pub(crate) async fn subscribe<S, E>(
        app_state: &AppState,
        stream_url: &str,
        bytes_stream: S,
        headers: Vec<(String, String)>,
        buffer_size: usize,)
    where
        S: Stream<Item=Result<Bytes, E>> + Unpin + 'static + std::marker::Send,
        E: std::fmt::Debug + std::marker::Send
    {
        let buf_size = std::cmp::max(buffer_size, STREAM_QUEUE_SIZE);
        let shared_state = SharedStreamState::new(headers, buf_size);
        shared_state.broadcast(stream_url, bytes_stream, Arc::clone(&app_state.shared_stream_manager));
        app_state.shared_stream_manager.register(stream_url, shared_state).await;
        debug_if_enabled!("Created shared provider stream {}", sanitize_sensitive_info(stream_url));
    }

    /// Creates a broadcast notify stream for the given URL if a shared stream exists.
    pub async fn subscribe_shared_stream(
        app_state: &AppState,
        stream_url: &str,
    ) -> Option<BoxedProviderStream> {
        debug_if_enabled!("Responding existing shared client stream {}", sanitize_sensitive_info(stream_url));
        app_state.shared_stream_manager.subscribe_stream(stream_url).await
    }
}