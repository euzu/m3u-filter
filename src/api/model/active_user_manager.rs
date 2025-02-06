use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct ActiveUserManager {
    pub user: HashMap<String, AtomicU32>,
}

impl Default for ActiveUserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveUserManager {
    pub fn new() -> Self {
        Self {
            user: HashMap::new(),
        }
    }

    pub fn user_connections(&self, username: &str) -> u32 {
        if let Some(counter) = self.user.get(username) {
            return counter.load(std::sync::atomic::Ordering::Relaxed);
        }
        0
    }

    pub fn active_users(&self) -> usize {
        self.user.len()
    }

    pub fn active_connections(&self) -> usize {
        self.user.values().map(|c| c.load(Ordering::Relaxed) as usize).sum()
    }

    pub fn add_connection(&mut self, username: &str) {
        if let Some(counter) = self.user.get(username) {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.user.insert(username.to_string(), AtomicU32::new(1));
        }
    }

    pub fn remove_connection(&mut self, username: &str) {
        if let Some(counter) = self.user.get(username) {
            if counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) == 1 {
                self.user.remove(username);
            }
        }
    }
}