use crate::api::model::active_provider_manager::ProviderAllocation;
use crate::model::config::{ConfigInput, ConfigInputAlias, InputType, InputUserInfo};
use std::ops::Deref;
use std::sync::Arc;
use jsonwebtoken::get_current_timestamp;
use log::debug;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum ProviderConfigAllocation {
    Exhausted,
    Available,
    GracePeriod,
}

#[derive(Debug)]
struct ProviderConfigConnection {
    current_connections: u16,
    granted_grace: bool,
    grace_ts: u64,
}

impl Default for ProviderConfigConnection {
    /// Creates a new `ProviderConfigConnection` with zero active connections and no grace period granted.
    ///
    /// # Examples
    ///
    /// ```
    /// let conn = ProviderConfigConnection::default();
    /// assert_eq!(conn.current_connections, 0);
    /// assert!(!conn.granted_grace);
    /// assert_eq!(conn.grace_ts, 0);
    /// ```
    fn default() -> Self {
        Self {
            current_connections: 0,
            granted_grace: false,
            grace_ts: 0,
        }
    }
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
    connection: RwLock<ProviderConfigConnection>,
}

impl ProviderConfig {
    /// Creates a new `ProviderConfig` from the given configuration input.
    ///
    /// Initializes all provider fields and sets the connection state to default values.
    ///
    /// # Examples
    ///
    /// ```
    /// let cfg = ConfigInput {
    ///     id: 1,
    ///     name: "ProviderA".to_string(),
    ///     url: "https://example.com".to_string(),
    ///     username: Some("user".to_string()),
    ///     password: Some("pass".to_string()),
    ///     input_type: InputType::Api,
    ///     max_connections: 10,
    ///     priority: 5,
    /// };
    /// let provider = ProviderConfig::new(&cfg);
    /// assert_eq!(provider.id, 1);
    /// assert_eq!(provider.max_connections, 10);
    /// ```
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
            connection: RwLock::new(ProviderConfigConnection::default()),
        }
    }

    /// Creates a new `ProviderConfig` using values from a configuration input and an alias.
    ///
    /// The resulting provider configuration uses the alias's ID, name, URL, credentials, connection limits, and priority, while inheriting the input type from the base configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let cfg = ConfigInput { /* fields omitted */ };
    /// let alias = ConfigInputAlias { /* fields omitted */ };
    /// let provider = ProviderConfig::new_alias(&cfg, &alias);
    /// assert_eq!(provider.id, alias.id);
    /// ```
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
            connection: RwLock::new(ProviderConfigConnection::default()),
        }
    }

    /// Returns user authentication information for this provider if credentials are available.
    ///
    /// Constructs an `InputUserInfo` using the provider's input type, username, password, and URL. Returns `None` if user information cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// let provider = ProviderConfig::new(&cfg);
    /// if let Some(user_info) = provider.get_user_info() {
    ///     // Use user_info for authentication
    /// }
    /// ```
    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        InputUserInfo::new(self.input_type, self.username.as_deref(), self.password.as_deref(), &self.url)
    }

    #[inline]
    /// Returns true if the provider has reached its maximum allowed concurrent connections.
    ///
    /// This method checks whether the current number of active connections is greater than or equal to the configured maximum. If the maximum is set to zero, it is treated as unlimited and always returns false.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn check_exhausted(cfg: &ProviderConfig) {
    /// let exhausted = cfg.is_exhausted().await;
    /// assert!(!exhausted); // if under the connection limit
    /// # }
    /// ```
    pub async fn is_exhausted(&self) -> bool {
        let max = self.max_connections;
        if max == 0 {
            return false;
        }
        self.connection.read().await.current_connections >= max
    }

    #[inline]
    /// Returns true if the number of active connections exceeds the configured maximum.
    ///
    /// This method checks whether the current active connections for the provider are greater than the allowed maximum. If the maximum is set to zero, it always returns false.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn check_over_limit(cfg: &ProviderConfig) {
    /// let over_limit = cfg.is_over_limit().await;
    /// assert!(!over_limit); // Assuming no connections are active and max_connections > 0
    /// # }
    /// ```
    pub async fn is_over_limit(&self) -> bool {
        let max = self.max_connections;
        if max == 0 {
            return false;
        }
        self.connection.read().await.current_connections > max
    }

    //
    // #[inline]
    // pub fn has_capacity(&self) -> bool {
    //     !self.is_exhausted()
    /// Increments the current connection count without checking limits.
    ///
    /// This method forcibly allocates a connection slot, bypassing maximum connection and grace period checks.
    /// 
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn example(cfg: ProviderConfig) {
    /// cfg.force_allocate().await;
    /// // The current connection count is incremented unconditionally.
    /// # }
    /// ```


    async fn force_allocate(&self) {
        let mut guard = self.connection.write().await;
        guard.current_connections += 1;
    }

    /// Attempts to allocate a connection slot, applying grace period logic if the maximum is reached.
    ///
    /// If the current connections are below the maximum, allocation succeeds. If the limit is reached and grace is enabled, a single grace allocation is allowed within the specified timeout. Further attempts during the grace period are denied until the timeout expires.
    ///
    /// # Parameters
    /// - `grace`: Whether to allow a grace period allocation when at capacity.
    /// - `grace_period_timeout_secs`: Duration in seconds for which the grace period remains active.
    ///
    /// # Returns
    /// A `ProviderConfigAllocation` indicating whether the allocation succeeded, used the grace period, or was denied.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::{ProviderConfig, ProviderConfigAllocation};
    /// # async fn example(cfg: ProviderConfig) {
    /// let result = cfg.try_allocate(true, 60).await;
    /// match result {
    ///     ProviderConfigAllocation::Available => println!("Connection allocated"),
    ///     ProviderConfigAllocation::GracePeriod => println!("Allocated with grace period"),
    ///     ProviderConfigAllocation::Exhausted => println!("No connections available"),
    /// }
    /// # }
    /// ```
    async fn try_allocate(&self, grace: bool, grace_period_timeout_secs: u64) -> ProviderConfigAllocation {
        let mut guard = self.connection.write().await;
        if self.max_connections == 0 {
            guard.current_connections += 1;
            return ProviderConfigAllocation::Available;
        }
        let connections = guard.current_connections;
        if connections < self.max_connections || (grace && connections <= self.max_connections) {
            if connections < self.max_connections {
                guard.granted_grace = false;
                guard.grace_ts = 0;
                guard.current_connections += 1;
                return ProviderConfigAllocation::Available;
            }

            let now = get_current_timestamp();
            if guard.granted_grace {
                if now - guard.grace_ts <= grace_period_timeout_secs {
                    // Grace timeout still active, deny connection
                    debug!("Provider access denied, grace exhausted, too many connections: {}", self.name);
                    return ProviderConfigAllocation::Exhausted;
                } else {
                    // Grace timeout expired, reset grace counters
                    guard.granted_grace = false;
                    guard.grace_ts = 0;
                }
            }
            guard.granted_grace = true;
            guard.grace_ts = now;
            guard.current_connections += 1;
            return ProviderConfigAllocation::GracePeriod
        }
        ProviderConfigAllocation::Exhausted
    }

    /// Checks if a new connection can be allocated, considering maximum connections and grace period logic, without incrementing the connection count.
    ///
    /// Returns `true` if allocation is possible under current limits or grace period conditions; otherwise, returns `false`.
    ///
    /// # Parameters
    /// - `grace`: Whether to allow allocation under a grace period if the maximum is reached.
    /// - `grace_period_timeout_secs`: Duration in seconds for which the grace period remains valid.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn example(cfg: ProviderConfig) {
    /// let can_allocate = cfg.get_next(true, 30).await;
    /// assert!(can_allocate || !can_allocate);
    /// # }
    /// ```
    async fn get_next(&self, grace: bool, grace_period_timeout_secs: u64) -> bool {
        if self.max_connections == 0 {
            return true;
        }
        let mut guard = self.connection.write().await;
        let connections = guard.current_connections;
        if connections < self.max_connections || (grace && connections <= self.max_connections) {
            let now = get_current_timestamp();
            if guard.granted_grace {
                if now - guard.grace_ts <= grace_period_timeout_secs {
                    // Grace timeout still active, deny connection
                    debug!("Provider access denied, grace exhausted, too many connections: {}", self.name);
                    return false;
                } else {
                    // Grace timeout expired, reset grace counters
                    guard.granted_grace = false;
                    guard.grace_ts = 0;
                }
            }
            return true;
        }
        false
    }

    /// Decrements the active connection count if greater than zero, preserving grace period state.
    ///
    /// This method reduces the number of current active connections for the provider configuration,
    /// but does not reset any grace period tracking. The grace period state is maintained until
    /// the next allocation attempt.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn example(cfg: ProviderConfig) {
    /// cfg.release().await;
    /// # }
    /// ```
    pub async fn release(&self) {
        // DO NOT reset granted_grace or grace_ts here!
        // We must preserve the grace period state until allocate() checks it.
        let mut guard = self.connection.write().await;
        if guard.current_connections > 0 {
            guard.current_connections -= 1;
        }
    }

    #[inline]
    /// Returns the current number of active connections for this provider.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ProviderConfig;
    /// # async fn example(cfg: ProviderConfig) {
    /// let count = cfg.get_current_connections().await;
    /// assert!(count >= 0);
    /// # }
    /// ```
    pub(crate) async fn get_current_connections(&self) -> u16 {
        self.connection.read().await.current_connections
    }

    #[inline]
    /// Returns the priority value assigned to this provider configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = ProviderConfig::new(&cfg_input);
    /// let priority = config.get_priority();
    /// assert_eq!(priority, config.priority);
    /// ```
    pub(crate) fn get_priority(&self) -> i16 {
        self.priority
    }
}

#[derive(Clone, Debug)]
pub(in crate::api::model) struct ProviderConfigWrapper {
    inner: Arc<ProviderConfig>,
}


impl ProviderConfigWrapper {
    /// Creates a new `ProviderConfigWrapper` containing the given provider configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = ProviderConfig::new(&cfg_input);
    /// let wrapper = ProviderConfigWrapper::new(config);
    /// assert_eq!(wrapper.id, config.id);
    /// ```
    pub fn new(cfg: ProviderConfig) -> Self {
        Self {
            inner: Arc::new(cfg)
        }
    }

    /// Unconditionally allocates a connection slot for the provider, bypassing connection limits.
    ///
    /// Returns a `ProviderAllocation::Available` variant containing a clone of the inner provider configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let wrapper = ProviderConfigWrapper::new(provider_config);
    /// let allocation = wrapper.force_allocate().await;
    /// match allocation {
    ///     ProviderAllocation::Available(cfg) => assert_eq!(cfg.id, provider_config.id),
    ///     _ => panic!("Expected allocation to be available"),
    /// }
    /// ```
    pub async fn force_allocate(&self) -> ProviderAllocation {
        self.inner.force_allocate().await;
        ProviderAllocation::Available(Arc::clone(&self.inner))
    }

    /// Attempts to allocate a connection slot for the provider, considering maximum connections and optional grace period.
    ///
    /// Returns a `ProviderAllocation` indicating whether the allocation succeeded, was granted under a grace period, or was exhausted.
    ///
    /// # Parameters
    /// - `grace`: If true, allows allocation under a grace period when at capacity.
    /// - `grace_period_timeout_secs`: Duration in seconds for which the grace period remains valid.
    ///
    /// # Returns
    /// A `ProviderAllocation` variant reflecting the allocation outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// let wrapper = ProviderConfigWrapper::new(provider_config);
    /// let allocation = wrapper.try_allocate(true, 30).await;
    /// match allocation {
    ///     ProviderAllocation::Available(cfg) => { /* use cfg */ }
    ///     ProviderAllocation::GracePeriod(cfg) => { /* use cfg with caution */ }
    ///     ProviderAllocation::Exhausted => { /* handle exhaustion */ }
    /// }
    /// ```
    pub async fn try_allocate(&self, grace: bool, grace_period_timeout_secs: u64) -> ProviderAllocation {
        match self.inner.try_allocate(grace, grace_period_timeout_secs).await {
            ProviderConfigAllocation::Available => ProviderAllocation::Available(Arc::clone(&self.inner)),
            ProviderConfigAllocation::GracePeriod => ProviderAllocation::GracePeriod(Arc::clone(&self.inner)),
            ProviderConfigAllocation::Exhausted => ProviderAllocation::Exhausted,
        }
    }

    /// Checks if a connection can be allocated for this provider, considering grace period rules, without incrementing the connection count.
    ///
    /// Returns a clone of the provider configuration if allocation is possible; otherwise, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::{ProviderConfigWrapper};
    /// # async fn example(wrapper: ProviderConfigWrapper) {
    /// let result = wrapper.get_next(true, 60).await;
    /// if let Some(provider) = result {
    ///     // Allocation is possible
    /// }
    /// # }
    /// ```
    pub async fn get_next(&self, grace: bool, grace_period_timeout_secs: u64) -> Option<Arc<ProviderConfig>> {
        if self.inner.get_next(grace, grace_period_timeout_secs).await {
            return Some(Arc::clone(&self.inner));
        }
        None
    }
}
impl Deref for ProviderConfigWrapper {
    type Target = ProviderConfig;

    /// Returns a reference to the inner `ProviderConfig`, enabling transparent access through the wrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = ProviderConfig::new(&cfg_input);
    /// let wrapper = ProviderConfigWrapper::new(config);
    /// assert_eq!(wrapper.id, wrapper.deref().id);
    /// ```
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}