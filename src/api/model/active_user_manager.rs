use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use log::{trace};
use tokio::sync::RwLock;
use crate::model::api_proxy::UserConnectionPermission;

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
            return counter.load(Ordering::SeqCst);
        }
        0
    }

    pub async fn connection_permission(&self, username: &str, max_connections: u32, grace_period: bool) -> UserConnectionPermission {
        if let Some(counter) = self.user.read().await.get(username) {
            let current_connections = counter.load(Ordering::SeqCst);
            let extra_con = u32::from(grace_period);
            if current_connections < max_connections + extra_con {
                return UserConnectionPermission::GracePeriod;
            }
            if current_connections >= max_connections {
                trace!("User access denied, too many connections: {username}");
                return UserConnectionPermission::Exhausted;
            }
        }
        UserConnectionPermission::Allowed
    }

    pub async fn active_users(&self) -> usize {
        self.user.read().await.len()
    }

    pub async fn active_connections(&self) -> usize {
        self.user.read().await.values().map(|c| c.load(Ordering::SeqCst) as usize).sum()
    }

    pub async fn add_connection(&self, username: &str) -> (usize, usize) {
        let mut lock = self.user.write().await;
        if let Some(counter) = lock.get(username) {
            counter.fetch_add(1, Ordering::SeqCst);
        } else {
            lock.insert(username.to_string(), AtomicU32::new(1));
        }
        drop(lock);
        (self.active_users().await, self.active_connections().await)
    }

    pub async fn remove_connection(&self, username: &str) -> (usize, usize) {
        let mut lock = self.user.write().await;
        if let Some(counter) = lock.get(username) {
            let new_count = counter.fetch_sub(1, Ordering::SeqCst) - 1;
            if new_count == 0 {
                lock.remove(username);
            }
        }
        drop(lock);
        (self.active_users().await, self.active_connections().await)
    }
}

//
// mod tests {
//     use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
//     use std::time::Instant;
//     use std::thread;
//
//     fn benchmark(ordering: Ordering, iterations: usize) -> u128 {
//         let counter = Arc::new(AtomicUsize::new(0));
//         let start = Instant::now();
//
//         let handles: Vec<_> = (0..32)
//             .map(|_| {
//                 let counter_ref = Arc::clone(&counter);
//                 thread::spawn(move || {
//                     for _ in 0..iterations {
//                         counter_ref.fetch_add(1, ordering);
//                     }
//                 })
//             })
//             .collect();
//
//         for handle in handles {
//             handle.join().unwrap();
//         }
//
//         let duration = start.elapsed();
//         duration.as_millis()
//     }
//
//     #[test]
//     fn test_ordering() {
//         let iterations = 1_000_000;
//
//         let time_acqrel = benchmark(Ordering::SeqCst, iterations);
//         println!("AcqRel: {} ms", time_acqrel);
//
//         let time_seqcst = benchmark(Ordering::SeqCst, iterations);
//         println!("SeqCst: {} ms", time_seqcst);
//     }
//
// }
