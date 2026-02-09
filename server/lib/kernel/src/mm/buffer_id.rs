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
/// use reovim_kernel::api::v1::*;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_zero() {
        let id = BufferId::from_raw(0);
        assert_eq!(id.as_usize(), 0);
    }

    #[test]
    fn from_raw_max() {
        let id = BufferId::from_raw(usize::MAX);
        assert_eq!(id.as_usize(), usize::MAX);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn display_zero() {
        let id = BufferId::from_raw(0);
        assert_eq!(format!("{id}"), "Buffer(0)");
    }

    #[test]
    fn default_creates_unique_id() {
        let id1 = BufferId::default();
        let id2 = BufferId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn hash_consistent() {
        use std::collections::HashSet;

        let id1 = BufferId::from_raw(10);
        let id2 = BufferId::from_raw(20);
        let id1_dup = BufferId::from_raw(10);

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id1_dup); // duplicate of id1

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn clone_and_copy() {
        let id = BufferId::from_raw(99);
        let cloned = id;
        assert_eq!(id, cloned);
        assert_eq!(id.as_usize(), cloned.as_usize());
    }

    #[test]
    fn ordering_from_raw() {
        let a = BufferId::from_raw(1);
        let b = BufferId::from_raw(2);
        let c = BufferId::from_raw(3);
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
        assert!(c > a);
    }

    #[test]
    fn eq_same_raw_value() {
        let a = BufferId::from_raw(42);
        let b = BufferId::from_raw(42);
        assert_eq!(a, b);
    }

    #[test]
    fn ne_different_raw_value() {
        let a = BufferId::from_raw(1);
        let b = BufferId::from_raw(2);
        assert_ne!(a, b);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn debug_format() {
        let id = BufferId::from_raw(7);
        let debug = format!("{id:?}");
        assert!(debug.contains("BufferId"));
        assert!(debug.contains('7'));
    }
}
