use std::collections::HashMap;
use std::sync::Arc;
use std::{fmt, io};
use std::path::{Path, PathBuf};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::tuliprox_error::str_to_io_error;

#[derive(Clone)]
pub struct FileLockManager {
    locks: Arc<Mutex<HashMap<PathBuf, Arc<RwLock<()>>>>>,
}

impl FileLockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Acquires a read lock for the specified file and returns a FileReadGuard.
    pub async fn read_lock(&self, path: &Path) -> FileReadGuard {
        let file_lock = self.get_or_create_lock(path).await;
        let guard = file_lock.read().await;
        // Clone the Arc to avoid moving `file_lock` out, as it is still borrowed by `guard`
        FileReadGuard::new(Arc::clone(&file_lock), guard)
    }

    // Acquires a write lock for the specified file and returns a FileWriteGuard.
    pub async fn write_lock(&self, path: &Path) -> FileWriteGuard {
        let file_lock = self.get_or_create_lock(path).await;
        let guard = file_lock.write().await;
        // Clone the Arc to avoid moving `file_lock` out, as it is still borrowed by `guard`
        FileWriteGuard::new(Arc::clone(&file_lock), guard)
    }

    // Tries to acquire a write lock for the specified file and returns a FileWriteGuard.
    pub async fn try_write_lock(&self, path: &Path) -> io::Result<FileWriteGuard> {
        let file_lock = self.get_or_create_lock(path).await;
        let guard = file_lock.try_write();
        match guard {
            // Clone the Arc to avoid moving `file_lock` out, as it is still borrowed by `guard`
            Ok(lock_guard) => Ok(FileWriteGuard::new(Arc::clone(&file_lock), lock_guard)),
            Err(_) => Err(str_to_io_error("Failed to acquire write lock"))
        }
    }


    // Helper function: retrieves or creates a lock for a file.
    async fn get_or_create_lock(&self, path: &Path) -> Arc<RwLock<()>> {
        let mut locks = self.locks.lock().await;

        if let Some(lock) = locks.get(path) {
            return lock.clone();
        }

        let file_lock = Arc::new(RwLock::new(()));
        locks.insert(path.to_path_buf(), file_lock.clone());
        drop(locks);
        file_lock
    }
}

impl Default for FileLockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for FileLockManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLockManager")
            // .field("locks", &self.locks.lock().await.keys().collect::<Vec<_>>())
            .finish()
    }
}

// Define FileReadGuard to hold both the lock reference and the actual read guard.
#[derive(Clone)]
#[allow(dead_code)]
pub struct FileReadGuard {
    lock: Arc<RwLock<()>>,
    guard: Arc<RwLockReadGuard<'static, ()>>,
}

impl FileReadGuard {
    pub fn new(lock: Arc<RwLock<()>>, guard: RwLockReadGuard<'_, ()>) -> Self {
        // Convert the lifetime of `guard` to 'static by transmuting.
        let static_guard: RwLockReadGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
        Self {
            lock,
            guard: Arc::new(static_guard),
        }
    }
}

// Define FileWriteGuard to hold both the lock reference and the actual write guard.
#[derive(Clone)]
#[allow(dead_code)]
pub struct FileWriteGuard {
    lock: Arc<RwLock<()>>,
    guard: Arc<RwLockWriteGuard<'static, ()>>,
}

impl FileWriteGuard {
    pub fn new(lock: Arc<RwLock<()>>, guard: RwLockWriteGuard<'_, ()>) -> Self {
        // Convert the lifetime of `guard` to 'static by transmuting.
        let static_guard: RwLockWriteGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
        Self {
            lock,
            guard: Arc::new(static_guard),
        }
    }
}
