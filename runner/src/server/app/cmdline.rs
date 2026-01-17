//! Command-line mode state for : commands.
//!
//! Tracks input buffer and command history for Vim-style Ex commands.
//! Similar to search.rs but for command-line mode instead of search.

// ============================================================================
// Command-line Infrastructure
// ============================================================================

/// Command-line state for the session.
///
/// Tracks input buffer and whether command-line mode is active.
/// History support can be added later.
#[derive(Debug, Clone, Default)]
pub struct CommandLineState {
    /// Input buffer for current command.
    pub input_buffer: String,
    /// Whether we're in command-line mode.
    pub active: bool,
    /// Cursor position within input buffer.
    pub cursor_pos: usize,
}

impl CommandLineState {
    /// Create a new command-line state with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter command-line mode.
    pub fn enter(&mut self) {
        self.active = true;
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    /// Cancel command-line mode (Esc).
    pub fn cancel(&mut self) {
        self.active = false;
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    /// Complete command-line input and return the command.
    ///
    /// Returns None if no input or not active.
    pub fn complete(&mut self) -> Option<String> {
        if !self.active || self.input_buffer.is_empty() {
            self.active = false;
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return None;
        }

        self.active = false;
        let command = std::mem::take(&mut self.input_buffer);
        self.cursor_pos = 0;

        Some(command)
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, ch: char) {
        if self.cursor_pos >= self.input_buffer.len() {
            self.input_buffer.push(ch);
        } else {
            self.input_buffer.insert(self.cursor_pos, ch);
        }
        self.cursor_pos += 1;
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input_buffer.remove(self.cursor_pos);
        }
    }

    /// Get the current input buffer.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input_buffer
    }

    /// Check if command-line mode is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }
}
