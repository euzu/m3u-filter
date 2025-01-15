use futures::{stream::Stream, task::{Context, Poll}, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
};
use std::sync::atomic::{AtomicBool, Ordering};
use log::debug;
use tokio::sync::mpsc::{channel};
use tokio_stream::wrappers::ReceiverStream;
use crate::api::model::provider_stream_factory::ResponseStream;
use crate::utils::request_utils::mask_sensitive_info;

pub struct BufferedStream
{
    stream: ReceiverStream<Result<bytes::Bytes, reqwest::Error>>,
}

impl BufferedStream {
    pub fn new(stream: ResponseStream, buffer_size: usize, client_close_signal: Arc<AtomicBool>, url: &str) -> Self {
        let (tx, rx) = channel(buffer_size);
        let masked_url = mask_sensitive_info(url);
        actix_rt::spawn(async move {
            let mut stream = stream;
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        // this is for backpressure, we fill the buffer and wait for the receiver
                        if let Ok(permit) = tx.reserve().await {
                            permit.send(Ok(chunk));
                        } else {
                            debug!("Client has disconnected from stream {masked_url}");
                            client_close_signal.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    Some(Err(_err)) => {
                    },
                    None => {
                         break
                    },
                }
            }
        });

        Self {
            stream: ReceiverStream::new(rx)
        }
    }
}

impl Stream for BufferedStream
{
    type Item = Result<bytes::Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}


// #[tokio::main]
// async fn main() {
//     use futures::stream;
//
//     // A slow source stream that produces items with a delay
//     let source_stream = stream::iter(vec![1, 2, 3, 4, 5, 6])
//         .throttle(tokio::time::Duration::from_millis(500));
//
//     // Create a buffered stream with a maximum buffer size of 3
//     let buffered_stream = BufferedStream::new(source_stream, 3);
//
//     // Consume the buffered stream
//     tokio::pin!(buffered_stream); // Pin the stream for asynchronous use
//
//     while let Some(item) = buffered_stream.next().await {
//         println!("Read: {}", item);
//         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // Simulate slower processing
//     }
// }
