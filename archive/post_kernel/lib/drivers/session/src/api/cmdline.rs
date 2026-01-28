//! Command-line mode extension for session.
//!
//! Provides a shared prompt type that commands can set and the runner can read.
//! This enables the runner to activate cmdline with the correct prompt type
//! without depending on policy modules.

use crate::SessionExtension;

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
}

impl SessionExtension for CmdlineState {
    fn create() -> Self {
        Self::default()
    }
}

impl CmdlineState {
    /// Enter cmdline mode with specified prompt type.
    pub const fn enter(&mut self, prompt: CmdlinePrompt) {
        self.active = true;
        self.prompt = prompt;
        self.cancelled = false;
    }

    /// Exit cmdline mode (execute action).
    ///
    /// Note: Preserves `prompt` so runner can read it after deactivation
    /// to determine what action to take (search vs ex command).
    pub const fn exit(&mut self) {
        self.active = false;
        self.cancelled = false;
        // prompt is preserved - runner reads it after exit
    }

    /// Cancel cmdline mode (don't execute action).
    ///
    /// Note: Preserves `prompt` for consistency, though `was_cancelled`
    /// prevents execution regardless.
    pub const fn cancel(&mut self) {
        self.active = false;
        self.cancelled = true;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmdline_state_default() {
        let state = CmdlineState::default();
        assert!(!state.is_active());
        assert_eq!(state.prompt(), CmdlinePrompt::Command);
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
}
