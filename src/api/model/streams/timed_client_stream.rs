use crate::api::model::stream_error::StreamError;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::Poll;
use std::time::{Duration, Instant};
use crate::api::model::stream::BoxedProviderStream;

pub struct TimeoutClientStream {
    inner: BoxedProviderStream,
    duration: Duration,
    start_time: Instant,
}

impl TimeoutClientStream {
    pub(crate) fn new(inner: BoxedProviderStream, duration: u32) -> Self {
        Self { inner, duration:  Duration::from_secs(u64::from(duration)) , start_time: Instant::now() }
    }
}
impl Stream for TimeoutClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>,cx: &mut std::task::Context<'_>,) -> Poll<Option<Self::Item>> {
        if  self.start_time.elapsed() > self.duration {
            return Poll::Ready(None);
        }
        Pin::as_mut(&mut self.inner).poll_next(cx)
    }
}