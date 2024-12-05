use futures::stream::{Stream};
use std::{
    error::Error as StdError,
    pin::Pin,
    task::{Context, Poll},
};
use std::sync::atomic::AtomicU32;
use bytes::Bytes;
use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::StreamExt;

pub struct NotifyStream<S> {
    stream: S,
    tx: Option<Sender<()>>,
}

impl<S, E> NotifyStream<S>
where
    S: Stream<Item = Result<Bytes, E>>
{
    pub fn new(stream: S) -> (Self, Receiver<()>) {
        let (send, recv) = channel();
        (NotifyStream {
            stream,
            tx: Some(send),
        }, recv)
    }
}

impl<S, E> Stream for NotifyStream<S> where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<Box<dyn StdError>> + 'static,
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let result = self.stream.poll_next_unpin(cx);
        let mut connection_closed = false;
        match &result {
            Poll::Ready(val) => {
                if val.is_none() {
                    connection_closed = true;
                }
            }
            Poll::Pending => {}
        }

        if connection_closed {
            if let Some(send) = self.tx.take() {
                // Ignore errors as they just mean the receiver was dropped.
                let _ = send.send(());
            }
        }

        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

pub struct SharedStream {
    pub data_stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>>>>,
    pub client_count: AtomicU32,
}

//
// async fn stream_handler(
//     state: web::Data<SharedState>,
//     _req: HttpRequest,
// ) -> impl Responder {
//     let mut shared_state = state.lock().unwrap();
//
//     // Check if a shared resource already exists
//     if let Some(resource) = shared_state.as_mut() {
//         // Increment client count
//         resource.client_count += 1;
//
//         // Create a stream for this client
//         let client_stream = resource.data_stream.clone();
//         return HttpResponse::Ok().streaming(client_stream);
//     }
//
//     // If no shared resource, create one
//     let data_stream = crate::api::model::shared_stream::create_data_stream(); // Replace with your actual stream logic
//     let boxed_stream = Box::pin(data_stream);
//
//     *shared_state = Some(SharedStream {
//         data_stream: boxed_stream.clone(),
//         client_count: 1,
//     });
//
//     // Respond with the stream for this client
//     HttpResponse::Ok().streaming(boxed_stream)
// }
//
//
// async fn cleanup_on_disconnect(state: web::Data<SharedState>) {
//     let mut shared_state = state.lock().unwrap();
//     if let Some(resource) = shared_state.as_mut() {
//         resource.client_count -= 1;
//         if resource.client_count == 0 {
//             // Drop the shared resource if no clients are left
//             *shared_state = None;
//         }
//     }
// }

