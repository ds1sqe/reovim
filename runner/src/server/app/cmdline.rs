//! Command-line mode state for `:`, `/`, and `?` commands.
//!
//! Tracks input buffer and prompt type for Vim-style Ex commands and search.
//! History support can be added later.

// ============================================================================
// Command-line Infrastructure
// ============================================================================

/// Type of prompt being displayed.
///
/// Distinguishes between Ex commands (`:`) and search patterns (`/`, `?`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptType {
    /// Ex command prompt (`:`)
    #[default]
    Command,
    /// Forward search prompt (`/`)
    SearchForward,
    /// Backward search prompt (`?`)
    SearchBackward,
}

impl PromptType {
    /// Get the prompt character for display.
    #[must_use]
    pub const fn char(self) -> char {
        match self {
            Self::Command => ':',
            Self::SearchForward => '/',
            Self::SearchBackward => '?',
        }
    }

    /// Check if this is a search prompt.
    #[must_use]
    pub const fn is_search(self) -> bool {
        matches!(self, Self::SearchForward | Self::SearchBackward)
    }
}

/// Command-line state for the session.
///
/// Tracks input buffer and whether command-line mode is active.
/// Used for Ex commands (`:`) and search patterns (`/`, `?`).
#[derive(Debug, Clone, Default)]
pub struct CommandLineState {
    /// Input buffer for current command/pattern.
    pub input_buffer: String,
    /// Whether we're in command-line mode.
    pub active: bool,
    /// Cursor position within input buffer.
    pub cursor_pos: usize,
    /// Type of prompt (command or search).
    pub prompt_type: PromptType,
}

impl CommandLineState {
    /// Create a new command-line state with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter command-line mode with default (Command) prompt.
    pub fn enter(&mut self) {
        self.enter_with_prompt(PromptType::Command);
    }

    /// Enter command-line mode with specified prompt type.
    pub fn enter_with_prompt(&mut self, prompt: PromptType) {
        self.active = true;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.prompt_type = prompt;
    }

    /// Cancel command-line mode (Esc).
    pub fn cancel(&mut self) {
        self.active = false;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.prompt_type = PromptType::Command;
    }

    /// Complete command-line input and return the input string.
    ///
    /// Returns None if no input or not active.
    /// The `prompt_type` is preserved for the caller to check.
    pub fn complete(&mut self) -> Option<String> {
        if !self.active || self.input_buffer.is_empty() {
            self.active = false;
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return None;
        }

        self.active = false;
        let input = std::mem::take(&mut self.input_buffer);
        self.cursor_pos = 0;
        // Note: prompt_type is preserved so caller can check it

        Some(input)
    }

    /// Get the current prompt type.
    #[must_use]
    pub const fn prompt_type(&self) -> PromptType {
        self.prompt_type
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
