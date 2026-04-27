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
#[path = "cli_panel_tests.rs"]
mod tests;
