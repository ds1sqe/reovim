//! Window identifier type.
//!
//! Linux equivalent: Window handle identifier (mechanism only)
//!
//! This module provides a simple identifier type for windows, similar to how
//! `BufferId` provides an identifier for buffers.

use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Unique window identifier.
///
/// Windows are identified by a unique, monotonically increasing ID.
/// This is a lightweight handle that can be cheaply cloned and compared.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::WindowId;
///
/// let window1 = WindowId::new();
/// let window2 = WindowId::new();
///
/// assert_ne!(window1, window2);
/// assert_eq!(window1, window1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

/// Global window ID counter.
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

impl WindowId {
    /// Create a new unique window ID.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a window ID from a raw value.
    ///
    /// Used when converting from external window ID representations.
    #[must_use]
    pub const fn from_raw(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "window:{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_id_new() {
        let id1 = WindowId::new();
        let id2 = WindowId::new();
        assert_ne!(id1, id2);
        assert!(id2.as_usize() > id1.as_usize());
    }

    #[test]
    fn test_window_id_clone() {
        let id1 = WindowId::new();
        let id2 = id1;
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_window_id_display() {
        let id = WindowId(42);
        assert_eq!(format!("{id}"), "window:42");
    }

    #[test]
    fn test_window_id_default() {
        let id = WindowId::default();
        assert!(id.as_usize() > 0);
    }

    #[test]
    fn test_window_id_from_raw() {
        let id = WindowId::from_raw(42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    fn test_window_id_ordering() {
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        // PartialOrd
        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);

        // Ord (for sorting)
        let mut ids = vec![id3, id1, id2];
        ids.sort();
        assert_eq!(ids, vec![id1, id2, id3]);
    }
}
