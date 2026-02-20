#![expect(clippy::new_without_default)]

use core::sync::atomic::Ordering;

use crate::types::{AtomicTimestamp, Timestamp};

/// A 1-deep message-passing buffer, which holds one slot for each dispatcher
/// priority in the system.
///
/// We only need one slot per priority, because for each priority, only one
/// dispatcher is guaranteed to run at any instant
pub struct RunQueue<const N: usize>([AtomicTimestamp; N]);

impl<const N: usize> RunQueue<N> {
    pub const fn new() -> Self {
        Self([const { AtomicTimestamp::new(Timestamp::MAX) }; N])
    }

    pub fn get(&self, idx: u16) -> Timestamp {
        #[cfg(debug_assertions)]
        let slot = self
            .0
            .get(idx as usize)
            .expect("BUG: dispatcher idx doesn't exist");

        #[cfg(not(debug_assertions))]
        let slot = unsafe { self.0.get_unchecked(idx as usize) };

        slot.load(Ordering::Acquire)
    }

    /// Insert a pending task to the queue for later retrieval
    pub(super) fn insert(&self, task_dl: Timestamp, idx: u16) {
        #[cfg(debug_assertions)]
        let slot = self
            .0
            .get(idx as usize)
            .expect("BUG: dispatcher idx doesn't exist");

        #[cfg(not(debug_assertions))]
        let slot = unsafe { self.0.get_unchecked(idx as usize) };

        slot.store(task_dl, Ordering::Release);
    }
}
