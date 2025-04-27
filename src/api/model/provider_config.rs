use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use tokio::sync::RwLock;
use crate::api::model::active_provider_manager::ProviderAllocation;
use crate::model::config::{ConfigInput, ConfigInputAlias, InputType, InputUserInfo};

#[derive(Debug)]
pub enum ProviderConfigAllocation {
    Exhausted,
    Available,
    GracePeriod,
}

/// This struct represents an individual provider configuration with fields like:
///
/// `id`, `name`, `url`, `username`, `password`
/// `input_type`: Determines the type of input the provider supports.
/// `max_connections`: Maximum allowed concurrent connections.
/// `priority`: Priority level for selecting providers.
/// `current_connections`: A `RwLock` to safely track the number of active connections.
#[derive(Debug)]
pub struct ProviderConfig {
    pub id: u16,
    pub name: String,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub input_type: InputType,
    max_connections: u16,
    priority: i16,
    current_connections: AtomicU16,
    granted_grace: AtomicBool,
    grace_ts: AtomicU64,
    lock: RwLock<()>,
}

impl ProviderConfig {
    pub fn new(cfg: &ConfigInput) -> Self {
        Self {
            id: cfg.id,
            name: cfg.name.clone(),
            url: cfg.url.clone(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            input_type: cfg.input_type,
            max_connections: cfg.max_connections,
            priority: cfg.priority,
            current_connections: AtomicU16::new(0),
            granted_grace: AtomicBool::new(false),
            grace_ts: AtomicU64::new(0),
            lock: RwLock::new(()),
        }
    }

    pub fn new_alias(cfg: &ConfigInput, alias: &ConfigInputAlias) -> Self {
        Self {
            id: alias.id,
            name: alias.name.clone(),
            url: alias.url.clone(),
            username: alias.username.clone(),
            password: alias.password.clone(),
            input_type: cfg.input_type,
            max_connections: alias.max_connections,
            priority: alias.priority,
            current_connections: AtomicU16::new(0),
            granted_grace: AtomicBool::new(false),
            grace_ts: AtomicU64::new(0),
            lock: RwLock::new(()),
        }
    }

    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        InputUserInfo::new(self.input_type, self.username.as_deref(), self.password.as_deref(), &self.url)
    }

    #[inline]
    pub fn is_exhausted(&self) -> bool {
        let max = self.max_connections;
        if max == 0 {
            return false;
        }
        self.current_connections.load(Ordering::Acquire) >= max
    }

    #[inline]
    pub fn is_over_limit(&self) -> bool {
        let max = self.max_connections;
        if max == 0 {
            return false;
        }
        self.current_connections.load(Ordering::Acquire) > max
    }

    //
    // #[inline]
    // pub fn has_capacity(&self) -> bool {
    //     !self.is_exhausted()
    // }


    fn force_allocate(&self) {
        self.current_connections.fetch_add(1, Ordering::SeqCst);
    }

    async fn try_allocate(&self, grace: bool) -> ProviderConfigAllocation {
        let _lock = self.lock.write().await;
        let connections = self.current_connections.load(Ordering::SeqCst);
        if self.max_connections == 0 {
            self.current_connections.fetch_add(1, Ordering::SeqCst);
            return ProviderConfigAllocation::Available;
        }
        if (!grace && connections < self.max_connections) || (grace && connections <= self.max_connections) {
            self.current_connections.fetch_add(1, Ordering::SeqCst);
            return if connections < self.max_connections { ProviderConfigAllocation::Available } else { ProviderConfigAllocation::GracePeriod };
        }
        ProviderConfigAllocation::Exhausted
    }

    // is intended to use with redirects, to cycle through provider
    async fn get_next(&self, grace: bool) -> bool {
        let _lock = self.lock.write().await;
        let connections = self.current_connections.load(Ordering::SeqCst);
        if self.max_connections == 0 {
            return true;
        }
        if (!grace && connections < self.max_connections) || (grace && connections <= self.max_connections) {
            return true;
        }
        false
    }

    pub async fn release(&self) {
        let _ = self.current_connections.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current > 0 {
                Some(current - 1)
            } else {
                None
            }
        });
    }

    #[inline]
    pub(crate) fn get_current_connections(&self) -> u16 {
        self.current_connections.load(Ordering::SeqCst)
    }

    #[inline]
    pub(crate) fn get_priority(&self) -> i16 {
        self.priority
    }

}

#[derive(Clone, Debug)]
pub(in crate::api::model) struct ProviderConfigWrapper {
    inner: Arc<ProviderConfig>,
}


impl ProviderConfigWrapper {
    pub fn new(cfg: ProviderConfig) -> Self {
        Self {
            inner: Arc::new(cfg)
        }
    }

    pub fn force_allocate(&self) -> ProviderAllocation {
        self.inner.force_allocate();
        ProviderAllocation::Available(Arc::clone(&self.inner))
    }

    pub async fn try_allocate(&self, grace: bool) -> ProviderAllocation {
        match self.inner.try_allocate(grace).await {
            ProviderConfigAllocation::Available => ProviderAllocation::Available(Arc::clone(&self.inner)),
            ProviderConfigAllocation::GracePeriod => ProviderAllocation::GracePeriod(Arc::clone(&self.inner)),
            ProviderConfigAllocation::Exhausted => ProviderAllocation::Exhausted,
        }
    }

    pub async fn get_next(&self, grace: bool) -> Option<Arc<ProviderConfig>> {
        if self.inner.get_next(grace).await {
            return Some(Arc::clone(&self.inner));
        }
        None
    }

}
impl Deref for ProviderConfigWrapper {
    type Target = ProviderConfig;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}