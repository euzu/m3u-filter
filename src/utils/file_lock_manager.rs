use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{fmt, io};
use std::path::{Path, PathBuf};

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
    pub fn read_lock(&self, path: &Path) -> io::Result<FileReadGuard> {
        let file_lock = self.get_or_create_lock(path)?;
        let guard = file_lock.read().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to acquire read lock")
        })?;
        // Clone the Arc to avoid moving `file_lock` out, as it is still borrowed by `guard`
        Ok(FileReadGuard::new(Arc::clone(&file_lock), guard))
    }

    // Acquires a write lock for the specified file and returns a FileWriteGuard.
    pub fn write_lock(&self, path: &Path) -> io::Result<FileWriteGuard> {
        let file_lock = self.get_or_create_lock(path)?;
        let guard = file_lock.write().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to acquire write lock")
        })?;
        // Clone the Arc to avoid moving `file_lock` out, as it is still borrowed by `guard`
        Ok(FileWriteGuard::new(Arc::clone(&file_lock), guard))
    }

    // Helper function: retrieves or creates a lock for a file.
    fn get_or_create_lock(&self, path: &Path) -> io::Result<Arc<RwLock<()>>> {
        let mut locks = self.locks.lock().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to acquire lock on lock manager")
        })?;

        if let Some(lock) = locks.get(path) {
            return Ok(lock.clone());
        }

        let file_lock = Arc::new(RwLock::new(()));
        locks.insert(path.to_path_buf(), file_lock.clone());
        drop(locks);
        Ok(file_lock)
    }
}

impl Default for FileLockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for FileLockManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Acquire the lock to safely access the HashMap
        let locks = self.locks.lock().unwrap();
        let keys: Vec<_> = locks.keys().collect();
        f.debug_struct("FileLockManager")
            .field("locks", &keys)
            .finish()
    }
}

// Define FileReadGuard to hold both the lock reference and the actual read guard.
#[allow(dead_code)]
pub struct FileReadGuard {
    lock: Arc<RwLock<()>>,
    guard: RwLockReadGuard<'static, ()>,
}

impl FileReadGuard {
    pub fn new(lock: Arc<RwLock<()>>, guard: RwLockReadGuard<'_, ()>) -> Self {
        // Convert the lifetime of `guard` to 'static by transmuting.
        let static_guard: RwLockReadGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
        Self {
            lock,
            guard: static_guard,
        }
    }
}

// Define FileWriteGuard to hold both the lock reference and the actual write guard.
#[allow(dead_code)]
pub struct FileWriteGuard {
    lock: Arc<RwLock<()>>,
    guard: RwLockWriteGuard<'static, ()>,
}

impl FileWriteGuard {
    pub fn new(lock: Arc<RwLock<()>>, guard: RwLockWriteGuard<'_, ()>) -> Self {
        // Convert the lifetime of `guard` to 'static by transmuting.
        let static_guard: RwLockWriteGuard<'static, ()> = unsafe { std::mem::transmute(guard) };
        Self {
            lock,
            guard: static_guard,
        }
    }
}
