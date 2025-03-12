use crate::model::config::{ConfigInput, ConfigInputAlias, InputType};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::RwLock;

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
    current_connections: RwLock<u16>,
}

impl ProviderConfig {
    pub fn new(cfg: &ConfigInput) -> Self {
        Self {
            id: cfg.id,
            name: cfg.name.clone(),
            url: cfg.url.clone(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            input_type: cfg.input_type.clone(),
            max_connections: cfg.max_connections,
            priority: cfg.priority,
            current_connections: RwLock::new(0),
        }
    }

    pub fn new_alias(cfg: &ConfigInput, alias: &ConfigInputAlias) -> Self {
        Self {
            id: alias.id,
            name: cfg.name.clone(),
            url: alias.url.clone(),
            username: alias.username.clone(),
            password: alias.password.clone(),
            input_type: cfg.input_type.clone(),
            max_connections: alias.max_connections,
            priority: alias.priority,
            current_connections: RwLock::new(0),
        }
    }

    #[inline]
    pub async fn is_exhausted(&self) -> bool {
        self.max_connections > 0 && *self.current_connections.read().await >= self.max_connections
    }
    //
    // #[inline]
    // pub fn has_capacity(&self) -> bool {
    //     !self.is_exhausted()
    // }

    pub async fn try_allocate(&self, force: bool) -> bool {
        let mut connections = self.current_connections.write().await;
        if force || *connections < self.max_connections {
            *connections += 1;
            return true;
        }
        false
    }

    pub async fn release(&self) {
        let mut connections = self.current_connections.write().await;
        if *connections > 0 {
            *connections -= 1;
        }
    }

    pub async fn get_connection(&self) -> u16 {
        *self.current_connections.read().await
    }
}

/// This manages different types of provider lineups:
///
/// `Single(SingleProviderLineup)`: A single provider.
/// `Multi(MultiProviderLineup)`: A set of providers grouped by priority.
#[derive(Debug)]
enum ProviderLineup {
    Single(SingleProviderLineup),
    Multi(MultiProviderLineup),
}

impl ProviderLineup {
    async fn acquire(&self, force: bool) -> Option<&ProviderConfig> {
        match self {
            ProviderLineup::Single(lineup) => lineup.acquire(force).await,
            ProviderLineup::Multi(lineup) => lineup.acquire(force).await,
        }
    }

    async fn release(&self, provider_id: u16) {
        match self {
            ProviderLineup::Single(lineup) => lineup.release(provider_id).await,
            ProviderLineup::Multi(lineup) => lineup.release(provider_id).await,
        }
    }
}

/// Handles a single provider and ensures safe allocation/release of connections.
#[derive(Debug)]
struct SingleProviderLineup {
    provider: ProviderConfig,
}

impl SingleProviderLineup {
    fn new(cfg: &ConfigInput) -> Self {
        Self {
            provider: ProviderConfig::new(cfg),
        }
    }

    async fn acquire(&self, force: bool) -> Option<&ProviderConfig> {
        if self.provider.try_allocate(force).await {
            Some(&self.provider)
        } else {
            None
        }
    }

    async fn release(&self, provider_id: u16) {
        if self.provider.id == provider_id {
            self.provider.release().await;
        }
    }
}


/// Manages provider groups based on priority:
///
/// `SingleProviderGroup(ProviderConfig)`: A single provider.
/// `MultiProviderGroup(AtomicUsize, Vec<ProviderConfig>)`: A list of providers with a priority index.
#[derive(Debug)]
enum ProviderPriorityGroup {
    SingleProviderGroup(ProviderConfig),
    MultiProviderGroup(AtomicUsize, Vec<ProviderConfig>),
}

impl ProviderPriorityGroup {
    async fn is_exhausted(&self) -> bool {
        match self {
            ProviderPriorityGroup::SingleProviderGroup(g) => g.is_exhausted().await,
            ProviderPriorityGroup::MultiProviderGroup(_, groups) => {
                for g in groups {
                    if !g.is_exhausted().await {
                        return false;
                    }
                }
                true
            }
        }
    }
}


/// Manages multiple providers, ensuring that connections are allocated in a round-robin manner based on priority.
#[derive(Debug)]
struct MultiProviderLineup {
    providers: Vec<ProviderPriorityGroup>,
    index: AtomicUsize,
}

impl MultiProviderLineup {
    pub fn new(input: &ConfigInput) -> Self {
        let mut inputs = vec![ProviderConfig::new(input)];
        if let Some(aliases) = &input.aliases {
            for alias in aliases {
                inputs.push(ProviderConfig::new_alias(input, alias));
            }
        }
        let mut providers = HashMap::new();
        for provider in inputs {
            let priority = provider.priority;
            providers.entry(priority)
                .or_insert_with(Vec::new)
                .push(provider);
        }
        let mut values: Vec<(i16, Vec<ProviderConfig>)> = providers.into_iter().collect();
        values.sort_by(|(p1, _), (p2, _)| p1.cmp(p2));
        let providers: Vec<ProviderPriorityGroup> = values.into_iter().map(|(_, mut group)| {
            if group.len() > 1 {
                ProviderPriorityGroup::MultiProviderGroup(AtomicUsize::new(0), group)
            } else {
                ProviderPriorityGroup::SingleProviderGroup(group.remove(0))
            }
        }).collect();

        Self {
            providers,
            index: AtomicUsize::new(0),
        }
    }

    /// Attempts to acquire the next available provider from a specific priority group.
    ///
    /// # Parameters
    /// - `group_index`: The index of the provider group to search within.
    /// - `force`: A boolean flag indicating whether to return a provider even if all are exhausted.
    ///
    /// # Returns
    /// - `Some(&ProviderConfig)`: A reference to the next available provider in the specified group.
    /// - `None`: If no providers are available in the group and `force` is `false`.
    ///
    /// # Behavior
    /// - Iterates through the providers in the given group in a round-robin manner.
    /// - Checks if a provider has available capacity before selecting it.
    /// - If `force` is `true`, returns a provider even if no capacity is available.
    /// - Uses atomic operations to maintain fair provider selection.
    ///
    /// # Thread Safety
    /// - Uses `RwLock` for safe concurrent access.
    /// - Ensures fair provider allocation across multiple threads.
    ///
    /// # Example Usage
    /// ```rust
    /// let lineup = MultiProviderLineup::new(&config);
    /// if let Some(provider) = lineup.acquire_next_provider_from_group(0, false) {
    ///     println!("Acquired provider: {}", provider.name);
    /// } else {
    ///     println!("No available providers in group 0.");
    /// }
    /// ```
    async fn acquire_next_provider_from_group(priority_group: &ProviderPriorityGroup) -> Option<&ProviderConfig> {
        match priority_group {
            ProviderPriorityGroup::SingleProviderGroup(p) => {
                if p.try_allocate(false).await {
                    return Some(p);
                }
            }
            ProviderPriorityGroup::MultiProviderGroup(index, pg) => {
                let mut idx = index.load(Ordering::SeqCst);
                let provider_count = pg.len();
                for _ in 0..provider_count {
                    let p = pg.get(idx).unwrap();
                    idx = (idx + 1) % provider_count;
                    if p.try_allocate(false).await {
                        index.store(idx, Ordering::SeqCst);
                        return Some(p);
                    }
                }
                index.store(idx, Ordering::SeqCst);
            }
        }
        None
    }

    /// Attempts to acquire a provider from the lineup based on priority and availability.
    ///
    /// # Parameters
    /// - `force`: A boolean flag indicating whether to force allocation even if all providers are exhausted.
    ///
    /// # Returns
    /// - `Some(&ProviderConfig)`: A reference to the acquired provider if allocation was successful.
    /// - `None`: If no providers are available and `force` is `false`.
    ///
    /// # Behavior
    /// - The method iterates through provider priority groups in a round-robin fashion.
    /// - It attempts to allocate a provider from the highest priority group first.
    /// - If a provider has available capacity, it is returned.
    /// - If all providers in a group are exhausted, it moves to the next group.
    /// - If `force` is `true`, it will return a provider even if all are exhausted.
    /// - Updates the internal index to ensure fair distribution of requests.
    ///
    /// # Thread Safety
    /// - Uses atomic operations (`AtomicUsize`) for thread-safe indexing.
    /// - Uses `RwLock` for thread-safe provider allocation.
    ///
    /// # Example Usage
    /// ```rust
    /// let lineup = MultiProviderLineup::new(&config);
    /// if let Some(provider) = lineup.acquire(false) {
    ///     println!("Acquired provider: {}", provider.name);
    /// } else {
    ///     println!("No available providers.");
    /// }
    /// ```
    async fn acquire(&self, force: bool) -> Option<&ProviderConfig> {
        let mut main_idx = self.index.load(Ordering::SeqCst);
        let provider_count = self.providers.len();

        for _ in 0..provider_count {
            let priority_group = &self.providers[main_idx];
            main_idx = (main_idx + 1) % provider_count;
            if let Some(provider) = Self::acquire_next_provider_from_group(priority_group).await {
                if priority_group.is_exhausted().await {
                    self.index.store(main_idx, Ordering::SeqCst);
                }
                return Some(provider);
            }
        }

        if force {
            let provider = &self.providers[main_idx];
            self.index.store((main_idx + 1) % provider_count, Ordering::SeqCst);

            return match provider {
                ProviderPriorityGroup::SingleProviderGroup(p) => Some(p),
                ProviderPriorityGroup::MultiProviderGroup(gindex, group) => {
                    let idx = gindex.load(Ordering::SeqCst);
                    gindex.store((idx + 1) % group.len(), Ordering::SeqCst);
                    group.get(idx)
                }
            };
        }

        None
    }


    async fn release(&self, provider_id: u16) {
        for g in &self.providers {
            match g {
                ProviderPriorityGroup::SingleProviderGroup(pc) => {
                    if pc.id == provider_id {
                        pc.release().await;
                        break;
                    }
                }
                ProviderPriorityGroup::MultiProviderGroup(_, group) => {
                    for pc in group {
                        if pc.id == provider_id {
                            pc.release().await;
                            return;
                        }
                    }
                }
            }
        }
    }

}

pub struct ActiveProviderManager {
    user_access_control: bool,
    providers: HashMap<String, ProviderLineup>,
}

impl ActiveProviderManager {
    pub fn new(user_access_control: bool) -> Self {
        Self {
            user_access_control,
            providers: HashMap::new(),
        }
    }

    pub fn add_provider(&mut self, name: &str, input: &ConfigInput) {
        let lineup = if input.aliases.as_ref().is_some_and(|a| !a.is_empty()) {
            ProviderLineup::Multi(MultiProviderLineup::new(input))
        } else {
            ProviderLineup::Single(SingleProviderLineup::new(input))
        };
        self.providers.insert(name.to_string(), lineup);
    }

    pub async fn acquire_connection(&self, lineup_name: &str) -> Option<&ProviderConfig> {
        match self.providers.get(lineup_name) {
            None => None,
            Some(lineup) => lineup.acquire(self.user_access_control).await
        }
    }

    pub async fn release_connection(&self, lineup_name: &str, provider_id: u16) {
        if let Some(lineup) = self.providers.get(lineup_name) {
            lineup.release(provider_id).await;
        }
    }

    pub async fn active_connections(&self) -> Option<HashMap<String, u16>> {
        let mut result = HashMap::<String, u16>::new();
        for lineup in self.providers.values() {
            match lineup {
                ProviderLineup::Single(provider_lineup) => {
                    let count = *provider_lineup.provider.current_connections.read().await;
                    result.insert(provider_lineup.provider.name.to_string(), count);
                }
                ProviderLineup::Multi(provider_lineup) => {
                    for provider_group in &provider_lineup.providers {
                        match provider_group {
                            ProviderPriorityGroup::SingleProviderGroup(provider) => {
                                let count = *provider.current_connections.read().await;
                                result.insert(provider.name.to_string(), count);
                            }
                            ProviderPriorityGroup::MultiProviderGroup(_, providers) => {
                                for provider in providers {
                                    let count = *provider.current_connections.read().await;
                                    result.insert(provider.name.to_string(), count);
                                }
                            }
                        }
                    }
                }
            }
        }
        if  result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Arc;
    use std::thread;

    // Helper function to create a ConfigInput instance
    fn create_config_input(id: u16, name: &str, priority: i16, max_connections: u16) -> ConfigInput {
        ConfigInput {
            id,
            name: name.to_string(),
            url: "http://example.com".to_string(),
            epg_url: None,
            username: None,
            password: None,
            persist: None,
            prefix: None,
            suffix: None,
            enabled: true,
            input_type: InputType::Xtream, // You can use a default value here
            max_connections,
            priority,
            aliases: None,
            headers: Default::default(),
            options: None,
        }
    }

    // Helper function to create a ConfigInputAlias instance
    fn create_config_input_alias(id: u16, url: &str, priority: i16, max_connections: u16) -> ConfigInputAlias {
        ConfigInputAlias {
            id,
            url: url.to_string(),
            username: Some("alias_user".to_string()),
            password: Some("alias_pass".to_string()),
            priority,
            max_connections,
        }
    }

    // Test acquiring with an alias
    #[test]
    fn test_provider_with_alias() {
        let mut input = create_config_input(1, "provider1_1", 1, 1);
        let alias = create_config_input_alias(2, "http://alias1", 2, 2);

        // Adding alias to the provider
        input.aliases = Some(vec![alias]);

        // Create MultiProviderLineup with the provider and alias
        let lineup = MultiProviderLineup::new(&input);

        // Test that the alias provider is available
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);

        // Try acquiring again
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);
        assert_eq!(provider.unwrap().name, "provider1_1");

        // Try acquiring with force (should succeed as force allows even exhausted providers)
        let provider = lineup.acquire(true);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);
        assert_eq!(provider.unwrap().name, "provider1_1");
    }

    // Test acquiring from a MultiProviderLineup where the alias has a different priority
    #[test]
    fn test_provider_with_priority_alias() {
        let mut input = create_config_input(1, "provider2_1", 1, 2);
        let alias = create_config_input_alias(2, "http://alias.com", 0, 2);

        // Adding alias with different priority
        input.aliases = Some(vec![alias]);

        let lineup = MultiProviderLineup::new(&input);

        // The alias has a higher priority, so the alias should be acquired first
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);

        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);

        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);
    }

    // Test provider when there are multiple aliases, all with distinct priorities
    #[test]
    fn test_provider_with_multiple_aliases() {
        let mut input = create_config_input(1, "provider3_1", 1, 1);
        let alias1 = create_config_input_alias(2, "http://alias1.com", 1, 2);
        let alias2 = create_config_input_alias(3, "http://alias2.com", 0, 1);

        // Adding multiple aliases
        input.aliases = Some(vec![alias1, alias2]);

        let lineup = MultiProviderLineup::new(&input);

        // The alias with priority 0 should be acquired first (higher priority)
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 3);

        // Acquire again, and provider should still be available (with remaining capacity)
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);

        // Check that the second alias with priority 2 is considered next
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);
    }

    // // Test acquiring when all aliases are exhausted
    #[test]
    fn test_provider_with_exhausted_aliases() {
        let mut input = create_config_input(1, "provider4_1", 1, 1);
        let alias1 = create_config_input_alias(2, "http://alias.com", 2, 1);
        let alias2 = create_config_input_alias(3, "http://alias.com", -2, 1);

        // Adding alias
        input.aliases = Some(vec![alias1, alias2]);

        let lineup = MultiProviderLineup::new(&input);

        // Acquire connection from alias2
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 3);

        // Acquire connection from provider1
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);

        // Acquire connection from alias1
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);

        // Now, all are exhausted
        assert!(lineup.acquire(false).is_none());
    }

    // Test acquiring a connection when there is available capacity
    #[test]
    fn test_acquire_when_capacity_available() {
        let cfg = create_config_input(1, "provider5_1", 1, 2);
        let lineup = SingleProviderLineup::new(&cfg);

        // First acquire attempt should succeed
        assert!(lineup.acquire(false).is_some());

        // Second acquire attempt should succeed as well
        assert!(lineup.acquire(false).is_some());

        // Third acquire attempt should fail as the provider is exhausted
        assert!(lineup.acquire(false).is_none());
    }

    // Test acquiring a connection with the force flag
    #[test]
    fn test_acquire_with_force_flag() {
        let cfg = create_config_input(1, "provider6_1", 1, 1);
        let lineup = SingleProviderLineup::new(&cfg);

        // First acquire attempt should succeed
        assert!(lineup.acquire(false).is_some());

        // Second acquire attempt should fail without force
        assert!(lineup.acquire(false).is_none());

        // Third acquire attempt should succeed because force is true
        assert!(lineup.acquire(true).is_some());
    }

    // Test releasing a connection
    #[test]
    fn test_release_connection() {
        let cfg = create_config_input(1, "provider7_1", 1, 2);
        let lineup = SingleProviderLineup::new(&cfg);

        // Acquire two connections
        assert!(lineup.acquire(false).is_some());
        assert!(lineup.acquire(false).is_some());

        // Release one connection
        lineup.release(1);

        // After release, one connection should be available
        assert!(lineup.acquire(false).is_some());

        // Release again, no connections should be available now
        assert!(lineup.acquire(false).is_none());
    }

    // Test acquiring with MultiProviderLineup and round-robin allocation
    #[test]
    fn test_multi_provider_acquire() {
        let mut cfg1 = create_config_input(1, "provider8_1", 1, 2);
        let alias = create_config_input_alias(2, "http://alias1", 1, 1);

        // Adding alias to the provider
        cfg1.aliases = Some(vec![alias]);

        // Create MultiProviderLineup with the provider and alias
        let lineup = MultiProviderLineup::new(&cfg1);

        // Test acquiring the first provider
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);

        // Test acquiring the second provider
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);

        // Test acquiring the first provider
        let provider = lineup.acquire(false);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);

        // Test no more providers available
        assert!(lineup.acquire(false).is_none());

        // Force flag should still allow allocation, round robin 2 because last was 1
        let provider = lineup.acquire(true);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 2);

        // Force flag should still allow allocation, round robin 1
        let provider = lineup.acquire(true);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id, 1);
    }

    // Test concurrent access to `acquire` using multiple threads
    #[test]
    fn test_concurrent_acquire() {
        let cfg = create_config_input(1, "provider9_1", 1, 2);
        let lineup = Arc::new(SingleProviderLineup::new(&cfg));

        let mut handles = vec![];

        for _ in 0..5 {
            let lineup_clone = Arc::clone(&lineup);
            let handle = thread::spawn(move || {
                // Each thread tries to acquire a connection
                let _result = lineup_clone.acquire(false);
            });
            handles.push(handle);
        }

        // Join all threads to ensure completion
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify that only the capacity of the provider was utilized (2 connections)
        assert_eq!(*lineup.provider.current_connections.read(), 2);
    }
}

