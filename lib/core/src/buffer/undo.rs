use crate::buffer::Change;

use super::Buffer;

impl Buffer {
    /// Record a change for undo
    ///
    /// If batching is active (during insert mode), changes are accumulated
    /// and committed as a single undo unit when batching ends.
    pub fn record_change(&mut self, change: Change) {
        if self.batching {
            self.pending_batch.push(change);
        } else {
            self.history.push(change);
        }
        self.modified = true;
    }

    /// Begin batching changes (called when entering insert mode)
    ///
    /// All changes recorded while batching is active will be committed
    /// as a single undo unit when `flush_batch` is called.
    pub fn begin_batch(&mut self) {
        self.batching = true;
        self.pending_batch.clear();
    }

    /// Flush pending batch to history (called when leaving insert mode)
    ///
    /// Commits all accumulated changes as a single undo unit.
    /// Does nothing if no changes were recorded.
    #[allow(clippy::missing_panics_doc)] // Safe: we check len() == 1 before unwrap
    pub fn flush_batch(&mut self) {
        self.batching = false;
        if self.pending_batch.is_empty() {
            return;
        }
        let changes = std::mem::take(&mut self.pending_batch);
        if changes.len() == 1 {
            // Single change doesn't need wrapping
            self.history.push(changes.into_iter().next().unwrap());
        } else {
            self.history.push(Change::Batch(changes));
        }
    }

    /// Apply undo - returns true if successful
    #[allow(clippy::cast_possible_truncation)]
    pub fn apply_undo(&mut self) -> bool {
        let Some(change) = self.history.undo() else {
            return false;
        };
        self.apply_change(&change);
        true
    }

    /// Apply redo - returns true if successful
    #[allow(clippy::cast_possible_truncation)]
    pub fn apply_redo(&mut self) -> bool {
        let Some(change) = self.history.redo() else {
            return false;
        };
        self.apply_change(&change);
        true
    }

    /// Apply a change to the buffer (used by undo/redo)
    #[allow(clippy::cast_possible_truncation)]
    fn apply_change(&mut self, change: &Change) {
        match change {
            Change::Insert { pos, text } => {
                self.cur = *pos;
                self.insert_text_at_cursor(text);
            }
            Change::Delete { pos, text } => {
                self.delete_text_at(*pos, text.len());
            }
            Change::Batch(changes) => {
                for c in changes {
                    self.apply_change(c);
                }
            }
        }
    }
}
