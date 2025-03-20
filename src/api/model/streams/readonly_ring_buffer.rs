use bytes::{Bytes, BytesMut};
use std::sync::Arc;

const CHUNK_SIZE: usize = 8192;

#[derive(Clone)]
pub struct ReadonlyRingBuffer {
    buffer: Arc<Vec<u8>>,
    current_pos: usize,
}

impl ReadonlyRingBuffer {
    pub fn new(buffer: Arc<Vec<u8>>) -> Self {
        Self {
            buffer,
            current_pos: 0,
        }
    }

    pub fn next_chunk(&mut self) -> Option<Bytes> {
        let buffer_len = self.buffer.len();
        let mut current_pos = self.current_pos;

        // Return None if the buffer is empty or all data is consumed.
        if buffer_len == 0 || current_pos >= buffer_len {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(CHUNK_SIZE);
        let mut remaining = CHUNK_SIZE;

        while remaining > 0 {
            // Calculate the start and end positions of the chunk to read
            let start = current_pos;
            let end = std::cmp::min(current_pos + remaining, buffer_len);

            // Read the chunk and extend to `bytes`
            bytes.extend_from_slice(&self.buffer[start..end]);

            // Update remaining bytes to read and the current position
            let chunk_len = end - start;
            remaining -= chunk_len;
            current_pos = (current_pos + chunk_len) % buffer_len;

            // If the chunk end wraps around to the beginning of the buffer, handle the wraparound
            if remaining > 0 && end == buffer_len {
                current_pos = 0;
            }
        }

        // Update the buffer's position for the next read
        self.current_pos = current_pos;

        // Return the chunk
        Some(bytes.freeze())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::api::model::streams::readonly_ring_buffer::ReadonlyRingBuffer;

    #[test]
    fn test_buffer() {
        let buffer: Vec<u8> = (0..20000).map(|x| (x % 256) as u8).collect();
        let mut ring_buffer = ReadonlyRingBuffer::new(Arc::new(buffer.clone()));

        let mut index:usize = 0;
        while let Some(chunk) = ring_buffer.next_chunk() {
            for &byte in chunk.iter() {
                let expected_value = buffer[index % buffer.len()];
                assert_eq!(byte, expected_value, "Wrong value {byte} != {expected_value} at index {index} detected!");
                index+=1;
            }
            if index > 400000 {
                break;
            }
        }
    }
}