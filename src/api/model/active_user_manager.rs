use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;

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

    pub async fn user_connections(&self, username: &str) -> u32 {
        if let Some(counter) = self.user.read().await.get(username) {
            return counter.load(Ordering::Acquire);
        }
        0
    }

    pub async fn active_users(&self) -> usize {
        self.user.read().await.len()
    }

    pub async fn active_connections(&self) -> usize {
        self.user.read().await.values().map(|c| c.load(Ordering::Acquire) as usize).sum()
    }

    pub async fn add_connection(&self, username: &str) -> (usize, usize) {
        let mut lock = self.user.write().await;
        if let Some(counter) = lock.get(username) {
            counter.fetch_add(1, Ordering::AcqRel);
        } else {
            lock.insert(username.to_string(), AtomicU32::new(1));
        }
        drop(lock);
        (self.active_users().await, self.active_connections().await)
    }

    pub async fn remove_connection(&self, username: &str) -> (usize, usize) {
        let mut lock = self.user.write().await;
        if let Some(counter) = lock.get(username) {
            if counter.fetch_sub(1, Ordering::AcqRel) == 1 {
                lock.remove(username);
            }
        }
        drop(lock);
        (self.active_users().await, self.active_connections().await)
    }
}