use crate::model::config::{Config, ConfigInput, ConfigInputAlias, InputType, InputUserInfo};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

#[derive(Debug)]
pub enum ProviderAllocation<'a> {
    Exhausted,
    Available(&'a ProviderConfig),
    GracePeriod(&'a ProviderConfig),
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
        }
    }

    pub fn get_user_info(&self) -> Option<InputUserInfo> {
        InputUserInfo::new(self.input_type, self.username.as_deref(), self.password.as_deref(), &self.url)
    }

    #[inline]
    pub fn is_exhausted(&self) -> bool {
        self.max_connections > 0 && self.current_connections.load(Ordering::SeqCst) >= self.max_connections
    }

    #[inline]
    pub fn is_over_limit(&self) -> bool {
        self.max_connections > 0 && self.current_connections.load(Ordering::SeqCst) > self.max_connections
    }

    //
    // #[inline]
    // pub fn has_capacity(&self) -> bool {
    //     !self.is_exhausted()
    // }

    pub fn try_allocate(&self, grace: bool) -> ProviderAllocation {
        let connections = self.current_connections.load(Ordering::SeqCst);
        if self.max_connections == 0 {
            self.current_connections.fetch_add(1, Ordering::SeqCst);
            return ProviderAllocation::Available(self);
        }
        if (!grace && connections < self.max_connections) || (grace && connections <= self.max_connections)  {
            self.current_connections.fetch_add(1, Ordering::SeqCst);
            return if connections < self.max_connections { ProviderAllocation::Available(self) } else { ProviderAllocation::GracePeriod(self) };
        }
        ProviderAllocation::Exhausted
    }

    pub fn release(&self) {
        let connections = self.current_connections.load(Ordering::SeqCst);
        if connections > 0 {
            self.current_connections.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn get_connection(&self) -> u16 {
        self.current_connections.load(Ordering::SeqCst)
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
    fn acquire(&self) -> ProviderAllocation {
        match self {
            ProviderLineup::Single(lineup) => lineup.acquire(),
            ProviderLineup::Multi(lineup) => lineup.acquire(),
        }
    }

    fn release(&self, provider_name: &str) {
        match self {
            ProviderLineup::Single(lineup) => lineup.release(provider_name),
            ProviderLineup::Multi(lineup) => lineup.release(provider_name),
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

    fn acquire(&self) -> ProviderAllocation {
        self.provider.try_allocate(true)
    }

    fn release(&self, provider_name: &str) {
        if self.provider.name == provider_name {
            self.provider.release();
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
    fn is_exhausted(&self) -> bool {
        match self {
            ProviderPriorityGroup::SingleProviderGroup(g) => g.is_exhausted(),
            ProviderPriorityGroup::MultiProviderGroup(_, groups) => {
                for g in groups {
                    if !g.is_exhausted() {
                        return false;
                    }
                }
                true
            }
        }
    }
}


/// Manages multiple providers, ensuring that connections are allocated in a round-robin manner based on priority.
#[repr(align(64))]
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
    /// - `priority_group`: Thep rovider group to search within.
    ///
    /// # Returns
    /// - `ProviderAllocation`: A reference to the next available provider in the specified group.
    ///
    /// # Behavior
    /// - Iterates through the providers in the given group in a round-robin manner.
    /// - Checks if a provider has available capacity before selecting it.
    /// - Uses atomic operations to maintain fair provider selection.
    ///
    /// # Thread Safety
    /// - Uses `RwLock` for safe concurrent access.
    /// - Ensures fair provider allocation across multiple threads.
    ///
    /// # Example Usage
    /// ```rust
    /// let lineup = MultiProviderLineup::new(&config);
    /// match lineup.acquire_next_provider_from_group(priority_group) {
    ///    ProviderAllocation::Exhausted => println!("All providers exhausted"),
    ///    ProviderAllocation::Available(provider) =>  println!("Provider available {}", provider.name),
    ///    ProviderAllocation::GracePeriodprovider) =>  println!("Provider with grace period {}", provider.name),
    /// }
    /// }
    /// ```
    fn acquire_next_provider_from_group(priority_group: &ProviderPriorityGroup, grace: bool) -> ProviderAllocation {
        match priority_group {
            ProviderPriorityGroup::SingleProviderGroup(p) => {
                let result = p.try_allocate(grace);
                match result {
                    ProviderAllocation::Exhausted => {}
                    ProviderAllocation::Available(_) | ProviderAllocation::GracePeriod(_) => return result
                }
            }
            ProviderPriorityGroup::MultiProviderGroup(index, pg) => {
                let mut idx = index.load(Ordering::SeqCst);
                let provider_count = pg.len();
                let start = idx;
                for _ in start..provider_count {
                    let p = pg.get(idx).unwrap();
                    idx = (idx + 1) % provider_count;
                    let result = p.try_allocate(grace);
                    match result {
                        ProviderAllocation::Exhausted => {}
                        ProviderAllocation::Available(_) | ProviderAllocation::GracePeriod(_) => {
                            index.store(idx, Ordering::SeqCst);
                            return result;
                        }
                    }
                }
                index.store(idx, Ordering::SeqCst);
            }
        }
        ProviderAllocation::Exhausted
    }

    /// Attempts to acquire a provider from the lineup based on priority and availability.
    ///
    /// # Returns
    /// - `ProviderAllocation`: A reference to the acquired provider if allocation was successful.
    ///
    /// # Behavior
    /// - The method iterates through provider priority groups in a round-robin fashion.
    /// - It attempts to allocate a provider from the highest priority group first.
    /// - If a provider has available capacity, it is returned.
    /// - If all providers in a group are exhausted, it moves to the next group.
    /// - Updates the internal index to ensure fair distribution of requests.
    ///
    /// # Thread Safety
    /// - Uses atomic operations (`AtomicUsize`) for thread-safe indexing.
    /// - Uses `RwLock` for thread-safe provider allocation.
    ///
    /// # Example Usage
    /// ```rust
    /// let lineup = MultiProviderLineup::new(&config);
    /// match lineup.acquire() {
    ///    ProviderAllocation::Exhausted => println!("All providers exhausted"),
    ///    ProviderAllocation::Available(provider) =>  println!("Provider available {}", provider.name),
    ///    ProviderAllocation::GracePeriodprovider) =>  println!("Provider with grace period {}", provider.name),
    /// }
    /// ```
    fn acquire(&self) -> ProviderAllocation {
        let main_idx = self.index.load(Ordering::SeqCst);
        let provider_count = self.providers.len();

        for index in main_idx..provider_count {
            let priority_group = &self.providers[index];
            let allocation = {
                let without_grace_allocation = Self::acquire_next_provider_from_group(priority_group, false);
                if matches!(without_grace_allocation, ProviderAllocation::Exhausted) {
                    Self::acquire_next_provider_from_group(priority_group, true)
                } else {
                    without_grace_allocation
                }
            };
            match allocation {
                ProviderAllocation::Exhausted => {}
                ProviderAllocation::Available(_) |
                ProviderAllocation::GracePeriod(_) => {
                    if priority_group.is_exhausted() {
                        self.index.store((index + 1) % provider_count, Ordering::SeqCst);
                    }
                    return allocation;
                }
            }
        }

        ProviderAllocation::Exhausted
        // let provider = &self.providers[main_idx];
        // self.index.store((main_idx + 1) % provider_count, Ordering::SeqCst);
        //
        // match provider {
        //     ProviderPriorityGroup::SingleProviderGroup(p) => ProviderAllocation::Available(p),
        //     ProviderPriorityGroup::MultiProviderGroup(gindex, group) => {
        //         let idx = gindex.load(Ordering::SeqCst);
        //         gindex.store((idx + 1) % group.len(), Ordering::SeqCst);
        //         match group.get(idx) {
        //             None => ProviderAllocation::Exhausted,
        //             Some(p) => ProviderAllocation::Available(p)
        //         }
        //     }
        // }
    }


    fn release(&self, provider_name: &str) {
        for g in &self.providers {
            match g {
                ProviderPriorityGroup::SingleProviderGroup(pc) => {
                    if pc.name == provider_name {
                        pc.release();
                        break;
                    }
                }
                ProviderPriorityGroup::MultiProviderGroup(_, group) => {
                    for pc in group {
                        if pc.name == provider_name {
                            pc.release();
                            return;
                        }
                    }
                }
            }
        }
    }
}

pub struct ActiveProviderManager {
    providers: Vec<ProviderLineup>,
}

impl ActiveProviderManager {
    pub fn new(cfg: &Config) -> Self {
        let mut this = Self {
            providers: Vec::new(),
        };
        for source in &cfg.sources {
            for input in &source.inputs {
                this.add_provider(input);
            }
        }
        this
    }

    pub fn add_provider(&mut self, input: &ConfigInput) {
        let lineup = if input.aliases.as_ref().is_some_and(|a| !a.is_empty()) {
            ProviderLineup::Multi(MultiProviderLineup::new(input))
        } else {
            ProviderLineup::Single(SingleProviderLineup::new(input))
        };
        self.providers.push(lineup);
    }

    fn get_provider_config(&self, name: &str) -> Option<(&ProviderLineup, &ProviderConfig)> {
        for lineup in &self.providers {
            match lineup {
                ProviderLineup::Single(single) => {
                    if single.provider.name == name {
                        return Some((lineup, &single.provider));
                    }
                }
                ProviderLineup::Multi(multi) => {
                    for group in &multi.providers {
                        match group {
                            ProviderPriorityGroup::SingleProviderGroup(config) => {
                                return Some((lineup, config));
                            }
                            ProviderPriorityGroup::MultiProviderGroup(_, configs) => {
                                for config in configs {
                                    if config.name == name {
                                        return Some((lineup, config));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn acquire_connection(&self, input_name: &str) -> ProviderAllocation {
        match self.get_provider_config(input_name) {
            None => ProviderAllocation::Exhausted,
            Some((lineup, _config)) => lineup.acquire()
        }
    }

    // we need the provider_name to exactly release this provider
    pub fn release_connection(&self, provider_name: &str) {
        if let Some((lineup, _config)) = self.get_provider_config(provider_name) {
            lineup.release(provider_name);
        }
    }

    pub fn active_connections(&self) -> Option<HashMap<String, u16>> {
        let result = RefCell::new(HashMap::<String, u16>::new());
        let add_provider = |provider: &ProviderConfig| {
            let count = provider.current_connections.load(Ordering::SeqCst);
            if count > 0 {
                result.borrow_mut().insert(provider.name.to_string(), count);
            }
        };
        for lineup in &self.providers {
            match lineup {
                ProviderLineup::Single(provider_lineup) => {
                    add_provider(&provider_lineup.provider);
                }
                ProviderLineup::Multi(provider_lineup) => {
                    for provider_group in &provider_lineup.providers {
                        match provider_group {
                            ProviderPriorityGroup::SingleProviderGroup(provider) => {
                                add_provider(provider);
                            }
                            ProviderPriorityGroup::MultiProviderGroup(_, providers) => {
                                for provider in providers {
                                    add_provider(provider);
                                }
                            }
                        }
                    }
                }
            }
        }
        let status = result.take();
        if status.is_empty() {
            None
        } else {
            Some(status)
        }
    }

    pub fn is_over_limit(&self, provider_name: &str) -> bool {
        if let Some((_, config)) = self.get_provider_config(provider_name) {
            config.is_over_limit()
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Arc;
    use std::thread;
    use crate::model::config::InputFetchMethod;

    macro_rules! should_available {
        ($lineup:expr, $provider_id:expr) => {
                      match $lineup.acquire() {
                ProviderAllocation::Exhausted => assert!(false, "Should available and not exhausted"),
                ProviderAllocation::Available(provider) => assert_eq!(provider.id, $provider_id),
                ProviderAllocation::GracePeriod(provider) => assert!(false, "Should available and not grace period: {}", provider.id),
            }
        };
    }
    macro_rules! should_grace_period {
        ($lineup:expr, $provider_id:expr) => {
                      match $lineup.acquire() {
                ProviderAllocation::Exhausted => assert!(false, "Should grace period and not exhausted"),
                ProviderAllocation::Available(provider) => assert!(false, "Should grace period and not available: {}", provider.id),
                ProviderAllocation::GracePeriod(provider) => assert_eq!(provider.id, $provider_id),
            }
        };
    }

    macro_rules! should_exhausted {
        ($lineup:expr) => {
                      match $lineup.acquire() {
                ProviderAllocation::Exhausted => {},
                ProviderAllocation::Available(provider) => assert!(false, "Should exhausted and not available: {}", provider.id),
                ProviderAllocation::GracePeriod(provider) => assert!(false, "Should exhausted and not grace period: {}", provider.id),
            }
        };
    }



    // Helper function to create a ConfigInput instance
    fn create_config_input(id: u16, name: &str, priority: i16, max_connections: u16) -> ConfigInput {
        ConfigInput {
            id,
            name: name.to_string(),
            url: "http://example.com".to_string(),
            epg: Default::default(),
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
            method: InputFetchMethod::default(),
        }
    }

    // Helper function to create a ConfigInputAlias instance
    fn create_config_input_alias(id: u16, url: &str, priority: i16, max_connections: u16) -> ConfigInputAlias {
        ConfigInputAlias {
            id,
            name: format!("alias_{id}"),
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
        should_available!(lineup, 1);
        // Try acquiring again
        should_available!(lineup, 2);
        should_available!(lineup, 2);
        should_grace_period!(lineup, 1);
        should_grace_period!(lineup, 2);
        should_exhausted!(lineup);
    }

    // // Test acquiring from a MultiProviderLineup where the alias has a different priority
    #[test]
    fn test_provider_with_priority_alias() {
        let mut input = create_config_input(1, "provider2_1", 1, 2);
        let alias = create_config_input_alias(2, "http://alias.com", 0, 2);
        // Adding alias with different priority
        input.aliases = Some(vec![alias]);
        let lineup = MultiProviderLineup::new(&input);
        // The alias has a higher priority, so the alias should be acquired first
        for _ in 0..2 {
            should_available!(lineup, 2);
        }
        should_available!(lineup, 1);
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
        should_available!(lineup, 3);
        // Acquire again, and provider should still be available (with remaining capacity)
        should_available!(lineup, 1);
        // // Check that the second alias with priority 2 is considered next
        should_available!(lineup, 2);
        should_available!(lineup, 2);

        should_grace_period!(lineup, 3);
        should_grace_period!(lineup, 1);
        should_grace_period!(lineup, 2);

        should_exhausted!(lineup);

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
        should_available!(lineup, 3);
        // Acquire connection from provider1
        should_available!(lineup, 1);
        // Acquire connection from alias1
        should_available!(lineup, 2);

        // Acquire connection from alias2
        should_grace_period!(lineup, 3);
        // Acquire connection from provider1
        should_grace_period!(lineup, 1);
        // Acquire connection from alias1
        should_grace_period!(lineup, 2);

        // Now, all are exhausted
        should_exhausted!(lineup);
    }

    // Test acquiring a connection when there is available capacity
    #[test]
    fn test_acquire_when_capacity_available() {
        let cfg = create_config_input(1, "provider5_1", 1, 2);
        let lineup = SingleProviderLineup::new(&cfg);

        // First acquire attempt should succeed
        should_available!(lineup, 1);
        // Second acquire attempt should succeed as well
        should_available!(lineup, 1);
        // Third with grace time
        should_grace_period!(lineup, 1);
        // Fourth acquire attempt should fail as the provider is exhausted
        should_exhausted!(lineup);
    }


    // Test releasing a connection
    #[test]
    fn test_release_connection() {
        let cfg = create_config_input(1, "provider7_1", 1, 2);
        let lineup = SingleProviderLineup::new(&cfg);

        // Acquire two connections
        should_available!(lineup, 1);
        should_available!(lineup, 1);
        should_grace_period!(lineup, 1);
        lineup.release("provider7_1");
        should_grace_period!(lineup, 1);
        lineup.release("provider7_1");
        lineup.release("provider7_1");
        should_available!(lineup, 1);
        should_grace_period!(lineup, 1);
        should_exhausted!(lineup);
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
        should_available!(lineup, 1);

        // Test acquiring the second provider
        should_available!(lineup, 2);

        // Test acquiring the first provider
        should_available!(lineup, 1);

        should_grace_period!(lineup, 1);
        should_grace_period!(lineup, 2);

        lineup.release("provider8_1");
        lineup.release("alias_2");
        lineup.release("provider8_1");

        should_available!(lineup, 1);
        should_grace_period!(lineup, 1);
        should_grace_period!(lineup, 2);

        should_exhausted!(lineup);
    }

    // Test concurrent access to `acquire` using multiple threads
    #[test]
    fn test_concurrent_acquire() {
        let cfg = create_config_input(1, "provider9_1", 1, 2);
        let lineup = Arc::new(SingleProviderLineup::new(&cfg));

        let mut handles = vec![];

        let available_count= Arc::new(AtomicU16::new(2));
        let grace_period_count= Arc::new(AtomicU16::new(1));
        let exhausted_count= Arc::new(AtomicU16::new(2));

        for _ in 0..5 {
            let lineup_clone = Arc::clone(&lineup);
            let available = Arc::clone(&available_count);
            let grace_period = Arc::clone(&grace_period_count);
            let exhausted = Arc::clone(&exhausted_count);
            let handle = thread::spawn(move || {
                // Each thread tries to acquire a connection
                match lineup_clone.acquire() {
                    ProviderAllocation::Exhausted => exhausted.fetch_sub(1, Ordering::SeqCst),
                    ProviderAllocation::Available(_) => available.fetch_sub(1, Ordering::SeqCst),
                    ProviderAllocation::GracePeriod(_) => grace_period.fetch_sub(1, Ordering::SeqCst),
                }

            });
            handles.push(handle);
        }

        // Join all threads to ensure completion
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(exhausted_count.load(Ordering::SeqCst), 0);
        assert_eq!(available_count.load(Ordering::SeqCst), 0);
        assert_eq!(grace_period_count.load(Ordering::SeqCst), 0);
    }
}

