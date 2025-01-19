use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
#[derive(Clone, Debug)]
pub struct AtomicOnceFlag {
    enabled: Arc<AtomicBool>,
    ordering: Ordering,
}

impl AtomicOnceFlag {
    /// Creates a new `AtomicOnceFlag` with the specified memory ordering.
    pub fn with_ordering(ordering: Ordering) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            ordering,
        }
    }

    /// Creates a new `AtomicOnceFlag` with a default memory ordering of `Relaxed`.
    pub fn new() -> Self {
        Self::with_ordering(Ordering::Relaxed)
    }

    /// Disables the flag. After calling this method, `is_active()` will always return `false`.
    ///
    /// This operation is atomic and uses the specified memory ordering.
    pub fn disable(&self) {
        self.enabled.store(false, self.ordering);
    }

    /// Checks if the flag is still active.
    ///
    /// Returns `true` if the flag is active (initial state). Returns `false` if the flag has been disabled.
    pub fn is_active(&self) -> bool {
        self.enabled.load(self.ordering)
    }
}