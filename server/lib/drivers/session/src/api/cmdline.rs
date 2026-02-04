//! Command-line mode extension for session.
//!
//! Provides a shared prompt type that commands can set and the runner can read.
//! This enables the runner to activate cmdline with the correct prompt type
//! without depending on policy modules.

use crate::{SessionExtension, TextInputSink};

/// Command-line prompt type for session extensions.
///
/// This is a minimal extension that stores the pending prompt type.
/// Commands set this when entering command-line mode, and the runner
/// reads it to determine the display prompt character.
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
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmdline_state_default() {
        let state = CmdlineState::default();
        assert!(!state.is_active());
        assert_eq!(state.prompt(), CmdlinePrompt::Command);
        assert!(state.input().is_empty());
    }

    #[test]
    fn test_cmdline_prompt_chars() {
        assert_eq!(CmdlinePrompt::Command.char(), ':');
        assert_eq!(CmdlinePrompt::SearchForward.char(), '/');
        assert_eq!(CmdlinePrompt::SearchBackward.char(), '?');
    }

    #[test]
    fn test_cmdline_prompt_is_search() {
        assert!(!CmdlinePrompt::Command.is_search());
        assert!(CmdlinePrompt::SearchForward.is_search());
        assert!(CmdlinePrompt::SearchBackward.is_search());
    }

    #[test]
    fn test_enter_and_exit() {
        let mut state = CmdlineState::default();

        state.enter(CmdlinePrompt::SearchForward);
        assert!(state.is_active());
        assert_eq!(state.prompt(), CmdlinePrompt::SearchForward);

        state.exit();
        assert!(!state.is_active());
        // prompt is preserved after exit so runner can read it
        assert_eq!(state.prompt(), CmdlinePrompt::SearchForward);
    }

    #[test]
    fn test_insert_and_take() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::SearchForward);

        state.insert_char('f');
        state.insert_char('o');
        state.insert_char('o');
        assert_eq!(state.input(), "foo");
        assert_eq!(state.cursor(), 3);

        let taken = state.take_cmdline_input();
        assert_eq!(taken, "foo");
        assert!(state.input().is_empty());
    }

    #[test]
    fn test_backspace() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::SearchForward);

        state.insert_char('a');
        state.insert_char('b');
        state.backspace();
        assert_eq!(state.input(), "a");

        state.backspace();
        assert!(state.input().is_empty());

        // Backspace on empty should be no-op
        state.backspace();
        assert!(state.input().is_empty());
    }

    #[test]
    fn test_cancel_clears_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::SearchForward);

        state.insert_char('t');
        state.insert_char('e');
        state.insert_char('s');
        state.insert_char('t');

        state.cancel();
        assert!(!state.is_active());
        assert!(state.was_cancelled());
        assert!(state.input().is_empty());
    }
}
