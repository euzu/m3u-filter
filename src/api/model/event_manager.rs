use crate::api::model::active_provider_manager::ActiveProviderManager;
use crate::api::model::active_user_manager::ActiveUserManager;
use log::info;
use std::sync::Arc;

type Username = String;
type InputName = Option<String>;

pub enum Event {
    StreamConnect((Username, InputName)),
    StreamDisconnect((Username, InputName)),
}

pub struct EventManager {
    active_user: Arc<ActiveUserManager>,
    active_provider: Arc<ActiveProviderManager>,
    log_active_clients: bool,
}

impl EventManager {
    pub fn new(active_user: &Arc<ActiveUserManager>,
               active_provider: &Arc<ActiveProviderManager>,
               log_active_clients: bool,
    ) -> Self {
        Self {
            active_user: Arc::clone(active_user),
            active_provider: Arc::clone(active_provider),
            log_active_clients,
        }
    }

    pub async fn fire(&self, event: Event) {
        match event {
            Event::StreamConnect((username, _input_name)) => {
                let (client_count, connection_count) = self.active_user.add_connection(&username).await;
                if self.log_active_clients {
                    info!("Active clients: {client_count}, active connections {connection_count}");
                }
            }
            Event::StreamDisconnect((username, input_name)) => {
                let (client_count, connection_count) = self.active_user.remove_connection(&username).await;
                if self.log_active_clients {
                    info!("Active clients: {client_count}, active connections {connection_count}");
                }

                if let Some(input) = input_name {
                    self.active_provider.release_connection(&input);
                }
            }
        };
    }
}
