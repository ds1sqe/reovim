//! Repeat state and undo transaction batching.
//!
//! Supports Vim's `.` repeat command and batched undo transactions
//! for insert mode text accumulation.

use reovim_kernel::api::v1::{BufferId, Edit, Position};

// ============================================================================
// Undo Transaction Batching Infrastructure
// ============================================================================

/// Pending edits for transaction batching.
///
/// Accumulates consecutive edits (typically character insertions in
/// insert mode) until a batch-breaking event occurs, then flushes
/// them as a single undo transaction.
///
/// # Design Philosophy
///
/// In Vim, typing "hello" in insert mode and pressing Escape creates
/// a single undo node - pressing `u` undoes all 5 characters at once.
/// This struct enables that behavior by collecting edits during insert
/// mode and committing them as a batch on mode exit or other break events.
///
/// # Batch Break Conditions
///
/// A batch is flushed when:
/// - Mode changes (e.g., Escape exits insert mode)
/// - A command is executed (e.g., Backspace, arrow keys)
/// - The buffer changes (different buffer ID)
///
/// # Example
///
/// ```ignore
/// // User types: ihello<Esc>
/// // Each 'h', 'e', 'l', 'l', 'o' calls accumulate_edit()
/// // <Esc> triggers flush_pending_edits()
/// // Result: single undo node containing all 5 edits
/// ```
#[derive(Debug, Default)]
pub struct PendingEditBatch {
    /// Buffer this batch applies to.
    ///
    /// When accumulating to a different buffer, the current batch
    /// is flushed before starting a new one.
    pub(super) buffer_id: Option<BufferId>,

    /// Accumulated edits in order.
    ///
    /// Each edit is typically a single character insertion, but
    /// can be any edit operation.
    pub(super) edits: Vec<Edit>,

    /// Cursor position before first edit in batch.
    ///
    /// Captured when the batch starts (first edit accumulated).
    /// Used as `cursor_before` when committing to undo tree.
    pub(super) cursor_before: Option<Position>,

    /// Cursor position after most recent edit.
    ///
    /// Updated on each accumulate. Used as `cursor_after` when
    /// committing to undo tree.
    pub(super) cursor_after: Option<Position>,
}

impl PendingEditBatch {
    /// Create a new empty pending edit batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the batch is empty (no pending edits).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Get the number of pending edits.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.edits.len()
    }

    /// Clear the batch, resetting all fields.
    pub fn clear(&mut self) {
        self.buffer_id = None;
        self.edits.clear();
        self.cursor_before = None;
        self.cursor_after = None;
    }
}

// ============================================================================
// Repeat Infrastructure
// ============================================================================

/// State for the repeat command (`.`).
///
/// Tracks the last repeatable command so that `.` can re-execute it.
/// Only certain commands are repeatable (text-modifying commands).
#[derive(Debug, Clone, Default)]
pub struct RepeatState {
    /// The last repeatable command ID.
    ///
    /// Stored as a string to avoid lifetime issues with `CommandId`.
    pub last_command: Option<String>,

    /// Text accumulated during insert mode.
    ///
    /// When insert mode is exited, this text is stored for `.` to replay.
    pub insert_text: String,

    /// Whether we're currently accumulating insert text.
    pub accumulating: bool,
}

impl RepeatState {
    /// Create a new empty repeat state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start accumulating insert mode text.
    pub fn start_accumulating(&mut self) {
        self.accumulating = true;
        self.insert_text.clear();
    }

    /// Add text to the insert accumulator.
    pub fn accumulate_insert(&mut self, text: &str) {
        if self.accumulating {
            self.insert_text.push_str(text);
        }
    }

    /// Stop accumulating and store the result.
    pub const fn stop_accumulating(&mut self) {
        self.accumulating = false;
    }

    /// Record a repeatable command.
    ///
    /// Only text-modifying commands should be recorded.
    pub fn record_command(&mut self, command_id: &str) {
        self.last_command = Some(command_id.to_string());
    }

    /// Clear the repeat state.
    pub fn clear(&mut self) {
        self.last_command = None;
        self.insert_text.clear();
        self.accumulating = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_edit_batch_new_is_empty() {
        let batch = PendingEditBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert!(batch.buffer_id.is_none());
    }

    #[test]
    fn test_pending_edit_batch_clear() {
        let mut batch = PendingEditBatch::new();
        batch.buffer_id = Some(BufferId::from_raw(1));
        batch.edits.push(Edit::insert(Position::new(0, 0), "a"));
        batch.cursor_before = Some(Position::new(0, 0));
        batch.cursor_after = Some(Position::new(0, 1));

        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
        assert!(batch.buffer_id.is_none());
    }

    #[test]
    fn test_repeat_state_new() {
        let state = RepeatState::new();
        assert!(state.last_command.is_none());
        assert!(state.insert_text.is_empty());
        assert!(!state.accumulating);
    }

    #[test]
    fn test_repeat_state_record_command() {
        let mut state = RepeatState::new();
        state.record_command("delete-word");
        assert_eq!(state.last_command, Some("delete-word".to_string()));
    }

    #[test]
    fn test_repeat_state_accumulate_insert() {
        let mut state = RepeatState::new();
        state.start_accumulating();
        state.accumulate_insert("hello");
        state.accumulate_insert(" world");
        assert_eq!(state.insert_text, "hello world");
        state.stop_accumulating();
        assert!(!state.accumulating);
    }

    #[test]
    fn test_repeat_state_accumulate_only_when_active() {
        let mut state = RepeatState::new();
        // Should not accumulate when not started
        state.accumulate_insert("ignored");
        assert!(state.insert_text.is_empty());

        // Start accumulating
        state.start_accumulating();
        state.accumulate_insert("kept");
        assert_eq!(state.insert_text, "kept");
    }

    #[test]
    fn test_repeat_state_clear() {
        let mut state = RepeatState::new();
        state.record_command("change");
        state.start_accumulating();
        state.accumulate_insert("text");

        state.clear();
        assert!(state.last_command.is_none());
        assert!(state.insert_text.is_empty());
        assert!(!state.accumulating);
    }
}
