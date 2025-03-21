use std::sync::atomic::{AtomicBool, Ordering};

/// A flag that is initially active (`true`) and can only be disabled once.
/// Once the flag is disabled by calling `disable()`, it remains inactive (`false`) forever.
///
/// ## Use Case
/// This type is useful when you need a one-way toggle to mark a resource or state as "finalized",
/// "shut down", or "disabled".
///
/// ## Example
/// ```rust
/// let flag = AtomicOnceFlag::new();
/// assert!(flag.is_active());
/// flag.disable();
/// assert!(!flag.is_active());
#[derive(Debug)]
pub struct AtomicOnceFlag {
    enabled: AtomicBool,
}

impl Default for AtomicOnceFlag {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicOnceFlag {
    /// Creates a new `AtomicOnceFlag`.
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
        }
    }

    /// Disables the flag. After calling this method, `is_active()` will always return `false`.
    ///
    /// This operation is atomic and uses the specified memory ordering.
    pub fn notify(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }

    /// Checks if the flag is still active.
    ///
    /// Returns `true` if the flag is active (initial state). Returns `false` if the flag has been disabled.
    pub fn is_active(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}