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
mod tests {
    use super::*;

    // -- Construction --

    #[test]
    fn new_log_is_empty() {
        let log = ByteUndoLog::new();
        assert_eq!(log.len(), 0);
        assert!(!log.can_undo());
        assert!(!log.can_redo());
    }

    #[test]
    fn default_log_is_empty() {
        let log = ByteUndoLog::default();
        assert_eq!(log.len(), 0);
    }

    // -- Push --

    #[test]
    fn push_single_entry() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"hello")]);
        assert_eq!(log.len(), 1);
        assert!(log.can_undo());
        assert!(!log.can_redo());
    }

    #[test]
    fn push_multiple_entries() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"hello")]);
        log.push(vec![ByteEdit::insert(5, b" world")]);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn push_empty_edits_is_valid() {
        let mut log = ByteUndoLog::new();
        log.push(vec![]);
        assert_eq!(log.len(), 1);
        assert!(log.can_undo());
    }

    #[test]
    fn push_clears_redo_stack() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        log.push(vec![ByteEdit::insert(1, b"b")]);

        // Undo one step, creating a redo entry
        log.undo();
        assert!(log.can_redo());
        assert_eq!(log.redo_len(), 1);

        // Push a new entry — redo stack should be cleared
        log.push(vec![ByteEdit::insert(1, b"c")]);
        assert!(!log.can_redo());
        assert_eq!(log.redo_len(), 0);
        assert_eq!(log.len(), 2);
    }

    // -- Undo --

    #[test]
    fn undo_returns_none_when_empty() {
        let mut log = ByteUndoLog::new();
        assert!(log.undo().is_none());
    }

    #[test]
    fn undo_returns_last_entry() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"hello")]);
        log.push(vec![ByteEdit::insert(5, b" world")]);

        let entry = log.undo().unwrap();
        assert_eq!(entry.edits().len(), 1);
        assert_eq!(entry.edits()[0].new_bytes, b" world");
    }

    #[test]
    fn undo_moves_cursor_backward() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        log.push(vec![ByteEdit::insert(1, b"b")]);

        assert_eq!(log.len(), 2);
        log.undo();
        assert_eq!(log.len(), 1);
        log.undo();
        assert_eq!(log.len(), 0);
        assert!(!log.can_undo());
    }

    #[test]
    fn undo_enables_redo() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        assert!(!log.can_redo());

        log.undo();
        assert!(log.can_redo());
    }

    #[test]
    fn undo_inverse_edits_are_reversed_and_inverted() {
        let mut log = ByteUndoLog::new();
        let e1 = ByteEdit::insert(0, b"hello");
        let e2 = ByteEdit::insert(5, b" world");
        log.push(vec![e1.clone(), e2.clone()]);

        let entry = log.undo().unwrap();
        let inv = entry.inverse_edits();

        // Inverse of [insert, insert] should be [delete, delete] in reverse order
        assert_eq!(inv.len(), 2);
        // Last edit inverted first
        assert_eq!(inv[0], e2.inverse());
        assert_eq!(inv[1], e1.inverse());
    }

    // -- Redo --

    #[test]
    fn redo_returns_none_when_at_end() {
        let mut log = ByteUndoLog::new();
        assert!(log.redo().is_none());

        log.push(vec![ByteEdit::insert(0, b"a")]);
        assert!(log.redo().is_none()); // cursor is at end
    }

    #[test]
    fn redo_after_undo() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"hello")]);
        log.undo();

        let entry = log.redo().unwrap();
        assert_eq!(entry.edits()[0].new_bytes, b"hello");
    }

    #[test]
    fn redo_moves_cursor_forward() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        log.push(vec![ByteEdit::insert(1, b"b")]);

        log.undo();
        log.undo();
        assert_eq!(log.len(), 0);

        log.redo();
        assert_eq!(log.len(), 1);
        log.redo();
        assert_eq!(log.len(), 2);
        assert!(!log.can_redo());
    }

    // -- Clear --

    #[test]
    fn clear_resets_everything() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        log.push(vec![ByteEdit::insert(1, b"b")]);
        log.undo(); // create redo entry

        log.clear();
        assert_eq!(log.len(), 0);
        assert!(!log.can_undo());
        assert!(!log.can_redo());
        assert_eq!(log.redo_len(), 0);
    }

    // -- ByteUndoEntry --

    #[test]
    fn entry_edits_accessor() {
        let edit = ByteEdit::insert(0, b"test");
        let entry = ByteUndoEntry {
            edits: vec![edit.clone()],
        };
        assert_eq!(entry.edits(), &[edit]);
    }

    #[test]
    fn entry_inverse_edits_single() {
        let edit = ByteEdit::insert(0, b"hello");
        let entry = ByteUndoEntry {
            edits: vec![edit.clone()],
        };
        let inv = entry.inverse_edits();
        assert_eq!(inv, vec![edit.inverse()]);
    }

    // -- Redo length --

    #[test]
    fn redo_len_tracks_correctly() {
        let mut log = ByteUndoLog::new();
        log.push(vec![ByteEdit::insert(0, b"a")]);
        log.push(vec![ByteEdit::insert(1, b"b")]);
        log.push(vec![ByteEdit::insert(2, b"c")]);

        assert_eq!(log.redo_len(), 0);
        log.undo();
        assert_eq!(log.redo_len(), 1);
        log.undo();
        assert_eq!(log.redo_len(), 2);
        log.redo();
        assert_eq!(log.redo_len(), 1);
    }
}
