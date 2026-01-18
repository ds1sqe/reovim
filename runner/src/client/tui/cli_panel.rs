//! CLI panel state for embedded REPL.
//!
//! Provides a tmux-like command panel for executing CLI commands
//! without leaving the TUI.

use std::collections::VecDeque;

/// Maximum command history size.
const MAX_HISTORY: usize = 50;

/// Maximum input length.
const MAX_INPUT_LENGTH: usize = 256;

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
    pub fn insert_char(&mut self, c: char) {
        if self.input.len() < MAX_INPUT_LENGTH {
            self.input.insert(self.cursor_pos, c);
            self.cursor_pos += 1;
            self.history_index = None;
        }
    }

    /// Delete character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input.remove(self.cursor_pos);
            self.history_index = None;
        }
    }

    /// Delete character at cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
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
    pub const fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start of input.
    pub const fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end of input.
    pub const fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    // History navigation

    /// Navigate to previous command in history.
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
            self.cursor_pos = self.input.len();
        }
    }

    /// Navigate to next command in history.
    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };

        if idx + 1 >= self.history.len() {
            // Return to saved input
            self.history_index = None;
            self.input = std::mem::take(&mut self.saved_input);
            self.cursor_pos = self.input.len();
        } else {
            self.history_index = Some(idx + 1);
            if let Some(entry) = self.history.get(idx + 1) {
                self.input = entry.command.clone();
                self.cursor_pos = self.input.len();
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

    /// Clear input buffer.
    pub fn clear_input(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
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
}
