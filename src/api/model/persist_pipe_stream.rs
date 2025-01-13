use async_std::sync::Mutex;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use bytes::Bytes;
use reqwest::Error;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_stream::Stream;
pub struct PersistPipeStream<S, W> {
    inner: S,
    completed: bool,
    writer: Arc<Mutex<W>>,
    size: AtomicUsize,
    callback: Arc<Box<dyn Fn(usize)>>,
}

impl<S, W> PersistPipeStream<S, W>
where
    S: Stream + Unpin,
    W: AsyncWrite + Unpin + 'static,
{
    pub fn new(inner: S, writer: Arc<Mutex<W>>, callback: Arc<Box<dyn Fn(usize)>>) -> Self {
        Self {
            inner,
            completed: false,
            writer,
            size: AtomicUsize::new(0),
            callback,
        }
    }

    fn on_complete(&mut self) {
        if !self.completed {
            self.completed = true;
            let writer = self.writer.clone();
            let size = self.size.load(Ordering::Relaxed);
            let callback = Arc::clone(&self.callback);
            actix_rt::spawn(async move {
                let mut guard = writer.lock().await;
                if (*guard).flush().await.is_ok() {
                    callback(size);
                }
            });
        }
    }

    fn on_data(&mut self, data: &Result<Bytes, Error>) {
        if let Ok(bytes) = data {
            self.size.fetch_add(bytes.len(), Ordering::Relaxed);
            let writer = self.writer.clone();
            let bytes_to_write = bytes.clone();
            actix_rt::spawn(async move {
                let mut guard = writer.lock().await;
                if let Err(e) = (*guard).write_all(&bytes_to_write).await {
                    eprintln!("Error writing to resource file: {e}");
                }
            });
        }
    }
}

impl<S, W> Stream for PersistPipeStream<S, W>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    W: AsyncWrite + Unpin + 'static,
{
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                this.on_complete();
                Poll::Ready(None)
            }
            Poll::Ready(Some(item)) => {
                this.on_data(&item);
                Poll::Ready(Some(item))
            }
        }
    }
}