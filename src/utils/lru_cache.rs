use crate::repository::storage::hash_string_as_hex;
use crate::utils::file_utils::traverse_dir;
use async_std::sync::RwLock;
use log::{debug, error};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use crate::debug_if_enabled;
use crate::utils::size_utils::human_readable_byte_size;

pub struct LRUResourceCache {
    capacity: usize,  // Maximum size in bytes
    cache_dir: PathBuf,
    current_size: usize,  // Current size in bytes
    cache: HashMap<String, (PathBuf, usize)>,
    usage_order: VecDeque<String>,
    lock: RwLock<()>,
}

impl LRUResourceCache {
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

    pub async fn scan(&mut self) -> std::io::Result<()> {
        let _write_lock = self.lock.write().await;
        let mut visit = |entry: &std::fs::DirEntry, metadata: &std::fs::Metadata| {
            self.current_size += usize::try_from(metadata.len()).unwrap_or(0);
            self.usage_order.push_back(entry.file_name().to_string_lossy().to_string());
        };
        let result = traverse_dir(&self.cache_dir, &mut visit);
        debug_if_enabled!("Cache scanned, current size {}", human_readable_byte_size(self.current_size as u64));
        result
    }

    pub async fn add_content(&mut self, url: &str, file_size: usize) -> std::io::Result<PathBuf> {
        let key = hash_string_as_hex(url);
        {
            let mut path = self.cache_dir.clone();
            path.push(&key);
            {
                let _write_lock = self.lock.write().await;
                debug!("Added file to cache: {}", &path.to_string_lossy());
                self.cache.insert(key.clone(), (path.clone(), file_size));
                self.usage_order.push_back(key);
                self.current_size += file_size;
            }
            if self.current_size > self.capacity {
                self.evict_if_needed().await;
            }
            Ok(path)
        }
    }

    pub fn store_path(&self, url: &str) -> PathBuf {
        let key = hash_string_as_hex(url);
        let mut path = self.cache_dir.clone();
        path.push(&key);
        path
    }

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

// #[cfg(test)]
// mod tests {
//     use crate::utils::lru_cache::LRUResourceCache;
//     use std::fs::File;
//     use std::future::Future;
//     use std::io;
//     use std::io::BufWriter;
//     use std::path::PathBuf;
//
//     const PHOTO_URL: &str = "https://dummyimage.com/";
//     const PHOTOS: &[&str] = &["300x200/000/fff", "300x200/f00/0ff", "300x200/0f0/f0f"];
//
//     fn download_file(url: &str, key: &str) -> io::Result<(PathBuf, usize)> {
//         match reqwest::blocking::get(url) {
//             Ok(mut response) => {
//                 println!("Downloaded file {url}");
//                 let mut path = PathBuf::from("/tmp");
//                 path.push(key);
//                 let mut writer = BufWriter::new(File::create(&path)?);
//                 let size = response.copy_to(&mut writer).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
//                 Ok((path, size as usize))
//             }
//             Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
//         }
//     }
//
//     #[cfg(target_os = "linux")]
//     #[test]
//     fn test_1() {
//         actix_rt::task::spawn_blocking(move ||{
//             let mut cache = LRUResourceCache::new(1000, &PathBuf::from("/tmp"), Box::new(download_file));
//             for photo in PHOTOS {
//                 match cache.get_content(format!("{PHOTO_URL}{photo}").as_str()).await {
//                     Ok(path) => {
//                         println!("path {path:?}");
//                     }
//                     Err(err) => { println!("Failed {err}") }
//                 }
//             }
//
//             for photo in PHOTOS {
//                 match cache.get_content(format!("{PHOTO_URL}{photo}").as_str()).await {
//                     Ok(path) => {
//                         println!("path {path:?}");
//                     }
//                     Err(err) => { println!("Failed {err}") }
//                 }
//             }
//         });
//
//     }
// }
