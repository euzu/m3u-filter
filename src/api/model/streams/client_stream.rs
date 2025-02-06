use crate::api::model::streams::provider_stream_factory::ResponseStream;
use bytes::Bytes;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Poll};
use futures::{Stream};
use crate::api::model::stream_error::StreamError;
use crate::utils::trace_if_enabled;
use crate::tools::atomic_once_flag::AtomicOnceFlag;
use crate::utils::network::request::sanitize_sensitive_info;

/// This stream counts the send bytes for reconnecting to the actual position and
/// sets the `close_signal`  if the client drops the connection.
pub(in crate::api::model) struct ClientStream {
    inner: ResponseStream,
    close_signal: Arc<AtomicOnceFlag>,
    total_bytes: Arc<Option<AtomicUsize>>,
    url: String,
}

impl ClientStream {
    pub(crate) fn new(inner: ResponseStream, close_signal: Arc<AtomicOnceFlag>, total_bytes: Arc<Option<AtomicUsize>>, url: &str) -> Self {
        Self { inner, close_signal, total_bytes, url: url.to_string() }
    }
}
impl Stream for ClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::as_mut(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if bytes.is_empty() {
                        continue;
                    }

                    if let Some(counter) = self.total_bytes.as_ref() {
                        counter.fetch_add(bytes.len(), Ordering::Relaxed);
                    }

                    return Poll::Ready(Some(Ok(bytes)));
                }
                Poll::Ready(None) => {
                    self.close_signal.notify();
                    return Poll::Ready(None);
                }
                other => return other,
            }
        }
    }
}


impl Drop for ClientStream {
    fn drop(&mut self) {
        trace_if_enabled!("Client disconnected {}", sanitize_sensitive_info(&self.url));
        self.close_signal.notify();
    }
}