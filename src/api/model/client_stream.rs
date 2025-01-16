use crate::api::model::provider_stream_factory::ResponseStream;
use async_std::prelude::Stream;
use bytes::Bytes;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;
use log::debug;
use crate::api::model::stream_error::StreamError;
use crate::utils::request_utils::mask_sensitive_info;

/// This stream counts the send bytes for reconnecting to the actual position and
/// sets the `close_signal`  if the client drops the connection.
pub(in crate::api::model) struct ClientStream {
    inner: ResponseStream,
    close_signal: Arc<AtomicBool>,
    total_bytes: Arc<Option<AtomicUsize>>,
    url: String,
}

impl ClientStream {
    pub(crate) fn new(inner: ResponseStream, close_signal: Arc<AtomicBool>, total_bytes: Arc<Option<AtomicUsize>>, url: &str) -> Self {
        Self { inner, close_signal, total_bytes, url: url.to_string() }
    }
}

impl Stream for ClientStream
{
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if let Some(counter) = self.total_bytes.as_ref() {
                    counter.fetch_add(bytes.len(), Ordering::Relaxed);
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

impl Drop for ClientStream {
    fn drop(&mut self) {
        debug!("Client disconnected {}", mask_sensitive_info(&self.url));
        self.close_signal.store(false, Ordering::Relaxed);
    }
}