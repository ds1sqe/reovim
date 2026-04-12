//! Clipboard history ring for numbered registers (0-255).
//!
//! Provides a LIFO ring buffer for tracking yank/delete history.
//! Index `0` is the most recent entry, `255` is the oldest.
//!
//! # Capacity
//!
//! Default capacity is 256 entries (the full `u8` range), addressable via
//! `Register::History(u8)`. Entries beyond capacity are discarded.
//!
//! # Per-Client Isolation (#515)
//!
//! Each client owns their own `HistoryRing`. Client A's yank/delete
//! operations do not affect Client B's history.

use std::collections::VecDeque;

use crate::register::RegisterContent;

/// Default capacity for the history ring (registers 0-255).
const DEFAULT_HISTORY_CAPACITY: usize = 256;

/// LIFO ring buffer for yank/delete history (numbered registers 0-255).
///
/// # Example
///
/// ```
/// use reovim_domain_text::*;
///
/// let mut ring = HistoryRing::new();
/// ring.push(RegisterContent::characterwise("first"));
/// ring.push(RegisterContent::characterwise("second"));
///
/// // Most recent is at index 0
/// assert_eq!(ring.get(0).map(|r| r.text.as_str()), Some("second"));
/// assert_eq!(ring.get(1).map(|r| r.text.as_str()), Some("first"));
/// ```
#[derive(Debug, Clone)]
pub struct HistoryRing {
    /// Entries in LIFO order (index 0 = most recent).
    entries: VecDeque<RegisterContent>,
    /// Maximum number of entries to keep.
    capacity: usize,
}

impl HistoryRing {
    /// Create a new history ring with default capacity (256).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: DEFAULT_HISTORY_CAPACITY,
        }
    }

    /// Create a history ring with custom capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push content to the front of the history.
    ///
    /// The new entry becomes register `0`. If the ring is full,
    /// the oldest entry (register 9) is discarded.
    pub fn push(&mut self, content: RegisterContent) {
        self.entries.push_front(content);
        if self.entries.len() > self.capacity {
            self.entries.pop_back();
        }
    }

    /// Get an entry by index (0 = most recent).
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&RegisterContent> {
        self.entries.get(index)
    }

    /// Get an entry by `u8` index (0 = most recent).
    ///
    /// This is the typed accessor for `Register::History(u8)`.
    /// Returns `None` if the index is beyond the current history length.
    #[must_use]
    pub fn get_by_index(&self, index: u8) -> Option<&RegisterContent> {
        self.entries.get(index as usize)
    }

    /// Get an entry by numbered register character ('0'-'9').
    ///
    /// Returns `None` if the character is not a digit or the index
    /// is beyond the current history length.
    #[must_use]
    pub fn get_numbered(&self, n: char) -> Option<RegisterContent> {
        if n.is_ascii_digit() {
            let index = (n as u8 - b'0') as usize;
            self.entries.get(index).cloned()
        } else {
            None
        }
    }

    /// Get the number of entries currently in the ring.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the ring is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the maximum capacity of the ring.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Iterate over entries in order (most recent first).
    pub fn iter(&self) -> impl Iterator<Item = &RegisterContent> {
        self.entries.iter()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for HistoryRing {
    fn default() -> Self {
        Self::new()
    }
}
