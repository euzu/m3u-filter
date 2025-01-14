use crate::debug_if_enabled;
use crate::repository::storage::hash_string_as_hex;
use crate::utils::file_utils::traverse_dir;
use crate::utils::size_utils::human_readable_byte_size;
use async_std::sync::RwLock;
use log::{debug, error, info};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

/// `LRUResourceCache`
///
/// A least-recently-used (LRU) file-based resource cache that stores files in a directory on disk,
/// automatically managing their lifecycle based on a specified maximum cache size. The cache evicts
/// the least recently used files when the size limit is exceeded.
///
/// # Fields
/// - `capacity`: The maximum cache size in bytes. Once the cache size exceeds this value, files are evicted.
/// - `cache_dir`: The directory where cached files are stored.
/// - `current_size`: The current total size of all files in the cache, in bytes.
/// - `cache`: A `HashMap` that maps a unique key to a tuple containing the file path and its size.
/// - `usage_order`: A `VecDeque` that tracks the access order of keys, with the oldest at the front.
/// - `lock`: An `RwLock` to ensure thread-safe access to the cache during read and write operations.
pub struct LRUResourceCache {
    capacity: usize,  // Maximum size in bytes
    cache_dir: PathBuf,
    current_size: usize,  // Current size in bytes
    cache: HashMap<String, (PathBuf, usize)>,
    usage_order: VecDeque<String>,
    lock: RwLock<()>,
}

impl LRUResourceCache {
    ///   - Creates a new `LRUResourceCache` instance.
    ///   - Arguments:
    ///     - `capacity`: The maximum size of the cache in bytes.
    ///     - `cache_dir`: The directory path where cached files are stored.
    ///
    pub fn new(capacity: usize, cache_dir: &Path) -> Self {
        Self {
            capacity,
            cache_dir: PathBuf::from(cache_dir),
            current_size: 0,
            cache: HashMap::<String, (PathBuf, usize)>::new(),
            usage_order: VecDeque::new(),
            lock: RwLock::new(()),
        }
    }

    /// - Scans the cache directory and populates the internal data structures with existing files and their sizes.
    /// - Updates the `current_size` and `usage_order` fields based on the scanned files.
    ///   The use/access order is not restored!!!
    pub async fn scan(&mut self) -> std::io::Result<()> {
        let _write_lock = self.lock.write().await;
        let mut visit = |entry: &std::fs::DirEntry, metadata: &std::fs::Metadata| {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let key = String::from(file_name.to_string_lossy());
                let file_size = usize::try_from(metadata.len()).unwrap_or(0);
                // we need to duplicate because of closure we cant call insert_to_cache
                {  // insert_to_cache
                    let mut path = self.cache_dir.clone();
                    path.push(&key);
                    debug!("Added file to cache: {}", &path.to_string_lossy());
                    self.cache.insert(key.clone(), (path.clone(), file_size));
                    self.usage_order.push_back(key);
                    self.current_size += file_size;
                }
            }
        };
        let result = traverse_dir(&self.cache_dir, &mut visit);
        info!("Cache scanned, current size {}", human_readable_byte_size(self.current_size as u64));
        result
    }

    ///   - Adds a new file to the cache.
    ///   - Evicts the least recently used files if the cache size exceeds the capacity after the addition.
    ///   - Arguments:
    ///     - `url`: The unique identifier for the file.
    ///     - `file_size`: The size of the file in bytes.
    ///   - Returns:
    ///     - The `PathBuf` where the file is stored.
    pub async fn add_content(&mut self, url: &str, file_size: usize) -> std::io::Result<PathBuf> {
        let key = hash_string_as_hex(url);
        let path = {
            self.insert_to_cache(key, file_size).await
        };
        if self.current_size > self.capacity {
            self.evict_if_needed().await;
        }
        Ok(path)
    }

    async fn insert_to_cache(&mut self, key: String, file_size: usize) -> PathBuf {
        let _write_lock = self.lock.write().await;
        let mut path = self.cache_dir.clone();
        path.push(&key);
        debug!("Added file to cache: {}", &path.to_string_lossy());
        self.cache.insert(key.clone(), (path.clone(), file_size));
        self.usage_order.push_back(key);
        self.current_size += file_size;
        path
    }

    pub fn store_path(&self, url: &str) -> PathBuf {
        let key = hash_string_as_hex(url);
        let mut path = self.cache_dir.clone();
        path.push(&key);
        path
    }

    ///   - Retrieves a file from the cache if it exists.
    ///   - Moves the file's key to the end of the usage queue to mark it as recently used.
    ///   - Arguments:
    ///     - `url`: The unique identifier for the file.
    ///   - Returns:
    ///     - The `PathBuf` of the file if it exists; `None` otherwise.
    pub async fn get_content(&mut self, url: &str) -> Option<PathBuf> {
        let key = hash_string_as_hex(url);
        {
            let _read_lock = self.lock.read().await;
            if let Some((path, size)) = self.cache.get(&key) {
                if path.exists() {
                    // Move to the end of the queue
                    self.usage_order.retain(|k| k != &key);   // remove from queue
                    self.usage_order.push_back(key);  // add to the to end
                    return Some(path.clone());
                }
                {
                    // this should not happen, someone deleted the file manually and the cache is not in sync
                    let _write_lock = self.lock.write().await;
                    self.current_size -= size;
                    self.cache.remove(&key);
                    self.usage_order.retain(|k| k != &key);
                }
            }
        }
        None
    }

    async fn evict_if_needed(&mut self) {
        let _write_lock = self.lock.write().await;
        // if the cache size is to small and one element exceeds the size than the cache won't work, we ignore this
        while self.current_size > self.capacity {
            if let Some(oldest_file) = self.usage_order.pop_front() {
                if let Some((file, size)) = self.cache.remove(&oldest_file) {
                    self.current_size -= size;
                    if let Err(err) = fs::remove_file(&file) {
                        error!("Failed to delete cached file {} {err}", file.to_string_lossy());
                    } else {
                        debug!("Removed file from cache: {}", file.to_string_lossy());
                    }
                }
            }
        }
    }
}

