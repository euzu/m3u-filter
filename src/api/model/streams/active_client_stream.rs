use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::provider_stream_factory::ResponseStream;
use crate::model::api_proxy::ProxyUserCredentials;
use bytes::Bytes;
use futures::Stream;
use log::info;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

pub(in crate::api) struct ActiveClientStream {
    inner: ResponseStream,
    active_clients: Arc<ActiveUserManager>,
    log_active_clients: bool,
    username: String,
}

impl ActiveClientStream {
    pub(crate) async fn new(inner: ResponseStream, active_clients: Arc<ActiveUserManager>, user: &ProxyUserCredentials, log_active_clients: bool) -> Self {
        let (client_count, connection_count) = active_clients.add_connection(&user.username).await;
        if log_active_clients {
            info!("Active clients: {client_count}, active connections {connection_count}");
        }
        Self { inner, active_clients, log_active_clients, username: user.username.clone() }
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
        let log_active_clients = self.log_active_clients;
        let active_clients = Arc::clone(&self.active_clients);

        tokio::spawn(async move {
            let username = username.clone();
            let (client_count, connection_count) = active_clients.remove_connection(&username).await;
            if log_active_clients {
                info!("Active clients: {client_count}, active connections {connection_count}");
            }
        });
    }
}