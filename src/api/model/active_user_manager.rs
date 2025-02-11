use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use parking_lot::RwLock;

pub struct ActiveUserManager {
    pub user: RwLock<HashMap<String, AtomicU32>>,
}

impl Default for ActiveUserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveUserManager {
    pub fn new() -> Self {
        Self {
            user: RwLock::new(HashMap::new()),
        }
    }

    pub fn user_connections(&self, username: &str) -> u32 {
        if let Some(counter) = self.user.read().get(username) {
            return counter.load(std::sync::atomic::Ordering::SeqCst);
        }
        0
    }

    pub fn active_users(&self) -> usize {
        self.user.read().len()
    }

    pub fn active_connections(&self) -> usize {
        self.user.read().values().map(|c| c.load(Ordering::SeqCst) as usize).sum()
    }

    pub fn add_connection(&self, username: &str) -> (usize, usize) {
        {
            let mut lock = self.user.write();
            if let Some(counter) = lock.get(username) {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            } else {
                lock.insert(username.to_string(), AtomicU32::new(1));
            }
            drop(lock);
        }
        (self.active_users(), self.active_connections())
    }

    pub fn remove_connection(&self, username: &str) -> (usize, usize) {
        {
            let mut lock = self.user.write();
            if let Some(counter) = lock.get(username) {
                if counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                    lock.remove(username);
                }
            }
            drop(lock);
        }
        (self.active_users(), self.active_connections())
    }
}