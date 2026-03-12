//! Tab page identifier type.
//!
//! Linux equivalent: Tab handle identifier (mechanism only)
//!
//! This module provides a simple identifier type for tab pages, following the
//! same pattern as `WindowId` and `BufferId`.

use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Unique tab page identifier.
///
/// Tab pages are identified by a unique, monotonically increasing ID.
/// This is a lightweight handle that can be cheaply cloned and compared.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::TabId;
///
/// let tab1 = TabId::new();
/// let tab2 = TabId::new();
///
/// assert_ne!(tab1, tab2);
/// assert_eq!(tab1, tab1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(usize);

/// Global tab ID counter.
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

impl TabId {
    /// Create a new unique tab ID.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a tab ID from a raw value.
    ///
    /// Used when converting from external tab ID representations.
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

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab:{}", self.0)
    }
}

#[cfg(test)]
#[path = "tests/tab_id.rs"]
mod tests;
