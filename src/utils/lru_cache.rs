use log::{debug, error};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{self};
use std::path::PathBuf;

type FetcherFn = dyn Fn(&str) -> io::Result<(PathBuf, usize)> + 'static;

struct LRUResourceCache {
    fetch_resource: Box<FetcherFn>,
    capacity: usize,  // Maximum size in bytes
    current_size: usize,  // Current size in bytes
    cache: HashMap<String, (PathBuf, usize)>,
    usage_order: VecDeque<String>,
}

impl LRUResourceCache {
    fn new(capacity: usize, fetcher_fn: Box<FetcherFn>) -> Self {
        Self {
            capacity,
            current_size: 0,
            cache: HashMap::<String, (PathBuf, usize)>::new(),
            usage_order: VecDeque::new(),
            fetch_resource: fetcher_fn,
        }
    }

    fn download_resource(&self, url: &str) -> io::Result<(PathBuf, usize)> {
        (self.fetch_resource)(url)
    }

    fn get_content(&mut self, url: &str) -> io::Result<PathBuf> {
        if let Some((path, size)) = self.cache.get(url) {
            if path.exists() {
                // Move to the end of the queue
                self.usage_order.retain(|e| e != url);   // remove from queue
                self.usage_order.push_back(url.to_string());  // add to the to end
                return Ok(path.clone());
            }
            // this should not happen, someone deleted the file manually and the cache is not in sync
            self.current_size -= size;
            self.cache.remove(url);
            // yes this is really frustrating.
            self.usage_order.retain(|key| key != url);
        }

        match self.download_resource(url) {
            Ok((filepath, file_size)) => {
                debug!("Added file to cache: {}", filepath.to_string_lossy());
                self.cache.insert(url.to_string(), (filepath.clone(), file_size));
                self.usage_order.push_back(url.to_string());
                self.current_size += file_size;
                self.evict_if_needed();
                Ok(filepath)
            },
            Err(err) => Err(err),
        }
    }

    fn evict_if_needed(&mut self) {
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

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io;
    use std::io::BufWriter;
    use std::path::PathBuf;
    use crate::repository::storage::hash_string_as_hex;
    use crate::utils::lru_cache::LRUResourceCache;

    const PHOTO_URL: &str = "https://dummyimage.com/";
    const PHOTOS: &[&str] = &["300x200/000/fff", "300x200/f00/0ff", "300x200/0f0/f0f"];

    fn download_file(url: &str) -> io::Result<(PathBuf, usize)> {
        match reqwest::blocking::get(url) {
            Ok(mut response) => {
                println!("Downloaded file {url}");
                let key = hash_string_as_hex(&url);
                let mut path = PathBuf::from("/tmp");
                path.push(key);
                let mut writer = BufWriter::new(File::create(&path)?);
                let size = response.copy_to(&mut writer).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
                Ok((path, size as usize))
            },
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
        }
    }

    #[test]
    fn test_1() {
        let mut cache = LRUResourceCache::new(1000, Box::new(download_file));
        for photo in PHOTOS {
            match cache.get_content(format!("{PHOTO_URL}{photo}").as_str()) {
                Ok(path) => {
                    println!("path {path:?}");
                }
                Err(err) => { println!("Failed {err}")}
            }
        }

        for photo in PHOTOS {
            match cache.get_content(format!("{PHOTO_URL}{photo}").as_str()) {
                Ok(path) => {
                    println!("path {path:?}");
                }
                Err(err) => { println!("Failed {err}")}
            }
        }

    }
}
