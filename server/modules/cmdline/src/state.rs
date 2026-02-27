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

    #[test]
    fn test_cancel_resets_cursor() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.insert_char('q');
        assert_eq!(state.cursor(), 2);

        state.cancel();
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_exit_preserves_prompt() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::SearchBackward);
        state.exit();

        // Prompt should be preserved after exit for runner to read
        assert_eq!(state.prompt(), CmdlinePrompt::SearchBackward);
        assert!(!state.was_cancelled());
    }

    #[test]
    fn test_enter_resets_cancelled_flag() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.cancel();
        assert!(state.was_cancelled());

        // Entering again should clear cancelled flag
        state.enter(CmdlinePrompt::SearchForward);
        assert!(!state.was_cancelled());
    }

    #[test]
    fn test_enter_clears_previous_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.insert_char('q');
        assert_eq!(state.input(), "wq");

        // Entering again should clear input
        state.enter(CmdlinePrompt::SearchForward);
        assert!(state.input().is_empty());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_take_cmdline_input_resets_cursor() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        assert_eq!(state.cursor(), 1);

        let input = state.take_cmdline_input();
        assert_eq!(input, "w");
        assert_eq!(state.cursor(), 0);
        assert!(state.input().is_empty());
    }

    #[test]
    fn test_insert_char_at_beginning_of_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);

        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.input(), "bc");
        assert_eq!(state.cursor(), 2);

        state.cursor = 0;
        state.insert_char('a');
        assert_eq!(state.input(), "abc");
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn test_insert_char_in_middle_of_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('c');
        assert_eq!(state.input(), "ac");

        state.cursor = 1;
        state.insert_char('b');
        assert_eq!(state.input(), "abc");
        assert_eq!(state.cursor(), 2);
    }

    #[test]
    fn test_backspace_at_beginning_is_noop() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.cursor = 0;
        state.backspace();
        assert!(state.input().is_empty());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_cmdline_prompt_default() {
        let prompt = CmdlinePrompt::default();
        assert_eq!(prompt, CmdlinePrompt::Command);
        assert_eq!(prompt.char(), ':');
        assert!(!prompt.is_search());
    }

    #[test]
    fn test_cmdline_prompt_clone_copy() {
        let prompt = CmdlinePrompt::SearchForward;
        let cloned = prompt;
        assert_eq!(prompt, cloned);
    }

    #[test]
    fn test_cmdline_state_debug() {
        let state = CmdlineState::default();
        let debug = format!("{state:?}");
        assert!(debug.contains("CmdlineState"));
    }

    #[test]
    fn test_session_extension_create() {
        let state = CmdlineState::create();
        assert!(!state.is_active());
        assert!(state.input().is_empty());
    }

    #[test]
    fn test_text_input_sink_integration() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);

        // Use the TextInputSink trait method
        TextInputSink::insert_char(&mut state, 'h');
        TextInputSink::insert_char(&mut state, 'i');

        assert_eq!(state.input(), "hi");
    }

    #[test]
    fn test_as_text_input_sink_returns_some() {
        let mut state = CmdlineState::default();
        let sink = SessionExtension::as_text_input_sink(&mut state);
        assert!(sink.is_some());
    }

    // -- Enhanced editing tests (#451) --

    #[test]
    fn test_move_cursor_left() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.cursor(), 2);

        state.move_cursor_left();
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn test_move_cursor_left_at_start() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.move_cursor_left();
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_move_cursor_right() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('b');
        state.cursor = 0;

        state.move_cursor_right();
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn test_move_cursor_right_at_end() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        assert_eq!(state.cursor(), 1);

        state.move_cursor_right();
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn test_move_to_start() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.cursor(), 3);

        state.move_to_start();
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_move_to_end() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('b');
        state.cursor = 0;

        state.move_to_end();
        assert_eq!(state.cursor(), 2);
    }

    #[test]
    fn test_delete_at_cursor() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        state.cursor = 1;

        state.delete_at_cursor();
        assert_eq!(state.input(), "ac");
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn test_delete_at_cursor_at_end() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        assert_eq!(state.cursor(), 1);

        state.delete_at_cursor();
        assert_eq!(state.input(), "a");
    }

    #[test]
    fn test_delete_word_back() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        for ch in "hello world".chars() {
            state.insert_char(ch);
        }
        assert_eq!(state.cursor(), 11);

        state.delete_word_back();
        assert_eq!(state.input(), "hello ");
        assert_eq!(state.cursor(), 6);
    }

    #[test]
    fn test_delete_word_back_at_start() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.cursor = 0;

        state.delete_word_back();
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_delete_word_back_trailing_spaces() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        for ch in "foo   ".chars() {
            state.insert_char(ch);
        }
        assert_eq!(state.cursor(), 6);

        state.delete_word_back();
        assert_eq!(state.input(), "");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_delete_to_start() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        for ch in "hello world".chars() {
            state.insert_char(ch);
        }
        state.cursor = 5;

        state.delete_to_start();
        assert_eq!(state.input(), " world");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn test_delete_to_start_at_start() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.cursor = 0;

        state.delete_to_start();
        assert_eq!(state.input(), "a");
        assert_eq!(state.cursor(), 0);
    }

    // -- History tests (#451) --

    #[test]
    fn test_history_up_empty() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.history_up();
        assert!(state.input().is_empty());
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_history_up_saves_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        for ch in "w".chars() {
            state.insert_char(ch);
        }
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        for ch in "current".chars() {
            state.insert_char(ch);
        }

        state.history_up();
        assert_eq!(state.input(), "w");
        assert_eq!(state.saved_input, "current");
    }

    #[test]
    fn test_history_up_multiple() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('b');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('c');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.history_up();
        assert_eq!(state.input(), "c");
        state.history_up();
        assert_eq!(state.input(), "b");
        state.history_up();
        assert_eq!(state.input(), "a");
    }

    #[test]
    fn test_history_up_at_oldest() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.history_up();
        assert_eq!(state.input(), "a");

        state.history_up();
        assert_eq!(state.input(), "a");
    }

    #[test]
    fn test_history_down() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('a');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('b');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.history_up();
        state.history_up();
        state.history_down();
        assert_eq!(state.input(), "b");
    }

    #[test]
    fn test_history_down_restores_input() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        for ch in "new".chars() {
            state.insert_char(ch);
        }

        state.history_up();
        state.history_down();
        assert_eq!(state.input(), "new");
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_history_down_not_navigating() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('x');
        state.history_down();
        assert_eq!(state.input(), "x");
    }

    #[test]
    fn test_push_to_history() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        for ch in "wq".chars() {
            state.insert_char(ch);
        }
        state.push_to_history();
        assert_eq!(state.command_history.len(), 1);
        assert_eq!(state.command_history[0], "wq");
    }

    #[test]
    fn test_push_to_history_empty() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.push_to_history();
        assert!(state.command_history.is_empty());
    }

    #[test]
    fn test_push_to_history_dedup() {
        let mut state = CmdlineState::default();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('q');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.push_to_history();

        assert_eq!(state.command_history.len(), 2);
        assert_eq!(state.command_history[0], "q");
        assert_eq!(state.command_history[1], "w");
    }

    #[test]
    fn test_push_to_history_max() {
        let mut state = CmdlineState::default();
        for i in 0..=MAX_HISTORY {
            state.enter(CmdlinePrompt::Command);
            state.input = format!("cmd{i}");
            state.push_to_history();
        }
        assert_eq!(state.command_history.len(), MAX_HISTORY);
        assert_eq!(state.command_history[0], "cmd1");
    }

    #[test]
    fn test_separate_command_search_history() {
        let mut state = CmdlineState::default();

        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.push_to_history();

        state.enter(CmdlinePrompt::SearchForward);
        for ch in "foo".chars() {
            state.insert_char(ch);
        }
        state.push_to_history();

        assert_eq!(state.command_history.len(), 1);
        assert_eq!(state.search_history.len(), 1);
        assert_eq!(state.command_history[0], "w");
        assert_eq!(state.search_history[0], "foo");
    }

    // -- Completion tests (#451) --

    #[test]
    fn test_set_completions() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);
        assert_eq!(state.completions().len(), 2);
        assert!(state.completion_index().is_none());
    }

    #[test]
    fn test_clear_completions() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string()]);
        state.clear_completions();
        assert!(state.completions().is_empty());
        assert!(state.completion_index().is_none());
    }

    #[test]
    fn test_complete_next() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

        assert!(state.complete_next());
        assert_eq!(state.input(), "write");
        assert_eq!(state.completion_index(), Some(0));

        assert!(state.complete_next());
        assert_eq!(state.input(), "wq");
        assert_eq!(state.completion_index(), Some(1));
    }

    #[test]
    fn test_complete_next_wraps() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

        state.complete_next();
        state.complete_next();
        state.complete_next();
        assert_eq!(state.input(), "write");
        assert_eq!(state.completion_index(), Some(0));
    }

    #[test]
    fn test_complete_prev() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions(
            "w".to_string(),
            vec!["write".to_string(), "wq".to_string(), "wall".to_string()],
        );

        assert!(state.complete_prev());
        assert_eq!(state.input(), "wall");
        assert_eq!(state.completion_index(), Some(2));

        assert!(state.complete_prev());
        assert_eq!(state.input(), "wq");
        assert_eq!(state.completion_index(), Some(1));
    }

    #[test]
    fn test_complete_prev_wraps() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

        state.complete_next();
        state.complete_prev();
        assert_eq!(state.input(), "wq");
    }

    #[test]
    fn test_complete_empty() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        assert!(!state.complete_next());
        assert!(!state.complete_prev());
    }

    #[test]
    fn test_insert_char_clears_completions() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string()]);
        state.complete_next();
        assert_eq!(state.completion_index(), Some(0));

        state.insert_char('x');
        assert!(state.completions().is_empty());
        assert!(state.completion_index().is_none());
    }

    #[test]
    fn test_backspace_clears_completions() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.set_completions("w".to_string(), vec!["write".to_string()]);
        state.complete_next();

        state.backspace();
        assert!(state.completions().is_empty());
    }

    #[test]
    fn test_delete_at_cursor_clears_completions() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.insert_char('r');
        state.cursor = 0;
        state.set_completions("w".to_string(), vec!["write".to_string()]);

        state.delete_at_cursor();
        assert!(state.completions().is_empty());
    }

    #[test]
    fn test_search_history_navigation() {
        let mut state = CmdlineState::default();

        // Build search history
        state.enter(CmdlinePrompt::SearchForward);
        for ch in "foo".chars() {
            state.insert_char(ch);
        }
        state.push_to_history();

        state.enter(CmdlinePrompt::SearchForward);
        for ch in "bar".chars() {
            state.insert_char(ch);
        }
        state.push_to_history();

        // Navigate search history (exercises current_history with search prompt)
        state.enter(CmdlinePrompt::SearchForward);
        state.history_up();
        assert_eq!(state.input(), "bar");
        state.history_up();
        assert_eq!(state.input(), "foo");
    }

    #[test]
    fn test_enter_resets_history_navigation() {
        let mut state = CmdlineState::default();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.push_to_history();

        state.enter(CmdlinePrompt::Command);
        state.history_up();
        assert!(state.history_index.is_some());

        state.enter(CmdlinePrompt::Command);
        assert!(state.history_index.is_none());
        assert!(state.saved_input.is_empty());
    }
}
