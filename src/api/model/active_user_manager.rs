use std::collections::HashMap;
use std::sync::Arc;
use jsonwebtoken::get_current_timestamp;
use log::{info, trace};
use tokio::sync::RwLock;
use crate::model::api_proxy::UserConnectionPermission;

// TODO this should be bound to grace_period, because grace_period can be more than 2 secs
const GRACE_TIME_CHECK: u64 = 2;

pub struct UserConnectionGuard {
    manager: Arc<ActiveUserManager>,
    username: String,
}
impl Drop for UserConnectionGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let username = self.username.clone();
        tokio::spawn(async move {
            manager.remove_connection(&username).await;
        });
    }
}

struct UserConnectionData {
    connections: u32,
    granted_grace: u32,
    grace_ts: u64,
}

impl UserConnectionData {
    fn new() -> Self {
        Self {
            connections: 1,
            granted_grace: 0,
            grace_ts: 0,
        }
    }
}

pub struct ActiveUserManager {
    log_active_user: bool,
    user: Arc<RwLock<HashMap<String, UserConnectionData>>>,
}

impl Default for ActiveUserManager {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ActiveUserManager {
    pub fn new(log_active_user: bool) -> Self {
        Self {
            log_active_user,
            user: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn clone_inner(&self) -> Self {
        Self {
            log_active_user: self.log_active_user,
            user: Arc::clone(&self.user),
        }
    }

    pub async fn user_connections(&self, username: &str) -> u32 {
        if let Some(connection_data) = self.user.read().await.get(username) {
            return connection_data.connections;
        }
        0
    }

    pub async fn connection_permission(&self, username: &str, max_connections: u32, grace_period: bool) -> UserConnectionPermission {
        if let Some(connection_data) = self.user.write().await.get_mut(username) {
            let current_connections = connection_data.connections;
            if connection_data.granted_grace > 0 && (get_current_timestamp() - connection_data.grace_ts) > GRACE_TIME_CHECK  {
                trace!("User access denied, grace exhausted, too many connections: {username}");
                return UserConnectionPermission::Exhausted;
            }
            let extra_con = u32::from(grace_period);
            if current_connections < max_connections + extra_con {
                connection_data.granted_grace += 1;
                connection_data.grace_ts = get_current_timestamp();
                return UserConnectionPermission::GracePeriod;
            }
            if current_connections >= max_connections {
                trace!("User access denied, too many connections: {username}");
                return UserConnectionPermission::Exhausted;
            }
            connection_data.granted_grace = 0;
        }
        UserConnectionPermission::Allowed
    }

    pub async fn active_users(&self) -> usize {
        self.user.read().await.len()
    }

    pub async fn active_connections(&self) -> usize {
        self.user.read().await.values().map(|c| c.connections as usize).sum()
    }

    pub async fn add_connection(&self, username: &str) -> UserConnectionGuard {
        let mut lock = self.user.write().await;
        if let Some(connection_data) = lock.get_mut(username) {
            connection_data.connections += 1;
        } else {
            lock.insert(username.to_string(), UserConnectionData::new());
        }
        drop(lock);

        self.log_active_user().await;

        UserConnectionGuard {
            manager: Arc::new(self.clone_inner()),
            username: username.to_string(),
        }
    }

    async fn remove_connection(&self, username: &str) {
        let mut lock = self.user.write().await;
        if let Some(connection_data) = lock.get_mut(username) {
            connection_data.connections -= 1;
            if connection_data.connections == 0 {
                lock.remove(username);
            }
        }
        drop(lock);

        self.log_active_user().await;
    }

    async fn log_active_user(&self) {
        if self.log_active_user {
            let user_count = self.active_users().await;
            let user_connection_count = self.active_connections().await;
            info!("Active Users: {user_count}, Active User Connections: {user_connection_count}");
        }
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
