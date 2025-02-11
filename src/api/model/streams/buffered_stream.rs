use crate::api::model::streams::provider_stream_factory::ResponseStream;
use futures::{stream::Stream, task::{Context, Poll}, StreamExt};
use std::{
    pin::Pin,
    sync::Arc,
};
use tokio::sync::mpsc::{channel, Sender};
use tokio_stream::wrappers::ReceiverStream;
use crate::api::model::stream_error::StreamError;
use crate::tools::atomic_once_flag::AtomicOnceFlag;

pub(in crate::api::model) struct BufferedStream {
    stream: ReceiverStream<Result<bytes::Bytes, StreamError>>,
}

impl BufferedStream {
    pub fn new(stream: ResponseStream, buffer_size: usize, client_close_signal: Arc<AtomicOnceFlag>, _url: &str) -> Self {
        let (tx, rx) = channel(buffer_size);
        actix_rt::spawn(Self::buffer_stream(tx, stream, client_close_signal));
        Self {
            stream: ReceiverStream::new(rx)
        }
    }

    async fn buffer_stream(
        tx: Sender<Result<bytes::Bytes, StreamError>>,
        mut stream: ResponseStream,
        client_close_signal: Arc<AtomicOnceFlag>,
    ) {
        loop {
            if !client_close_signal.is_active() {
                break;
            }
            match stream.next().await {
                Some(Ok(chunk)) => {
                    match tx.reserve().await {
                        Ok(permit) => permit.send(Ok(chunk)),
                        Err(_err) => {
                            // Receiver dropped, notify and exit
                            client_close_signal.notify();
                            break;
                        }
                    }
                }
                Some(Err(err)) => {
                    eprintln!("Buffered Stream Error: {err:?}");
                    // actix_web::rt::time::sleep(sleep_duration).await;
                    // Attempt to send error to client
                    if tx.send(Err(err)).await.is_err() {
                        client_close_signal.notify();
                    }
                    break;
                }
                None => break,
            }
        }
    }
}

impl Stream for BufferedStream {
    type Item = Result<bytes::Bytes, StreamError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().stream).poll_next(cx)
    }
}
