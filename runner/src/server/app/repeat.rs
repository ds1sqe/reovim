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

/// Maximum insert count to prevent denial-of-service (e.g., `999999i` would be very slow).
pub const MAX_INSERT_COUNT: usize = 999;

/// Type of command that entered insert mode.
///
/// Used by the repeat system to know how to repeat text on exit.
/// For example, "3o" opens a line and repeats text on 2 more new lines on exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertEntryType {
    /// Normal insert (i, a, I, A) - repeat text inline.
    #[default]
    Inline,
    /// Open line below (o) - repeat text on new lines below.
    OpenBelow,
    /// Open line above (O) - repeat text on new lines above.
    OpenAbove,
}

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

    /// Count for insert mode entry (e.g., `3i` means repeat text 3 times).
    ///
    /// Defaults to 1. Set when entering insert mode with a count prefix.
    /// Used by `ExitToNormal` to repeat the accumulated text.
    insert_count: usize,

    /// Type of command that entered insert mode.
    ///
    /// Used to determine how to repeat text on exit:
    /// - `Inline`: repeat text at cursor (i, a, I, A)
    /// - `OpenBelow`: repeat text on new lines below (o)
    /// - `OpenAbove`: repeat text on new lines above (O)
    insert_entry_type: InsertEntryType,
}

impl RepeatState {
    /// Create a new empty repeat state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            insert_count: 1,
            ..Self::default()
        }
    }

    /// Start accumulating insert mode text with an optional count and entry type.
    ///
    /// # Arguments
    ///
    /// * `count` - Repeat count for insert mode (e.g., 3 for `3i`).
    ///   Values of 0 default to 1. Values above `MAX_INSERT_COUNT` are capped.
    /// * `entry_type` - How insert mode was entered (affects how text is repeated).
    pub fn start_accumulating_with_count_and_type(
        &mut self,
        count: usize,
        entry_type: InsertEntryType,
    ) {
        self.accumulating = true;
        self.insert_text.clear();
        self.set_insert_count(count);
        self.insert_entry_type = entry_type;
    }

    /// Start accumulating insert mode text with an optional count.
    ///
    /// Uses `InsertEntryType::Inline` by default.
    ///
    /// # Arguments
    ///
    /// * `count` - Repeat count for insert mode (e.g., 3 for `3i`).
    ///   Values of 0 default to 1. Values above `MAX_INSERT_COUNT` are capped.
    pub fn start_accumulating_with_count(&mut self, count: usize) {
        self.start_accumulating_with_count_and_type(count, InsertEntryType::Inline);
    }

    /// Start accumulating insert mode text (count defaults to 1, inline entry).
    pub fn start_accumulating(&mut self) {
        self.start_accumulating_with_count(1);
    }

    /// Get the insert entry type.
    #[must_use]
    pub const fn get_insert_entry_type(&self) -> InsertEntryType {
        self.insert_entry_type
    }

    /// Set the insert count with validation.
    ///
    /// - Count of 0 is treated as 1 (no repetition)
    /// - Count above `MAX_INSERT_COUNT` is capped
    pub fn set_insert_count(&mut self, count: usize) {
        self.insert_count = if count == 0 {
            1
        } else {
            count.min(MAX_INSERT_COUNT)
        };
    }

    /// Get the current insert count.
    #[must_use]
    pub const fn get_insert_count(&self) -> usize {
        self.insert_count
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
        self.insert_count = 1;
        self.insert_entry_type = InsertEntryType::Inline;
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
        state.start_accumulating_with_count(5);
        state.accumulate_insert("text");

        state.clear();
        assert!(state.last_command.is_none());
        assert!(state.insert_text.is_empty());
        assert!(!state.accumulating);
        assert_eq!(state.get_insert_count(), 1);
    }

    #[test]
    fn test_repeat_state_insert_count_default() {
        let state = RepeatState::new();
        assert_eq!(state.get_insert_count(), 1);
    }

    #[test]
    fn test_repeat_state_set_insert_count() {
        let mut state = RepeatState::new();
        state.set_insert_count(3);
        assert_eq!(state.get_insert_count(), 3);
    }

    #[test]
    fn test_repeat_state_insert_count_zero_becomes_one() {
        let mut state = RepeatState::new();
        state.set_insert_count(0);
        assert_eq!(state.get_insert_count(), 1);
    }

    #[test]
    fn test_repeat_state_insert_count_capped() {
        let mut state = RepeatState::new();
        state.set_insert_count(1000);
        assert_eq!(state.get_insert_count(), MAX_INSERT_COUNT);

        state.set_insert_count(10000);
        assert_eq!(state.get_insert_count(), MAX_INSERT_COUNT);
    }

    #[test]
    fn test_repeat_state_insert_count_max_accepted() {
        let mut state = RepeatState::new();
        state.set_insert_count(MAX_INSERT_COUNT);
        assert_eq!(state.get_insert_count(), MAX_INSERT_COUNT);
    }

    #[test]
    fn test_repeat_state_start_accumulating_with_count() {
        let mut state = RepeatState::new();
        state.start_accumulating_with_count(5);
        assert!(state.accumulating);
        assert!(state.insert_text.is_empty());
        assert_eq!(state.get_insert_count(), 5);
    }

    #[test]
    fn test_repeat_state_sequential_inserts_reset_count() {
        let mut state = RepeatState::new();

        // First insert with count 3
        state.start_accumulating_with_count(3);
        state.accumulate_insert("hello");
        state.stop_accumulating();
        assert_eq!(state.get_insert_count(), 3);

        // Second insert without explicit count
        state.start_accumulating();
        assert_eq!(state.get_insert_count(), 1);
    }

    // =========================================================================
    // Insert Entry Type Tests
    // =========================================================================

    #[test]
    fn test_insert_entry_type_default() {
        let state = RepeatState::new();
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::Inline);
    }

    #[test]
    fn test_start_accumulating_with_count_and_type_inline() {
        let mut state = RepeatState::new();
        state.start_accumulating_with_count_and_type(3, InsertEntryType::Inline);
        assert!(state.accumulating);
        assert_eq!(state.get_insert_count(), 3);
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::Inline);
    }

    #[test]
    fn test_start_accumulating_with_count_and_type_open_below() {
        let mut state = RepeatState::new();
        state.start_accumulating_with_count_and_type(5, InsertEntryType::OpenBelow);
        assert!(state.accumulating);
        assert_eq!(state.get_insert_count(), 5);
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::OpenBelow);
    }

    #[test]
    fn test_start_accumulating_with_count_and_type_open_above() {
        let mut state = RepeatState::new();
        state.start_accumulating_with_count_and_type(2, InsertEntryType::OpenAbove);
        assert!(state.accumulating);
        assert_eq!(state.get_insert_count(), 2);
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::OpenAbove);
    }

    #[test]
    fn test_clear_resets_entry_type() {
        let mut state = RepeatState::new();
        state.start_accumulating_with_count_and_type(5, InsertEntryType::OpenBelow);
        state.clear();
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::Inline);
    }

    #[test]
    fn test_start_accumulating_without_type_defaults_to_inline() {
        let mut state = RepeatState::new();
        // First set it to something else
        state.start_accumulating_with_count_and_type(3, InsertEntryType::OpenBelow);
        // Then start accumulating with just count
        state.start_accumulating_with_count(5);
        assert_eq!(state.get_insert_entry_type(), InsertEntryType::Inline);
    }
}
