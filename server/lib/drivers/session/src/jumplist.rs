//! Jump list for cursor position history.
//!
//! Linux equivalent: Navigation history like shell directory stack
//!
//! This module provides the [`Jumplist`] data structure for tracking
//! "major" cursor position changes, enabling Ctrl-O (jump older) and
//! Ctrl-I (jump newer) navigation.
//!
//! # Design Philosophy
//!
//! The jumplist is a **mechanism** for tracking positions. Policy decisions
//! (what constitutes a "jump") are made by modules/commands that call `push()`.
//!
//! # Example
//!
//! ```
//! use reovim_driver_session::{JumpEntry, Jumplist};
//! use reovim_kernel::api::v1::BufferId;
//! use reovim_types_text::Position;
//!
//! let buffer_id = BufferId::new();
//! let mut jumplist = Jumplist::new();
//!
//! // Record positions as user navigates
//! jumplist.push(JumpEntry::new(buffer_id, Position::new(0, 0)));
//! jumplist.push(JumpEntry::new(buffer_id, Position::new(10, 5)));
//! jumplist.push(JumpEntry::new(buffer_id, Position::new(50, 0)));
//!
//! // Navigate backward (Ctrl-O)
//! if let Some(entry) = jumplist.backward() {
//!     println!("Jump to: {:?}", entry.position);
//! }
//!
//! // Navigate forward (Ctrl-I)
//! if let Some(entry) = jumplist.forward() {
//!     println!("Jump to: {:?}", entry.position);
//! }
//! ```

use {reovim_kernel::api::v1::BufferId, reovim_types_text::Position};

/// Maximum number of entries in the jump list.
pub const MAX_JUMPLIST_SIZE: usize = 100;

/// A single entry in the jump list.
///
/// Each entry records a buffer ID and position within that buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpEntry {
    /// The buffer this jump entry refers to.
    pub buffer: BufferId,
    /// Position within the buffer.
    pub position: Position,
}

impl JumpEntry {
    /// Create a new jump entry.
    #[must_use]
    pub const fn new(buffer: BufferId, position: Position) -> Self {
        Self { buffer, position }
    }
}

/// Jump list for tracking cursor position history.
///
/// Tracks "major" cursor position changes for Ctrl-O / Ctrl-I navigation.
/// The list maintains a current index that moves as the user navigates
/// backward and forward through history.
///
/// # Behavior
///
/// - `push()`: Adds a new entry, truncating any "future" entries after
///   the current position (like git commit after detached HEAD)
/// - `push_current()`: Adds without truncating, used when recording
///   current position (e.g., on INSERT mode leave)
/// - `backward()`: Move to older position (Ctrl-O)
/// - `forward()`: Move to newer position (Ctrl-I)
///
/// # Duplicate Suppression
///
/// Consecutive duplicate entries are automatically suppressed to prevent
/// noise from repeated jumps to the same location.
#[derive(Debug, Default, Clone)]
pub struct Jumplist {
    /// Stored entries (oldest first).
    entries: Vec<JumpEntry>,
    /// Current position in the list.
    ///
    /// When at the end (newest), this equals `entries.len()`.
    /// After `backward()`, points to the entry we just jumped to.
    current: usize,
}

impl Jumplist {
    /// Create a new empty jump list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a jump list with custom maximum size.
    ///
    /// Note: The size limit is enforced during push operations.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity.min(MAX_JUMPLIST_SIZE)),
            current: 0,
        }
    }

    /// Record a new position in the jump list.
    ///
    /// This truncates any "future" entries if we're not at the end
    /// (similar to git commit after checkout to an older commit).
    ///
    /// # Returns
    ///
    /// `true` if the entry was added, `false` if it was a duplicate
    /// of the most recent entry.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_session::{JumpEntry, Jumplist};
    /// use reovim_kernel::api::v1::BufferId;
    /// use reovim_types_text::Position;
    ///
    /// let buf = BufferId::new();
    /// let mut list = Jumplist::new();
    ///
    /// assert!(list.push(JumpEntry::new(buf, Position::new(0, 0))));
    /// assert!(!list.push(JumpEntry::new(buf, Position::new(0, 0)))); // Duplicate
    /// ```
    pub fn push(&mut self, entry: JumpEntry) -> bool {
        // Don't add duplicate of last entry
        if self.entries.last() == Some(&entry) {
            self.current = self.entries.len();
            return false;
        }

        // Truncate any entries after current position
        self.entries.truncate(self.current);

        // Add new entry
        self.entries.push(entry);

        // Enforce max size (remove oldest)
        if self.entries.len() > MAX_JUMPLIST_SIZE {
            self.entries.remove(0);
        }

        // Update index to point past the end
        self.current = self.entries.len();
        true
    }

    /// Push a jump entry without truncating history.
    ///
    /// Used when recording the current position without intention to
    /// navigate (e.g., recording position on INSERT mode leave).
    /// Unlike `push()`, this preserves future entries.
    ///
    /// # Returns
    ///
    /// `true` if the entry was added, `false` if duplicate.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn push_current(&mut self, entry: JumpEntry) -> bool {
        // Don't add duplicate of last entry
        if self.entries.last() == Some(&entry) {
            if !self.entries.is_empty() {
                self.current = self.entries.len() - 1;
            }
            return false;
        }

        // DON'T truncate - preserve history
        self.entries.push(entry);

        // Enforce max size
        if self.entries.len() > MAX_JUMPLIST_SIZE {
            self.entries.remove(0);
        }

        // Point past current entry so first Ctrl+O goes to previous
        self.current = self.entries.len();
        true
    }

    /// Jump to older position (Ctrl-O).
    ///
    /// Moves the current index backward and returns the entry at that
    /// position. Returns `None` if already at the beginning.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_session::{JumpEntry, Jumplist};
    /// use reovim_kernel::api::v1::BufferId;
    /// use reovim_types_text::Position;
    ///
    /// let buf = BufferId::new();
    /// let mut list = Jumplist::new();
    /// list.push(JumpEntry::new(buf, Position::new(0, 0)));
    /// list.push(JumpEntry::new(buf, Position::new(10, 5)));
    ///
    /// let entry = list.backward().unwrap();
    /// assert_eq!(entry.position, Position::new(10, 5));
    /// ```
    pub fn backward(&mut self) -> Option<&JumpEntry> {
        if self.current > 0 {
            self.current -= 1;
            self.entries.get(self.current)
        } else {
            None
        }
    }

    /// Jump to newer position (Ctrl-I).
    ///
    /// Moves the current index forward and returns the entry at that
    /// position. Returns `None` if already at the end.
    pub fn forward(&mut self) -> Option<&JumpEntry> {
        if self.current < self.entries.len() {
            let entry = self.entries.get(self.current);
            self.current += 1;
            entry
        } else {
            None
        }
    }

    /// Get the current entry without moving.
    ///
    /// Returns `None` if the list is empty or current is past the end.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn current(&self) -> Option<&JumpEntry> {
        if self.current > 0 && self.current <= self.entries.len() {
            self.entries.get(self.current - 1)
        } else {
            None
        }
    }

    /// Get the current index for debugging/display.
    #[must_use]
    pub const fn current_index(&self) -> usize {
        self.current
    }

    /// Get all entries (oldest first).
    #[must_use]
    pub fn entries(&self) -> &[JumpEntry] {
        &self.entries
    }

    /// Get the number of entries.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len() is not const in stable Rust
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the jump list is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty() is not const in stable Rust
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = 0;
    }
}

#[cfg(test)]
#[path = "jumplist_tests.rs"]
mod tests;
