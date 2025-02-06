use crate::api::model::streams::provider_stream_factory::ResponseStream;
use bytes::Bytes;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Poll};
use futures::{Stream};
use log::info;
use crate::api::model::stream_error::StreamError;

pub(in crate::api) struct ActiveClientStream {
    inner: ResponseStream,
    active_clients: Arc<AtomicUsize>,
    log_active_clients: bool,
}

impl ActiveClientStream {
    pub(crate) fn new(inner: ResponseStream, active_clients: Arc<AtomicUsize>, log_active_clients: bool) -> Self {
        let client_count = active_clients.fetch_add(1, Ordering::Relaxed) + 1;
        if log_active_clients {
            info!("Active clients: {client_count}");
        }
        Self { inner, active_clients, log_active_clients }
    }
}
impl Stream for ActiveClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
            Pin::as_mut(&mut self.inner).poll_next(cx)
    }
}


impl Drop for ActiveClientStream {
    fn drop(&mut self) {
        let client_count = self.active_clients.fetch_sub(1, Ordering::Relaxed) -1;
        if self.log_active_clients {
            info!("Active clients: {client_count}");
        }
    }
}