use crate::api::model::stream_error::StreamError;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

const CHUNK_SIZE: usize = 8192;


pub struct FreezeFrameStream {
    buffer: Arc<Vec<u8>>,
    buffer_len: usize,
    current_pos: usize, // Keep track of the current position in the buffer
}

impl FreezeFrameStream {
    pub fn new(_status: u16, buffer: Arc<Vec<u8>>) -> Self {
        let buffer_len = buffer.len();
        Self {
            buffer,
            buffer_len,
            current_pos: 0,
        }
    }
}

impl Stream for FreezeFrameStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {

        if self.buffer_len == 0 {
            return Poll::Ready(None); // If buffer is empty, return None (end of stream)
        }

        // Calculate the start and end positions for the chunk
        let start = self.current_pos;
        let end = (self.current_pos + CHUNK_SIZE).min(self.buffer_len);

        // Create a chunk from the buffer
        let chunk = self.buffer[start..end].to_vec();
        let bytes_chunk = Bytes::from(chunk);

        // Update the current position
        self.current_pos = if end == self.buffer_len {
            0 // Wrap around if we reach the end
        } else {
            end
        };

        Poll::Ready(Some(Ok(bytes_chunk)))
    }
}
