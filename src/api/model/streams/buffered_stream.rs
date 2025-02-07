use crate::api::model::streams::provider_stream_factory::ResponseStream;
use futures::{stream::Stream, task::{Context, Poll}, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
};
use std::time::Duration;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use crate::api::model::stream_error::StreamError;
use crate::tools::atomic_once_flag::AtomicOnceFlag;

pub(in crate::api::model) struct BufferedStream {
    stream: ReceiverStream<Result<bytes::Bytes, StreamError>>,
}

impl BufferedStream {
    pub fn new(stream: ResponseStream, buffer_size: usize, client_close_signal: Arc<AtomicOnceFlag>, _url: &str) -> Self {
        let (tx, rx) = channel(buffer_size);
        actix_rt::spawn(async move {
            let mut stream = stream;
            let sleep_duration= Duration::from_millis(100);
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        // this is for backpressure, we fill the buffer and wait for the receiver
                        if let Ok(permit) = tx.reserve().await {
                            permit.send(Ok(chunk));
                        } else {
                            // receiver closed.
                            client_close_signal.notify();
                            break;
                        }
                    }
                    Some(Err(_err)) => {
                        actix_web::rt::time::sleep(sleep_duration).await;
                    }
                    None => {
                        break
                    }
                }
            }
        });

        Self {
            stream: ReceiverStream::new(rx)
        }
    }
}

impl Stream for BufferedStream {
    type Item = Result<bytes::Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}
