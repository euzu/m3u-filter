use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::provider_stream_factory::ResponseStream;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use crate::api::model::event_manager::{Event, EventManager};

pub(in crate::api) struct ActiveClientStream {
    inner: ResponseStream,
    event_manager: Arc<EventManager>,
    username: String,
    input_name: Option<String>,
}

impl ActiveClientStream {
    pub(crate) async fn new(inner: ResponseStream, event_manager: Arc<EventManager>, username: &str, input_name: Option<String>) -> Self {
        event_manager.fire(Event::StreamConnect((username.to_string(), input_name.clone()))).await;
        Self { inner, event_manager, username: username.to_string(), input_name }
    }
}
impl Stream for ActiveClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>,cx: &mut std::task::Context<'_>,) -> Poll<Option<Self::Item>> {
        Pin::as_mut(&mut self.inner).poll_next(cx)
    }
}


impl Drop for ActiveClientStream {
    fn drop(&mut self) {
        let username = self.username.clone();
        let input_name = self.input_name.clone();
        let event_manager = Arc::clone(&self.event_manager);

        tokio::spawn(async move {
            event_manager.fire(Event::StreamDisconnect((username.to_string(), input_name))).await;
        });
    }
}