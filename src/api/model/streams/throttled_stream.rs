use bytes::Bytes;
use futures::{Stream};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use std::future::Future;
use tokio::time::{sleep, Sleep};
use crate::api::model::stream_error::StreamError;

pub struct ThrottledStream<S> {
    inner: S,
    rate_bytes_per_sec: f64,
    next_delay: Option<Pin<Box<Sleep>>>,
}

impl<S> ThrottledStream<S> {
    pub fn new(inner: S, throttle_kibps: usize) -> Self {
        assert!(throttle_kibps > 0, "Rate must be greater than 0");
        let rate_bytes_per_sec = (throttle_kibps as f64) * 1024.0;
        Self {
            inner,
            rate_bytes_per_sec,
            next_delay: None,
        }
    }
}

impl<S> Stream for ThrottledStream<S>
where
    S: Stream<Item = Result<Bytes, StreamError>> + Unpin,
{
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        // Check if there's an active delay
        if let Some(mut delay) = this.next_delay.take() {
            match delay.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    // Delay completed, proceed to poll inner stream
                }
                Poll::Pending => {
                    // Re-insert the delay and return Pending
                    this.next_delay = Some(delay);
                    return Poll::Pending;
                }
            }
        }

        // Poll the inner stream
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let len = bytes.len() as f64;
                let delay_duration = Duration::from_secs_f64(len / this.rate_bytes_per_sec);

                // Schedule the next delay
                this.next_delay = Some(Box::pin(sleep(delay_duration)));

                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                // Emit error without delaying
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}