use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use bytes::Bytes;
use log::error;
use tokio_stream::Stream;
use crate::api::model::stream_error::StreamError;

/// `PersistPipeStream`
///
/// A stream wrapper that pipes data from an input stream to a writer while tracking
/// the total number of bytes processed. Upon completion, it triggers a user-provided
/// callback with the total size of the data written.
/// - `callback`: A user-provided function that is called with the total size of the processed data once the stream is completed.
///
/// # Stream Implementation
/// - Implements the `Stream` trait to poll the underlying stream for data.
/// - For each chunk of data:
///   - Writes it to the writer.
///   - Updates the size tracker.
/// - When the stream is exhausted:
///   - Calls `on_complete()` to finalize the operation and trigger the callback.
pub struct PersistPipeStream<S, W> {
    inner: S,
    completed: bool,
    writer: W,
    size: AtomicUsize,
    callback: Arc<dyn Fn(usize) + Send + Sync>,
}

impl<S, W> PersistPipeStream<S, W>
where
    S: Stream + Unpin,
    W: Write + Unpin + 'static,
{
    ///   - Creates a new `PersistPipeStream` instance.
    ///   - Arguments:
    ///     - `inner`: The input stream providing the data.
    ///     - `writer`: The writer to which the data is written.
    ///     - `callback`: A callback function to be called with the total size upon stream completion.
    pub fn new(inner: S, writer: W, callback: Arc<dyn Fn(usize) + Send + Sync>) -> Self {
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
            let size = self.size.load(Ordering::SeqCst);
            if self.writer.flush().is_ok() {
                (self.callback)(size);
            }
        }
    }

    fn on_data(&mut self, data: &Result<Bytes, StreamError>) {
        if let Ok(bytes) = data {
            self.size.fetch_add(bytes.len(), Ordering::SeqCst);
            let bytes_to_write = bytes.clone();
            if let Err(e) = self.writer.write_all(&bytes_to_write) {
                error!("Error writing to resource file: {e}");
            }
        }
    }
}

impl<S, W> Stream for PersistPipeStream<S, W>
where
    S: Stream<Item = Result<bytes::Bytes, StreamError>> + Unpin,
    W: Write + Unpin + 'static,
{
    type Item = Result<Bytes, StreamError>;

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
