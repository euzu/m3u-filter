use log::error;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;
use chrono::Local;
use cron::Schedule;
use tokio::sync::RwLock;
use tokio::time::Instant;
use crate::utils::sys_utils::exit;
use crate::model::hls::HlsEntry;

const EXPIRE_DURATION: u64 = 600; // 10 minutes
const TOKEN_MAX: u32 = u32::MAX - u16::MAX  as u32;

fn start_garbage_collector(cache: &Arc<HlsCache>) {
    let cache_clone = cache.clone();
    tokio::spawn(async move {
        match Schedule::from_str("0 */15  *  *  *  *  *") {
            Ok(schedule) => {
                let offset = *Local::now().offset();
                loop {
                    let mut upcoming = schedule.upcoming(offset).take(1);
                    if let Some(datetime) = upcoming.next() {
                        tokio::time::sleep_until(tokio::time::Instant::from(crate::api::scheduler::datetime_to_instant(datetime))).await;
                        cache_clone.gc().await;
                    }
                }
            }
            Err(err) => exit!("Failed to start scheduler: {}", err)
        }
    });
}

pub struct HlsCache {
    pub entries: RwLock<HashMap<u32, HlsEntry>>,
    counter:  AtomicU32,
}

impl HlsCache {
    pub fn garbage_collected() -> Arc<Self> {
        let cache = Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
            counter: AtomicU32::new(1),
        });

        start_garbage_collector(&cache);
        cache
    }

    pub async fn add_entry(&self, entry: HlsEntry) {
        self.entries.write().await.insert(entry.token, entry);
    }

    pub async fn get_entry(&self, token: u32) -> Option<HlsEntry>{
        self.entries.read().await.get(&token).cloned()
    }

    pub async fn gc(&self) {
        let threshold = Instant::now() - Duration::from_secs(EXPIRE_DURATION);
        // Remove all expired elements
        self.entries.write().await.retain(|_, entry| entry.ts > threshold);
    }

    pub fn new_token(&self) -> u32 {
        let token = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if token > TOKEN_MAX {
            self.counter.store(1, std::sync::atomic::Ordering::SeqCst);
            return 1;
        }
        return token;
    }
}