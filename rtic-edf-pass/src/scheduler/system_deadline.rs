use core::sync::atomic::Ordering;

use crate::types::{AtomicTimestamp, Timestamp};

/// The global system deadline representing the minimum deadline task currently
/// executing
pub struct SystemDeadline(AtomicTimestamp);

impl SystemDeadline {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(AtomicTimestamp::new(Timestamp::MAX))
    }

    /// Get the system deadline
    #[inline]
    pub(super) fn load(&self) -> Timestamp {
        self.0.load(Ordering::Acquire)
    }

    /// Get the system deadline
    #[inline]
    pub(super) fn store(&self, new_dl: Timestamp) {
        self.0.store(new_dl, Ordering::Release)
    }

    /// Replaces the system deadline with the deadline provided, and returns the
    /// old deadline
    #[inline]
    #[must_use]
    pub(super) fn swap(&self, new_dl: Timestamp) -> Timestamp {
        self.0.swap(new_dl, Ordering::Release)
    }
}
