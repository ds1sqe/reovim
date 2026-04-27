//! Universal byte-level undo log.
//!
//! [`ByteUndoLog`] records groups of [`ByteEdit`]s as append-only entries.
//! Providers push edits after applying them; the log supports undo/redo by
//! returning inverse edits to re-apply.
//!
//! # Design
//!
//! - **Universal**: one log per buffer, shared across codec/provider switches.
//!   Any provider can push edits; any provider can undo them.
//! - **Append-only entries**: each `push()` creates a new [`ByteUndoEntry`]
//!   grouping one or more `ByteEdit`s into an atomic undo step.
//! - **Clear-on-push**: pushing a new entry clears the redo stack (standard
//!   linear undo behaviour).
//! - **Position-agnostic**: no domain cursor state — that lives in the
//!   provider's semantic undo layer (e.g., `UndoTree`).
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::{ByteEdit, ByteUndoLog};
//!
//! let mut log = ByteUndoLog::new();
//!
//! // Provider pushes an edit group
//! log.push(vec![ByteEdit::insert(0, b"hello")]);
//! log.push(vec![ByteEdit::insert(5, b" world")]);
//!
//! assert_eq!(log.len(), 2);
//! assert!(log.can_undo());
//! assert!(!log.can_redo());
//!
//! // Undo returns inverse edits (last-in, first-out)
//! let entry = log.undo().unwrap();
//! assert_eq!(entry.inverse_edits().len(), 1);
//! assert!(log.can_redo());
//!
//! // Redo re-applies
//! let entry = log.redo().unwrap();
//! assert_eq!(entry.edits().len(), 1);
//! ```

use super::ByteEdit;

/// A single undo entry: one atomic group of byte edits.
#[derive(Debug, Clone)]
pub struct ByteUndoEntry {
    /// The edits in forward (apply) order.
    edits: Vec<ByteEdit>,
}

impl ByteUndoEntry {
    /// The edits to apply (forward direction).
    #[must_use]
    pub fn edits(&self) -> &[ByteEdit] {
        &self.edits
    }

    /// The edits to apply for undo (inverse, in reverse order).
    ///
    /// To undo this entry, apply these edits in order to the storage.
    #[must_use]
    pub fn inverse_edits(&self) -> Vec<ByteEdit> {
        self.edits.iter().rev().map(ByteEdit::inverse).collect()
    }
}

/// Append-only byte-level undo log.
///
/// Records groups of [`ByteEdit`]s with linear undo/redo semantics.
/// All providers sharing a buffer write to the same log so undo works
/// regardless of which codec is active at undo time.
#[derive(Debug, Default)]
pub struct ByteUndoLog {
    /// All entries ever pushed (index = entry index).
    entries: Vec<ByteUndoEntry>,
    /// Current position: `entries[0..cursor]` are applied.
    /// `entries[cursor..]` are the redo stack.
    cursor: usize,
}

impl ByteUndoLog {
    /// Create an empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new group of edits as one atomic undo entry.
    ///
    /// Clears any redo entries beyond the current cursor.
    pub fn push(&mut self, edits: Vec<ByteEdit>) {
        // Trim redo stack
        self.entries.truncate(self.cursor);
        self.entries.push(ByteUndoEntry { edits });
        self.cursor = self.entries.len();
    }

    /// Number of entries in the log (applied entries only, excludes redo stack).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cursor
    }

    /// Whether the log has no applied entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cursor == 0
    }

    /// Whether there are entries to undo.
    #[must_use]
    pub const fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether there are entries to redo.
    #[must_use]
    pub const fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Step backward (undo): returns the entry to invert, or `None`.
    ///
    /// The caller must apply [`ByteUndoEntry::inverse_edits`] to storage.
    /// The cursor is moved backward.
    pub fn undo(&mut self) -> Option<&ByteUndoEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(&self.entries[self.cursor])
    }

    /// Step forward (redo): returns the entry to re-apply, or `None`.
    ///
    /// The caller must apply [`ByteUndoEntry::edits`] to storage.
    /// The cursor is moved forward.
    pub fn redo(&mut self) -> Option<&ByteUndoEntry> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[self.cursor];
        self.cursor += 1;
        Some(entry)
    }

    /// Clear the entire log (e.g., after a file save or buffer reset).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    /// Number of redo entries available.
    #[must_use]
    pub const fn redo_len(&self) -> usize {
        self.entries.len() - self.cursor
    }
}

#[cfg(test)]
#[path = "byte_undo_log_tests.rs"]
mod tests;
