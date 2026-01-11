//! Buffer identification.
//!
//! Provides unique, monotonically increasing identifiers for buffers.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Global counter for buffer IDs.
static NEXT_BUFFER_ID: AtomicUsize = AtomicUsize::new(0);

/// Unique identifier for a buffer.
///
/// Buffer IDs are monotonically increasing and never reused within a session.
/// This ensures that buffer references remain unambiguous even after buffers
/// are closed.
///
/// # Example
///
/// ```
/// use reovim_kernel::mm::BufferId;
///
/// let id1 = BufferId::new();
/// let id2 = BufferId::new();
/// assert_ne!(id1, id2);
/// assert!(id1 < id2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

impl BufferId {
    /// Create a new unique buffer ID.
    ///
    /// Each call returns a distinct ID, guaranteed to be unique within
    /// the current process lifetime.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw numeric value.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Create a `BufferId` from a raw value.
    ///
    /// This is primarily useful for testing or deserialization.
    ///
    /// # Warning
    ///
    /// Using this incorrectly may create duplicate IDs. Prefer [`BufferId::new`]
    /// for normal usage.
    #[must_use]
    pub const fn from_raw(value: usize) -> Self {
        Self(value)
    }
}

impl Default for BufferId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BufferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Buffer({})", self.0)
    }
}
