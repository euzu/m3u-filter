use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::provider_stream_factory::ResponseStream;
use crate::model::api_proxy::ProxyUserCredentials;
use bytes::Bytes;
use futures::Stream;
use log::info;
use parking_lot::{RwLock};
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

pub(in crate::api) struct ActiveClientStream {
    inner: ResponseStream,
    active_clients: Arc<RwLock<ActiveUserManager>>,
    log_active_clients: bool,
    username: String,
}

impl ActiveClientStream {
    pub(crate) fn new(inner: ResponseStream, active_clients: Arc<RwLock<ActiveUserManager>>, user: &ProxyUserCredentials, log_active_clients: bool) -> Self {
        let client_count = {
            let mut clients = active_clients.write();
            clients.add_connection(&user.username);
            clients.active_users()
        };
        if log_active_clients {
            info!("Active clients: {client_count}");
        }
        Self { inner, active_clients, log_active_clients, username: user.username.clone() }
    }
}
impl Stream for ActiveClientStream {
    type Item = Result<Bytes, StreamError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::as_mut(&mut self.inner).poll_next(cx)
    }
}


impl Drop for ActiveClientStream {
    fn drop(&mut self) {
        let client_count = {
            let mut clients = self.active_clients.write();
            clients.remove_connection(&self.username);
            clients.active_users()
        };
        if self.log_active_clients {
            info!("Active clients: {client_count}");
        }
    }
}