use std::pin::Pin;
use std::task::{Context, Poll};
use async_broadcast::{Receiver, RecvError};
use bytes::Bytes;
use futures::{Stream};
use log::error;
use crate::api::model::stream_error::StreamError;

#[derive(Debug)]
pub struct BroadcastStream {
    inner: Receiver<bytes::Bytes>
}

impl BroadcastStream {
    pub fn new(mut recv: Receiver<Bytes>) -> Self {
        recv.set_overflow(true);
        Self { inner: recv }
    }
}

impl Stream for BroadcastStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.inner.is_closed() {
            return Poll::Ready(None);
        }
        match Pin::new(&mut self.inner).poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(item)) => {
                match item {
                    Ok(data) => Poll::Ready(Some(Ok(data))),
                    Err(err) => {
                        error!("Shared stream client error {err}");
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(None) => Poll::Ready(Some(Err(StreamError::ReceiverClosed))),

        }
    }
}
