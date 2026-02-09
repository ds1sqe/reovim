//! CLI panel state for embedded REPL.
//!
//! Provides a tmux-like command panel for executing CLI commands
//! without leaving the TUI.

use std::collections::VecDeque;

/// Maximum command history size.
const MAX_HISTORY: usize = 50;

/// Maximum input length.
const MAX_INPUT_LENGTH: usize = 256;

/// Convert a character index to a byte index in a UTF-8 string.
///
/// Returns `s.len()` if `char_idx` is beyond the string length.
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

/// Get the byte range for the character at `char_idx`.
///
/// Returns `None` if `char_idx` is out of bounds.
fn char_byte_range(s: &str, char_idx: usize) -> Option<(usize, usize)> {
    let mut chars = s.char_indices();
    if let Some((start, c)) = chars.nth(char_idx) {
        let end = start + c.len_utf8();
        Some((start, end))
    } else {
        None
    }
}

/// CLI panel state.
#[derive(Debug)]
pub struct CliPanelState {
    /// Whether panel is visible.
    pub visible: bool,
    /// Current input line.
    pub input: String,
    /// Cursor position in input.
    pub cursor_pos: usize,
    /// Command history.
    pub history: VecDeque<CliHistoryEntry>,
    /// History navigation index (None = editing new command).
    history_index: Option<usize>,
    /// Saved input when navigating history.
    saved_input: String,
    /// Panel height in rows.
    pub height: u16,
    /// Scroll offset in history view.
    pub scroll_offset: usize,
}

/// Entry in command history.
#[derive(Debug, Clone)]
pub struct CliHistoryEntry {
    /// The command that was executed.
    pub command: String,
    /// Result (success or error).
    pub result: CliResult,
}

/// Result of a CLI command.
#[derive(Debug, Clone)]
pub enum CliResult {
    /// Success with formatted output.
    Ok(String),
    /// Error message.
    Err(String),
    /// Pending response (waiting for server).
    Pending(String),
}

impl Default for CliPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl CliPanelState {
    /// Create a new CLI panel state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            visible: false,
            input: String::new(),
            cursor_pos: 0,
            history: VecDeque::new(),
            history_index: None,
            saved_input: String::new(),
            height: 10,
            scroll_offset: 0,
        }
    }

    /// Toggle panel visibility.
    ///
    /// Returns the new visibility state.
    pub const fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        if self.visible {
            // Reset scroll to bottom when showing
            self.scroll_offset = 0;
        }
        self.visible
    }

    /// Show the panel.
    pub const fn show(&mut self) {
        self.visible = true;
        self.scroll_offset = 0;
    }

    /// Hide the panel.
    pub const fn hide(&mut self) {
        self.visible = false;
    }

    // Input editing methods

    /// Insert a character at cursor position.
    ///
    /// `cursor_pos` is a character index, not a byte index.
    pub fn insert_char(&mut self, c: char) {
        if self.input.chars().count() < MAX_INPUT_LENGTH {
            let byte_idx = char_to_byte_index(&self.input, self.cursor_pos);
            self.input.insert(byte_idx, c);
            self.cursor_pos += 1;
            self.history_index = None;
        }
    }

    /// Delete character before cursor (backspace).
    ///
    /// `cursor_pos` is a character index, not a byte index.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0
            && let Some((start, end)) = char_byte_range(&self.input, self.cursor_pos - 1)
        {
            self.input.replace_range(start..end, "");
            self.cursor_pos -= 1;
            self.history_index = None;
        }
    }

    /// Delete character at cursor (delete key).
    ///
    /// `cursor_pos` is a character index, not a byte index.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn delete(&mut self) {
        let char_count = self.input.chars().count();
        if self.cursor_pos < char_count
            && let Some((start, end)) = char_byte_range(&self.input, self.cursor_pos)
        {
            self.input.replace_range(start..end, "");
            self.history_index = None;
        }
    }

    /// Move cursor left.
    pub const fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right.
    pub fn move_cursor_right(&mut self) {
        let char_count = self.input.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start of input.
    pub const fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end of input.
    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    // History navigation

    /// Navigate to previous command in history.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Save current input and go to most recent
                self.saved_input = self.input.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(idx) if idx > 0 => {
                self.history_index = Some(idx - 1);
            }
            Some(_) => {
                // Already at oldest - do nothing
                return;
            }
        }

        if let Some(idx) = self.history_index
            && let Some(entry) = self.history.get(idx)
        {
            self.input = entry.command.clone();
            self.cursor_pos = self.input.chars().count();
        }
    }

    /// Navigate to next command in history.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };

        if idx + 1 >= self.history.len() {
            // Return to saved input
            self.history_index = None;
            self.input = std::mem::take(&mut self.saved_input);
            self.cursor_pos = self.input.chars().count();
        } else {
            self.history_index = Some(idx + 1);
            if let Some(entry) = self.history.get(idx + 1) {
                self.input = entry.command.clone();
                self.cursor_pos = self.input.chars().count();
            }
        }
    }

    // Command submission

    /// Submit current input and return command string.
    ///
    /// Returns `None` if input is empty/whitespace.
    pub fn submit(&mut self) -> Option<String> {
        let trimmed = self.input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let cmd = std::mem::take(&mut self.input);
        self.cursor_pos = 0;
        self.history_index = None;
        self.saved_input.clear();
        Some(cmd)
    }

    /// Add a command result to history.
    pub fn add_result(&mut self, command: String, result: CliResult) {
        self.history.push_back(CliHistoryEntry { command, result });
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        // Scroll to bottom to show new result
        self.scroll_offset = 0;
    }

    /// Update a history entry's result by index.
    ///
    /// Returns `true` if the entry was updated, `false` if index out of bounds.
    pub fn update_result(&mut self, index: usize, result: CliResult) -> bool {
        if let Some(entry) = self.history.get_mut(index) {
            entry.result = result;
            true
        } else {
            false
        }
    }

    /// Clear input buffer.
    pub fn clear_input(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
    }

    // Scroll methods

    /// Scroll history up by n lines (older entries).
    pub const fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
    }

    /// Scroll history down by n lines (newer entries).
    pub const fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll to bottom of history (most recent).
    pub const fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_panel_state() {
        let state = CliPanelState::new();
        assert!(!state.visible);
        assert!(state.input.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_toggle() {
        let mut state = CliPanelState::new();
        assert!(!state.visible);
        assert!(state.toggle());
        assert!(state.visible);
        assert!(!state.toggle());
        assert!(!state.visible);
    }

    #[test]
    fn test_insert_char() {
        let mut state = CliPanelState::new();
        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.input, "hi");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn test_backspace() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 5;
        state.backspace();
        assert_eq!(state.input, "hell");
        assert_eq!(state.cursor_pos, 4);
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 3;

        state.move_cursor_left();
        assert_eq!(state.cursor_pos, 2);

        state.move_cursor_right();
        assert_eq!(state.cursor_pos, 3);

        state.move_cursor_home();
        assert_eq!(state.cursor_pos, 0);

        state.move_cursor_end();
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn test_submit() {
        let mut state = CliPanelState::new();
        state.input = "  ".to_string();
        assert!(state.submit().is_none());

        state.input = "keys hello".to_string();
        state.cursor_pos = 10;
        let cmd = state.submit();
        assert_eq!(cmd, Some("keys hello".to_string()));
        assert!(state.input.is_empty());
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_history_navigation() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.add_result("cmd2".to_string(), CliResult::Ok("ok".to_string()));

        state.input = "current".to_string();
        state.cursor_pos = 7;

        // Navigate to previous (cmd2)
        state.history_prev();
        assert_eq!(state.input, "cmd2");

        // Navigate to previous (cmd1)
        state.history_prev();
        assert_eq!(state.input, "cmd1");

        // Navigate to next (cmd2)
        state.history_next();
        assert_eq!(state.input, "cmd2");

        // Navigate to next (back to current)
        state.history_next();
        assert_eq!(state.input, "current");
    }

    #[test]
    fn test_add_result_overflow() {
        let mut state = CliPanelState::new();
        for i in 0..60 {
            state.add_result(format!("cmd{i}"), CliResult::Ok("ok".to_string()));
        }
        assert_eq!(state.history.len(), MAX_HISTORY);
        // First command should be cmd10 (0-9 were removed)
        assert_eq!(state.history.front().unwrap().command, "cmd10");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cli_result_pending_variant() {
        let pending = CliResult::Pending("Querying...".to_string());
        match pending {
            CliResult::Pending(msg) => assert_eq!(msg, "Querying..."),
            _ => panic!("Expected Pending variant"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_update_result_valid_index() {
        let mut state = CliPanelState::new();
        state.add_result("mode".to_string(), CliResult::Pending("Querying...".to_string()));
        assert_eq!(state.history.len(), 1);

        // Update should succeed
        let updated = state.update_result(0, CliResult::Ok("Mode: NORMAL".to_string()));
        assert!(updated);

        // Verify the result was updated
        match &state.history[0].result {
            CliResult::Ok(msg) => assert_eq!(msg, "Mode: NORMAL"),
            _ => panic!("Expected Ok variant after update"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_update_result_invalid_index() {
        let mut state = CliPanelState::new();
        state.add_result("mode".to_string(), CliResult::Pending("Querying...".to_string()));

        // Update with invalid index should return false
        let updated = state.update_result(99, CliResult::Ok("Result".to_string()));
        assert!(!updated);

        // Original entry should be unchanged
        match &state.history[0].result {
            CliResult::Pending(msg) => assert_eq!(msg, "Querying..."),
            _ => panic!("Expected Pending variant to be unchanged"),
        }
    }

    #[test]
    fn test_update_result_after_clear() {
        let mut state = CliPanelState::new();
        state.add_result("mode".to_string(), CliResult::Pending("Querying...".to_string()));

        // Clear history
        state.history.clear();

        // Update should fail gracefully (return false, no panic)
        let updated = state.update_result(0, CliResult::Ok("Result".to_string()));
        assert!(!updated);
    }

    // Unicode handling tests

    #[test]
    fn test_insert_unicode_char() {
        let mut state = CliPanelState::new();
        state.insert_char('한');
        state.insert_char('글');
        assert_eq!(state.input, "한글");
        assert_eq!(state.cursor_pos, 2); // Character count, not byte count
    }

    #[test]
    fn test_insert_unicode_in_middle() {
        let mut state = CliPanelState::new();
        state.input = "ab".to_string();
        state.cursor_pos = 1; // After 'a'
        state.insert_char('中');
        assert_eq!(state.input, "a中b");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn test_backspace_unicode() {
        let mut state = CliPanelState::new();
        state.input = "안녕".to_string();
        state.cursor_pos = 2; // At end (2 chars)
        state.backspace();
        assert_eq!(state.input, "안");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn test_backspace_unicode_in_middle() {
        let mut state = CliPanelState::new();
        state.input = "a中b".to_string();
        state.cursor_pos = 2; // After 'a中'
        state.backspace();
        assert_eq!(state.input, "ab");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn test_delete_unicode() {
        let mut state = CliPanelState::new();
        state.input = "日本語".to_string();
        state.cursor_pos = 1; // Before '本'
        state.delete();
        assert_eq!(state.input, "日語");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn test_cursor_movement_unicode() {
        let mut state = CliPanelState::new();
        state.input = "한글".to_string();
        state.cursor_pos = 0;

        state.move_cursor_right();
        assert_eq!(state.cursor_pos, 1);

        state.move_cursor_right();
        assert_eq!(state.cursor_pos, 2);

        // At end, should not move further
        state.move_cursor_right();
        assert_eq!(state.cursor_pos, 2);

        state.move_cursor_end();
        assert_eq!(state.cursor_pos, 2); // 2 chars, not 6 bytes
    }

    #[test]
    fn test_scroll_methods() {
        let mut state = CliPanelState::new();
        assert_eq!(state.scroll_offset, 0);

        state.scroll_up(5);
        assert_eq!(state.scroll_offset, 5);

        state.scroll_up(3);
        assert_eq!(state.scroll_offset, 8);

        state.scroll_down(2);
        assert_eq!(state.scroll_offset, 6);

        state.scroll_to_bottom();
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_down_saturates() {
        let mut state = CliPanelState::new();
        state.scroll_offset = 3;

        state.scroll_down(10); // More than current offset
        assert_eq!(state.scroll_offset, 0); // Should saturate at 0
    }

    #[test]
    fn test_default_impl() {
        let state = CliPanelState::default();
        assert!(!state.visible);
        assert!(state.input.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_show_and_hide() {
        let mut state = CliPanelState::new();
        assert!(!state.visible);

        state.show();
        assert!(state.visible);
        assert_eq!(state.scroll_offset, 0);

        state.hide();
        assert!(!state.visible);
    }

    #[test]
    fn test_show_resets_scroll() {
        let mut state = CliPanelState::new();
        state.scroll_offset = 10;
        state.show();
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_toggle_resets_scroll_on_show() {
        let mut state = CliPanelState::new();
        state.scroll_offset = 5;

        state.toggle(); // Show
        assert!(state.visible);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 0;

        // Backspace at position 0 should be a no-op
        state.backspace();
        assert_eq!(state.input, "hello");
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_delete_at_end() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 5; // At end

        // Delete at end should be a no-op
        state.delete();
        assert_eq!(state.input, "hello");
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn test_delete_in_middle() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 2;

        state.delete();
        assert_eq!(state.input, "helo");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn test_move_cursor_left_at_zero() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 0;

        // Should be a no-op at 0
        state.move_cursor_left();
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_move_cursor_right_at_end() {
        let mut state = CliPanelState::new();
        state.input = "hi".to_string();
        state.cursor_pos = 2;

        // Should be a no-op at end
        state.move_cursor_right();
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn test_history_prev_empty() {
        let mut state = CliPanelState::new();
        state.input = "test".to_string();

        // Should be a no-op when history is empty
        state.history_prev();
        assert_eq!(state.input, "test");
    }

    #[test]
    fn test_history_prev_at_oldest() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));

        // Navigate to oldest
        state.history_prev();
        assert_eq!(state.input, "cmd1");

        // Try to go further back - should stay at oldest
        state.history_prev();
        assert_eq!(state.input, "cmd1");
    }

    #[test]
    fn test_history_next_without_navigation() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.input = "test".to_string();

        // history_next without prior history_prev should be a no-op
        state.history_next();
        assert_eq!(state.input, "test");
    }

    #[test]
    fn test_clear_input() {
        let mut state = CliPanelState::new();
        state.input = "hello world".to_string();
        state.cursor_pos = 5;
        // Navigate to set history_index
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.history_prev();

        state.clear_input();
        assert!(state.input.is_empty());
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_submit_clears_state() {
        let mut state = CliPanelState::new();
        state.input = "cmd test".to_string();
        state.cursor_pos = 8;

        // Set some history navigation state
        state.add_result("old".to_string(), CliResult::Ok("ok".to_string()));
        state.history_prev();
        state.input = "cmd test".to_string();

        let cmd = state.submit();
        assert_eq!(cmd.unwrap(), "cmd test");
        assert!(state.input.is_empty());
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_submit_empty_returns_none() {
        let mut state = CliPanelState::new();
        state.input = String::new();
        assert!(state.submit().is_none());
    }

    #[test]
    fn test_submit_whitespace_only_returns_none() {
        let mut state = CliPanelState::new();
        state.input = "   \t  ".to_string();
        assert!(state.submit().is_none());
    }

    #[test]
    fn test_insert_char_resets_history_index() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.history_prev();
        assert!(state.history_index.is_some());

        state.insert_char('x');
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_backspace_resets_history_index() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.history_prev();
        assert!(state.history_index.is_some());

        state.backspace();
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_delete_resets_history_index() {
        let mut state = CliPanelState::new();
        state.add_result("cmd1".to_string(), CliResult::Ok("ok".to_string()));
        state.history_prev();
        assert!(state.history_index.is_some());
        state.cursor_pos = 0; // Position before content

        state.delete();
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_insert_char_max_length() {
        let mut state = CliPanelState::new();
        // Fill to max length
        for _ in 0..MAX_INPUT_LENGTH {
            state.insert_char('a');
        }
        assert_eq!(state.input.chars().count(), MAX_INPUT_LENGTH);

        // Try to insert one more - should be a no-op
        state.insert_char('b');
        assert_eq!(state.input.chars().count(), MAX_INPUT_LENGTH);
    }

    #[test]
    fn test_history_entry_debug_clone() {
        let entry = CliHistoryEntry {
            command: "test".to_string(),
            result: CliResult::Ok("output".to_string()),
        };
        let cloned = entry.clone();
        assert_eq!(cloned.command, "test");
        let debug = format!("{entry:?}");
        assert!(debug.contains("CliHistoryEntry"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cli_result_err_variant() {
        let err = CliResult::Err("not found".to_string());
        match err {
            CliResult::Err(msg) => assert_eq!(msg, "not found"),
            _ => panic!("Expected Err variant"),
        }
    }

    #[test]
    fn test_cli_result_debug() {
        let ok = CliResult::Ok("success".to_string());
        let debug = format!("{ok:?}");
        assert!(debug.contains("Ok"));
    }

    #[test]
    fn test_char_to_byte_index_ascii() {
        assert_eq!(char_to_byte_index("hello", 0), 0);
        assert_eq!(char_to_byte_index("hello", 3), 3);
        assert_eq!(char_to_byte_index("hello", 5), 5); // Beyond end
        assert_eq!(char_to_byte_index("hello", 10), 5); // Way beyond end
    }

    #[test]
    fn test_char_to_byte_index_unicode() {
        // Each CJK char is 3 bytes in UTF-8
        assert_eq!(char_to_byte_index("한글", 0), 0);
        assert_eq!(char_to_byte_index("한글", 1), 3);
        assert_eq!(char_to_byte_index("한글", 2), 6); // Beyond end
    }

    #[test]
    fn test_char_byte_range_ascii() {
        assert_eq!(char_byte_range("hello", 0), Some((0, 1)));
        assert_eq!(char_byte_range("hello", 4), Some((4, 5)));
        assert_eq!(char_byte_range("hello", 5), None); // Out of bounds
    }

    #[test]
    fn test_char_byte_range_unicode() {
        assert_eq!(char_byte_range("한글", 0), Some((0, 3)));
        assert_eq!(char_byte_range("한글", 1), Some((3, 6)));
        assert_eq!(char_byte_range("한글", 2), None); // Out of bounds
    }

    #[test]
    fn test_add_result_scroll_reset() {
        let mut state = CliPanelState::new();
        state.scroll_offset = 10;

        state.add_result("cmd".to_string(), CliResult::Ok("result".to_string()));
        assert_eq!(state.scroll_offset, 0); // Reset to show new result
    }

    #[test]
    fn test_history_navigation_three_entries() {
        let mut state = CliPanelState::new();
        state.add_result("a".to_string(), CliResult::Ok("1".to_string()));
        state.add_result("b".to_string(), CliResult::Ok("2".to_string()));
        state.add_result("c".to_string(), CliResult::Ok("3".to_string()));

        state.input = "current".to_string();
        state.cursor_pos = 7;

        // Navigate backward through all three
        state.history_prev(); // -> c
        assert_eq!(state.input, "c");
        state.history_prev(); // -> b
        assert_eq!(state.input, "b");
        state.history_prev(); // -> a
        assert_eq!(state.input, "a");

        // At oldest, should stay
        state.history_prev();
        assert_eq!(state.input, "a");

        // Navigate forward
        state.history_next(); // -> b
        assert_eq!(state.input, "b");
        state.history_next(); // -> c
        assert_eq!(state.input, "c");
        state.history_next(); // -> back to current
        assert_eq!(state.input, "current");
    }

    #[test]
    fn test_panel_state_debug() {
        let state = CliPanelState::new();
        let debug = format!("{state:?}");
        assert!(debug.contains("CliPanelState"));
    }

    #[test]
    fn test_insert_at_cursor_middle() {
        let mut state = CliPanelState::new();
        state.input = "hllo".to_string();
        state.cursor_pos = 1;
        state.insert_char('e');
        assert_eq!(state.input, "hello");
        assert_eq!(state.cursor_pos, 2);
    }
}
