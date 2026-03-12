#![allow(clippy::doc_markdown)] // Session extensions use CamelCase in docs
//! Command-line mode state - POLICY layer.
//!
//! `CmdlineState` stores all command-line mode data: input buffer, cursor,
//! prompt type, history, and completion state.
//!
//! # Architecture (#468)
//!
//! CmdlineState is a per-client `SessionExtension`. It lives in the module
//! layer (POLICY) because it defines HOW command-line mode behaves.
//! The driver layer only provides the `SessionExtension` trait (MECHANISM).

use reovim_driver_session::{SessionExtension, TextInputSink};

/// Command-line prompt type for session extensions.
///
/// Determines the display prompt character and history pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CmdlinePrompt {
    /// Ex command prompt (`:`)
    #[default]
    Command,
    /// Forward search prompt (`/`)
    SearchForward,
    /// Backward search prompt (`?`)
    SearchBackward,
}

impl CmdlinePrompt {
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

/// A message to display in the command-line area.
///
/// Used for ex-command errors ("E492: Not an editor command") and
/// informational messages. Cleared on next keypress in normal mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmdlineMessage {
    /// Error message (displayed with error highlighting).
    Error(String),
    /// Informational message (displayed normally).
    Info(String),
}

impl CmdlineMessage {
    /// Get the message text.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Error(s) | Self::Info(s) => s,
        }
    }

    /// Get the message kind as a string for serialization.
    #[must_use]
    pub const fn kind(&self) -> &str {
        match self {
            Self::Error(_) => "error",
            Self::Info(_) => "info",
        }
    }
}

/// Maximum number of history entries per type.
const MAX_HISTORY: usize = 100;

/// Session extension for command-line state.
///
/// Policy (modules) sets this when entering/exiting command-line mode.
/// Mechanism (runner) reads this to sync display state.
#[derive(Debug, Default)]
pub struct CmdlineState {
    /// Whether cmdline is active.
    active: bool,
    /// The prompt type for the current command-line session.
    prompt: CmdlinePrompt,
    /// Whether the exit was a cancellation (Escape) vs execution (Enter).
    cancelled: bool,
    /// Input text buffer for the command line.
    input: String,
    /// Cursor position within the input buffer.
    cursor: usize,
    /// History for `:` commands.
    command_history: Vec<String>,
    /// History for `/` and `?` searches.
    search_history: Vec<String>,
    /// Current history navigation index (`None` = new input).
    history_index: Option<usize>,
    /// Saved input when navigating history.
    saved_input: String,
    /// Available completion candidates.
    completions: Vec<String>,
    /// Currently selected completion index.
    completion_index: Option<usize>,
    /// The prefix that generated the current completions.
    completion_prefix: String,
    /// Message to display in the command-line area (cleared on next keypress).
    message: Option<CmdlineMessage>,
}

impl SessionExtension for CmdlineState {
    fn create() -> Self {
        Self::default()
    }

    /// `CmdlineState` accepts text input, so it implements `TextInputSink`.
    ///
    /// This enables the runner to route characters from command-line mode
    /// to this extension without string-based mode detection (#482).
    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        Some(self)
    }
}

/// `TextInputSink` implementation for `CmdlineState`.
///
/// Delegates to the existing `insert_char` method.
impl TextInputSink for CmdlineState {
    fn insert_char(&mut self, ch: char) {
        // Delegate to the existing method
        Self::insert_char(self, ch);
    }
}

impl CmdlineState {
    /// Enter cmdline mode with specified prompt type.
    pub fn enter(&mut self, prompt: CmdlinePrompt) {
        self.active = true;
        self.prompt = prompt;
        self.cancelled = false;
        self.input.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_input.clear();
        self.message = None;
    }

    /// Exit cmdline mode (execute action).
    ///
    /// Note: Preserves `prompt` so runner can read it after deactivation
    /// to determine what action to take (search vs ex command).
    pub const fn exit(&mut self) {
        self.active = false;
        self.cancelled = false;
        // prompt is preserved - runner reads it after exit
        // input is cleared by take_cmdline_input() before this is called
    }

    /// Cancel cmdline mode (don't execute action).
    ///
    /// Note: Preserves `prompt` for consistency, though `was_cancelled`
    /// prevents execution regardless.
    pub fn cancel(&mut self) {
        self.active = false;
        self.cancelled = true;
        self.input.clear();
        self.cursor = 0;
        // prompt is preserved for consistency
    }

    /// Check if cmdline is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Check if cmdline was cancelled (vs executed).
    #[must_use]
    pub const fn was_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Get the current prompt type.
    #[must_use]
    pub const fn prompt(&self) -> CmdlinePrompt {
        self.prompt
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, ch: char) {
        if self.cursor >= self.input.len() {
            self.input.push(ch);
        } else {
            self.input.insert(self.cursor, ch);
        }
        self.cursor += 1;
        self.clear_completions();
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
            self.clear_completions();
        }
    }

    /// Get the current input buffer.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Take ownership of the input, clearing the buffer.
    ///
    /// Called by commands when exiting cmdline mode to get the entered text.
    pub fn take_cmdline_input(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.input)
    }

    /// Get cursor position.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    // =========================================================================
    // Enhanced editing methods (#451)
    // =========================================================================

    /// Move cursor left by one position.
    pub const fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right by one position.
    pub const fn move_cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start of input.
    pub const fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end of input.
    pub const fn move_to_end(&mut self) {
        self.cursor = self.input.len();
    }

    /// Delete character at cursor position (Del key).
    pub fn delete_at_cursor(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.clear_completions();
        }
    }

    /// Delete word before cursor (Ctrl-W).
    pub fn delete_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut pos = self.cursor;
        // Skip trailing whitespace
        while pos > 0 && self.input.as_bytes()[pos - 1] == b' ' {
            pos -= 1;
        }
        // Delete back to start of word
        while pos > 0 && self.input.as_bytes()[pos - 1] != b' ' {
            pos -= 1;
        }
        self.input.drain(pos..self.cursor);
        self.cursor = pos;
        self.clear_completions();
    }

    /// Delete from cursor to start of line (Ctrl-U).
    pub fn delete_to_start(&mut self) {
        if self.cursor > 0 {
            self.input.drain(..self.cursor);
            self.cursor = 0;
            self.clear_completions();
        }
    }

    // =========================================================================
    // History methods (#451)
    // =========================================================================

    /// Get the history for the current prompt type.
    fn current_history(&self) -> &[String] {
        if self.prompt.is_search() {
            &self.search_history
        } else {
            &self.command_history
        }
    }

    /// Get mutable history for the current prompt type.
    const fn current_history_mut(&mut self) -> &mut Vec<String> {
        if self.prompt.is_search() {
            &mut self.search_history
        } else {
            &mut self.command_history
        }
    }

    /// Navigate to an older history entry (Up / Ctrl-P).
    pub fn history_up(&mut self) {
        let history_len = self.current_history().len();
        if history_len == 0 {
            return;
        }

        let idx = match self.history_index {
            None => {
                // Save current input and show most recent history
                self.saved_input.clone_from(&self.input);
                history_len - 1
            }
            Some(0) => return, // already at oldest
            Some(i) => i - 1,
        };

        self.history_index = Some(idx);
        let entry = self.current_history()[idx].clone();
        self.input = entry;
        self.cursor = self.input.len();
    }

    /// Navigate to a newer history entry (Down / Ctrl-N).
    pub fn history_down(&mut self) {
        let Some(idx) = self.history_index else {
            return; // not navigating history
        };

        let history_len = self.current_history().len();
        if idx + 1 >= history_len {
            // Past newest → restore saved input
            self.history_index = None;
            self.input = std::mem::take(&mut self.saved_input);
        } else {
            let entry = self.current_history()[idx + 1].clone();
            self.history_index = Some(idx + 1);
            self.input = entry;
        }
        self.cursor = self.input.len();
    }

    /// Push the current input to history (called on successful execution).
    pub fn push_to_history(&mut self) {
        if self.input.is_empty() {
            return;
        }

        let input_clone = self.input.clone();
        let history = self.current_history_mut();

        // Deduplicate: remove if already present
        history.retain(|entry| *entry != input_clone);
        history.push(input_clone);

        // Cap size
        if history.len() > MAX_HISTORY {
            history.remove(0);
        }
    }

    // =========================================================================
    // Completion methods (#451)
    // =========================================================================

    /// Set available completions for a given prefix.
    pub fn set_completions(&mut self, prefix: String, candidates: Vec<String>) {
        self.completion_prefix = prefix;
        self.completions = candidates;
        self.completion_index = None;
    }

    /// Clear all completion state.
    pub fn clear_completions(&mut self) {
        self.completions.clear();
        self.completion_index = None;
        self.completion_prefix.clear();
    }

    /// Cycle to the next completion (Tab).
    ///
    /// Returns `true` if a completion was applied, `false` if no completions exist.
    pub fn complete_next(&mut self) -> bool {
        if self.completions.is_empty() {
            return false;
        }
        let idx = match self.completion_index {
            None => 0,
            Some(i) => (i + 1) % self.completions.len(),
        };
        self.completion_index = Some(idx);
        self.apply_completion(idx);
        true
    }

    /// Cycle to the previous completion (Shift-Tab).
    ///
    /// Returns `true` if a completion was applied, `false` if no completions exist.
    pub fn complete_prev(&mut self) -> bool {
        if self.completions.is_empty() {
            return false;
        }
        let idx = match self.completion_index {
            None | Some(0) => self.completions.len() - 1,
            Some(i) => i - 1,
        };
        self.completion_index = Some(idx);
        self.apply_completion(idx);
        true
    }

    /// Apply a completion at the given index to the input.
    fn apply_completion(&mut self, idx: usize) {
        if let Some(completion) = self.completions.get(idx) {
            self.input = completion.clone();
            self.cursor = self.input.len();
        }
    }

    /// Get the current completions list.
    #[must_use]
    pub fn completions(&self) -> &[String] {
        &self.completions
    }

    /// Get the currently selected completion index.
    #[must_use]
    pub const fn completion_index(&self) -> Option<usize> {
        self.completion_index
    }

    // =========================================================================
    // Message methods (#558)
    // =========================================================================

    /// Set a message to display in the command-line area.
    ///
    /// The message persists until cleared (by next keypress or entering cmdline).
    pub fn set_message(&mut self, message: CmdlineMessage) {
        self.message = Some(message);
    }

    /// Clear the current message.
    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Get the current message, if any.
    #[must_use]
    pub const fn message(&self) -> Option<&CmdlineMessage> {
        self.message.as_ref()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
