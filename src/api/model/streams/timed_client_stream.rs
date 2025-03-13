use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::provider_stream_factory::ResponseStream;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::Poll;
use std::time::{Duration, Instant};

pub struct TimedClientStream {
    inner: ResponseStream,
    duration: Duration,
    start_time: Instant,
}

impl TimedClientStream {
    pub(crate) fn new(inner: ResponseStream, duration: u32) -> Self {
        Self { inner, duration:  Duration::from_secs(u64::from(duration)) , start_time: Instant::now() }
    }
}
impl Stream for TimedClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>,cx: &mut std::task::Context<'_>,) -> Poll<Option<Self::Item>> {
        if  self.start_time.elapsed() > self.duration {
            return Poll::Ready(None);
        }
        Pin::as_mut(&mut self.inner).poll_next(cx)
    }
}