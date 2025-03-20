use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::readonly_ring_buffer::ReadonlyRingBuffer;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};


#[derive(Clone)]
pub struct CustomVideoStream {
    buffer: ReadonlyRingBuffer,
}

impl CustomVideoStream {
    pub fn new(buffer: Arc<Vec<u8>>) -> Self {
        Self {
            buffer: ReadonlyRingBuffer::new(buffer)
        }
    }
}

impl Stream for CustomVideoStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.buffer.next_chunk() {
            None => {
                Poll::Ready(None)
            }
            Some(bytes) => {
                Poll::Ready(Some(Ok(bytes)))
            }
        }
    }
}
