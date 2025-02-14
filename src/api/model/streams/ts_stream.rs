use futures::stream::{Stream};
use std::pin::Pin;
use bytes::Bytes;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::provider_stream_factory::ResponseStream;

const TS_PACKET_SIZE: usize = 188;

fn is_valid_mpeg_ts_package(packet_data: &[u8]) -> bool {
    packet_data[0] == 0x47 // Standard Sync Byte für MPEG-TS-Pakete
}

// Der Stream-Wrapper für BoxedStream
pub struct TsStream {
    inner: ResponseStream,
    buffer: Vec<u8>,
    found_valid_packet: bool, // Flag for first valid package
}

impl TsStream {
    pub fn new(inner: ResponseStream) -> Self {
        TsStream {
            inner,
            buffer: Vec::new(),
            found_valid_packet: false,
        }
    }
}

impl Stream for TsStream {
    type Item = Result<bytes::Bytes, StreamError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            // Wenn wir weniger als ein Paket im Buffer haben, lesen wir mehr Daten
            if this.buffer.len() < TS_PACKET_SIZE {
                match Pin::new(&mut this.inner).poll_next(cx) {
                    std::task::Poll::Ready(Some(Ok(chunk))) => {
                        this.buffer.extend_from_slice(&chunk);
                    }
                    std::task::Poll::Ready(Some(Err(e))) => return std::task::Poll::Ready(Some(Err(e))),
                    std::task::Poll::Ready(None) => return std::task::Poll::Ready(None), // Ende des Streams
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            }

            // Wenn wir genug Daten haben, um ein Paket zu extrahieren
            if this.buffer.len() >= TS_PACKET_SIZE {
                let packet_data = &this.buffer[0..TS_PACKET_SIZE]; // Nimm das erste Paket
                if is_valid_mpeg_ts_package(packet_data) {
                    this.found_valid_packet = true; // Wir haben nun ein gültiges Paket gefunden
                    // Entferne das verarbeitete Paket aus dem Buffer
                    this.buffer = this.buffer[TS_PACKET_SIZE..].to_vec();
                    return std::task::Poll::Ready(Some(Ok(Bytes::from_static(packet_data))));
                } else if  !this.found_valid_packet {
                    // Fehlerhaftes Paket überspringen, entferne es aus dem Buffer
                    // Wenn wir das erste gültige Paket nicht gefunden haben, überspringen wir ungültige Pakete
                    this.buffer = this.buffer[TS_PACKET_SIZE..].to_vec();
                    continue;
                }
            }
        }
    }
}