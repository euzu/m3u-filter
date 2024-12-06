use bytes::Bytes;
use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::stream::Stream;
use futures::StreamExt;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::{
    error::Error as StdError,
    pin::Pin,
    task::{Context, Poll},
};
use std::collections::HashMap;

pub struct NotifyStream<S> {
    stream: S,
    tx: Option<Sender<()>>,
}

impl<S, T, E> NotifyStream<S>
where
    S: Stream<Item=Result<T, E>>,
{
    pub fn new(stream: S) -> (Self, Receiver<()>) {
        let (send, recv) = channel();
        (NotifyStream {
            stream,
            tx: Some(send),
        }, recv)
    }
}

impl<S, T, E> Stream for NotifyStream<S>
where
    S: Stream<Item=Result<T, E>> + Unpin,
    E: Into<Box<dyn StdError>> + 'static,
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let result = self.stream.poll_next_unpin(cx);
        let mut connection_closed = false;
        match &result {
            Poll::Ready(val) => {
                if val.is_none() {
                    connection_closed = true;
                }
            }
            Poll::Pending => {}
        }

        if connection_closed {
            if let Some(send) = self.tx.take() {
                // Ignore errors as they just mean the receiver was dropped.
                let _ = send.send(());
            }
        }

        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

pub struct SharedStream {
    pub data_stream: Arc<tokio::sync::broadcast::Sender<Bytes>>,
    pub client_count: AtomicU32,
    pub header: HashMap<String, Vec<u8>>,
}

