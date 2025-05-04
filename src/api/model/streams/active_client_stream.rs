use crate::api::model::active_user_manager::UserConnectionGuard;
use crate::api::api_utils::StreamDetails;
use crate::api::model::active_provider_manager::{ActiveProviderManager, ProviderConnectionGuard};
use crate::api::model::active_user_manager::ActiveUserManager;
use crate::api::model::app_state::AppState;
use crate::api::model::stream::BoxedProviderStream;
use crate::api::model::stream_error::StreamError;
use crate::api::model::streams::chunked_buffer::ChunkedBuffer;
use crate::model::{ProxyUserCredentials, UserConnectionPermission};
use bytes::Bytes;
use futures::Stream;
use log::{error, info};
use std::pin::Pin;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;
use std::task::Poll;

const PROVIDER_EXHAUSTED_FLAG: u8 = 1;
const USER_EXHAUSTED_FLAG: u8 = 2;

#[repr(align(64))]
pub(in crate::api) struct ActiveClientStream {
    inner: BoxedProviderStream,
    #[allow(unused)]
    user_connection_guard: Option<UserConnectionGuard>,
    send_custom_stream_flag: Option<Arc<AtomicU8>>,
    custom_video: (Option<ChunkedBuffer>, Option<ChunkedBuffer>),
    #[allow(dead_code)]
    provider_connection_guard: Option<ProviderConnectionGuard>,
}

impl ActiveClientStream {
    pub(crate) async fn new(mut stream_details: StreamDetails,
                            app_state: &AppState,
                            user: &ProxyUserCredentials,
                            connection_permission: UserConnectionPermission) -> Self {
        let active_user = app_state.active_users.clone();
        let active_provider = app_state.active_provider.clone();
        if connection_permission == UserConnectionPermission::Exhausted {
            error!("Something is wrong this should not happen");
        }
        let grant_user_grace_period = connection_permission == UserConnectionPermission::GracePeriod;
        let username = user.username.as_str();
        let user_connection_guard = Some(active_user.add_connection(username, user.max_connections).await);
        let grace_stop_flag = Self::stream_grace_period(&stream_details, &active_provider, grant_user_grace_period, user, &active_user);
        Self {
            inner: stream_details.stream.take().unwrap(),
            user_connection_guard,
            send_custom_stream_flag: grace_stop_flag,
            custom_video: (
                app_state.config.t_provider_connections_exhausted_video.as_ref().map(|a| ChunkedBuffer::new(Arc::clone(a))),
                app_state.config.t_user_connections_exhausted_video.as_ref().map(|a| ChunkedBuffer::new(Arc::clone(a))),
            ),
            provider_connection_guard: stream_details.provider_connection_guard,
        }
    }

    fn stream_grace_period(stream_details: &StreamDetails, active_provider: &Arc<ActiveProviderManager>,
                           user_grace_period: bool, user: &ProxyUserCredentials, active_user: &Arc<ActiveUserManager>) -> Option<Arc<AtomicU8>> {
        let provider_grace_check = if stream_details.has_grace_period() && stream_details.input_name.is_some() {
            let provider_name = stream_details.input_name.as_deref().unwrap_or_default().to_string();
            let provider_manager = Arc::clone(active_provider);
            let reconnect_flag = stream_details.reconnect_flag.clone();
            Some((provider_name, provider_manager, reconnect_flag))
        } else {
            None
        };
        let user_max_connections = user.max_connections;
        let user_grace_check = if user_grace_period && user_max_connections > 0 {
            let user_name = user.username.clone();
            let user_manager = Arc::clone(active_user);
            let reconnect_flag = stream_details.reconnect_flag.clone();
            Some((user_name, user_manager, user_max_connections, reconnect_flag))
        } else {
            None
        };

        if provider_grace_check.is_some() || user_grace_check.is_some() {
            let stop_flag = Arc::new(AtomicU8::new(0));
            let stop_stream_flag = Arc::clone(&stop_flag);
            let grace_period_millis = stream_details.grace_period_millis;
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_millis)).await;
                if let Some((username, user_manager, max_connections, reconnect_flag)) = user_grace_check {
                    let active_connections = user_manager.user_connections(&username).await;
                    if active_connections > max_connections {
                        info!("User connections exhausted for active clients: {username}");
                        stop_stream_flag.store(USER_EXHAUSTED_FLAG, std::sync::atomic::Ordering::SeqCst);
                        if let Some(connect_flag) = reconnect_flag {
                            info!("Stopped reconnect, user connections exhausted");
                            connect_flag.notify();
                        }
                    }
                }
                if let Some((provider_name, provider_manager, reconnect_flag)) = provider_grace_check {
                    if provider_manager.is_over_limit(&provider_name).await {
                        info!("Provider connections exhausted for active clients: {provider_name}");
                        stop_stream_flag.store(PROVIDER_EXHAUSTED_FLAG, std::sync::atomic::Ordering::SeqCst);
                        if let Some(connect_flag) = reconnect_flag {
                            info!("Stopped reconnect, provider connections exhausted");
                            connect_flag.notify();
                        }
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

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        let send_custom_stream = if let Some(send_custom_stream_flag) = &self.send_custom_stream_flag {
            send_custom_stream_flag.load(std::sync::atomic::Ordering::SeqCst)
        } else {
            0
        };
        if send_custom_stream > 0 {
            if self.provider_connection_guard.is_some() {
                drop(self.provider_connection_guard.take());
            }
            let custom_video = if send_custom_stream == PROVIDER_EXHAUSTED_FLAG {
                self.custom_video.0.as_mut()
            } else {
                self.custom_video.1.as_mut()
            };
            return match custom_video {
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
            };
        }
        Pin::new(&mut self.inner).poll_next(cx)
    }
}
