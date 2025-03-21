use crate::api::model::stream_error::StreamError;
use bytes::Bytes;
use futures::{Stream};
use std::pin::Pin;
use std::sync::{Arc};
use std::sync::atomic::AtomicBool;
use std::task::Poll;
use log::info;
use crate::api::api_utils::StreamDetails;
use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::app_state::AppState;
use crate::api::model::stream::BoxedProviderStream;
use crate::api::model::streams::chunked_buffer::ChunkedBuffer;

const GRACE_PERIOD_SECONDS: u64 = 2;

#[repr(align(64))]
pub(in crate::api) struct ActiveClientStream {
    inner: BoxedProviderStream,
    username: String,
    input_name: Option<String>,
    active_user: Arc<ActiveUserManager>,
    active_provider: Arc<ActiveProviderManager>,
    log_active_clients: bool,
    send_custom_stream_flag: Option<Arc<AtomicBool>>,
    custom_video: Option<ChunkedBuffer>,
}

impl ActiveClientStream {
    pub(crate) async fn new(mut stream_details: StreamDetails,
                            app_state: &AppState,
                            username: &str) -> Self {
        let active_user = app_state.active_users.clone();
        let active_provider = app_state.active_provider.clone();
        let log_active_clients = app_state.config.log.as_ref().is_some_and(|l| l.active_clients);
        let (client_count, connection_count) = active_user.add_connection(&username).await;
        if log_active_clients {
            info!("Active clients: {client_count}, active connections {connection_count}");
        }

        let stop_flag = Self::stream_grace_period(&stream_details, &active_provider);

        Self {
            inner: stream_details.stream.take().unwrap(),
            active_user,
            active_provider,
            log_active_clients,
            username: username.to_string(),
            input_name: stream_details.input_name,
            send_custom_stream_flag: stop_flag,
            custom_video: app_state.config.t_provider_connections_exhausted_video.as_ref().map(|a| ChunkedBuffer::new(Arc::clone(a))),
        }
    }
    fn stream_grace_period(stream_details: &StreamDetails, active_provider: &Arc<ActiveProviderManager>) -> Option<Arc<AtomicBool>> {
        if stream_details.grace_period && stream_details.input_name.is_some() {
            let provider_name = stream_details.input_name.as_ref().unwrap().to_string();
            let provider_manager = active_provider.clone();
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_stream_flag = Arc::clone(&stop_flag);
            let reconnect_flag = stream_details.reconnect_flag.clone();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(GRACE_PERIOD_SECONDS)).await;
                if provider_manager.is_over_limit(&provider_name) {
                    info!("is over limit for active clients: {provider_name}");
                    stop_stream_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    if let Some(connect_flag) = reconnect_flag {
                        info!("stopped reconnect");
                        connect_flag.notify();
                    }
                }
            });
            return Some(stop_flag);

        }
        None
    }
}
impl Stream for ActiveClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>,cx: &mut std::task::Context<'_>,) -> Poll<Option<Self::Item>> {
        if let Some(send_custom_stream_flag) = &self.send_custom_stream_flag {
             if send_custom_stream_flag.load(std::sync::atomic::Ordering::SeqCst) {
                return match self.custom_video.as_mut() {
                    None => {
                        Poll::Ready(None)
                    }
                    Some(video) => {
                        match video.next_chunk() {
                            None => {
                                Poll::Ready(None)
                            }
                            Some(bytes) => {
                                Poll::Ready(Some(Ok(bytes)))
                            }
                        }
                    }
                }
             }
        }
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for ActiveClientStream {
    fn drop(&mut self) {
        let username = self.username.clone();
        let input_name = self.input_name.clone();
        let log_active_clients = self.log_active_clients;
        let active_user = Arc::clone(&self.active_user);
        let active_provider = Arc::clone(&self.active_provider);
        tokio::spawn(async move {
            let (client_count, connection_count) = active_user.remove_connection(&username).await;
            if log_active_clients {
                info!("Active clients: {client_count}, active connections {connection_count}");
            }
            if let Some(input) = input_name {
                active_provider.release_connection(&input);
            }
        });
    }
}