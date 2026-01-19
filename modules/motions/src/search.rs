//! Search motion commands.
//!
//! Implements vim search commands: `/`, `?`, `n`, `N`, `*`, `#`, `:noh`.
//!
//! These commands use the search infrastructure from the runner:
//! - `/` and `?` return `SearchAction::EnterSearchMode` to start input mode
//! - `n` and `N` return `SearchAction::Next/Previous` for repeat search
//! - `*` and `#` return `SearchAction::WordUnderCursor` for word search
//! - `:noh` returns `SearchAction::ClearHighlight` to clear highlighting

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult, SearchAction},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// =============================================================================
// Search Forward (/)
// =============================================================================

/// Search forward for a pattern.
///
/// Press `/` to enter search mode. Type a pattern and press Enter to search.
/// The cursor moves to the first match after the current position.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchForward;

impl Command for SearchForward {
    fn id(&self) -> CommandId {
        ids::SEARCH_FORWARD
    }

    fn description(&self) -> &'static str {
        "Search forward (/)"
    }
}

impl CommandHandler for SearchForward {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::EnterSearchMode {
            direction: reovim_driver_command::SearchDirection::Forward,
        })
    }
}

// =============================================================================
// Search Backward (?)
// =============================================================================

/// Search backward for a pattern.
///
/// Press `?` to enter search mode. Type a pattern and press Enter to search.
/// The cursor moves to the first match before the current position.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchBackward;

impl Command for SearchBackward {
    fn id(&self) -> CommandId {
        ids::SEARCH_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Search backward (?)"
    }
}

impl CommandHandler for SearchBackward {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::EnterSearchMode {
            direction: reovim_driver_command::SearchDirection::Backward,
        })
    }
}

// =============================================================================
// Search Next (n)
// =============================================================================

/// Go to next search match.
///
/// Press `n` to move to the next occurrence of the last search pattern
/// in the same direction as the original search.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchNext;

impl Command for SearchNext {
    fn id(&self) -> CommandId {
        ids::SEARCH_NEXT
    }

    fn description(&self) -> &'static str {
        "Go to next search match (n)"
    }
}

impl CommandHandler for SearchNext {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::Next)
    }
}

// =============================================================================
// Search Previous (N)
// =============================================================================

/// Go to previous search match.
///
/// Press `N` to move to the previous occurrence of the last search pattern
/// (opposite direction from the original search).
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchPrevious;

impl Command for SearchPrevious {
    fn id(&self) -> CommandId {
        ids::SEARCH_PREV
    }

    fn description(&self) -> &'static str {
        "Go to previous search match (N)"
    }
}

impl CommandHandler for SearchPrevious {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::Previous)
    }
}

// =============================================================================
// Search Word Under Cursor Forward (*)
// =============================================================================

/// Search for word under cursor forward.
///
/// Press `*` to search forward for the word under the cursor.
/// Word boundaries are automatically added to the pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchWordForward;

impl Command for SearchWordForward {
    fn id(&self) -> CommandId {
        ids::SEARCH_WORD_FORWARD
    }

    fn description(&self) -> &'static str {
        "Search word under cursor forward (*)"
    }
}

impl CommandHandler for SearchWordForward {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::WordUnderCursor {
            direction: reovim_driver_command::SearchDirection::Forward,
        })
    }
}

// =============================================================================
// Search Word Under Cursor Backward (#)
// =============================================================================

/// Search for word under cursor backward.
///
/// Press `#` to search backward for the word under the cursor.
/// Word boundaries are automatically added to the pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchWordBackward;

impl Command for SearchWordBackward {
    fn id(&self) -> CommandId {
        ids::SEARCH_WORD_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Search word under cursor backward (#)"
    }
}

impl CommandHandler for SearchWordBackward {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::WordUnderCursor {
            direction: reovim_driver_command::SearchDirection::Backward,
        })
    }
}

// =============================================================================
// Clear Search Highlight (:noh)
// =============================================================================

/// Clear search highlighting.
///
/// Use `:noh` or `:nohlsearch` to clear search highlighting.
/// The last search pattern is preserved for `n` and `N`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClearSearchHighlight;

impl Command for ClearSearchHighlight {
    fn id(&self) -> CommandId {
        ids::CLEAR_SEARCH_HIGHLIGHT
    }

    fn description(&self) -> &'static str {
        "Clear search highlighting (:noh)"
    }
}

impl CommandHandler for ClearSearchHighlight {
    fn execute(
        &self,
        _ctx: &mut reovim_kernel::api::v1::KernelContext,
        _args: &CommandContext,
    ) -> CommandResult {
        CommandResult::SearchAction(SearchAction::ClearHighlight)
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all search motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(SearchForward),
        Box::new(SearchBackward),
        Box::new(SearchNext),
        Box::new(SearchPrevious),
        Box::new(SearchWordForward),
        Box::new(SearchWordBackward),
        Box::new(ClearSearchHighlight),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_forward_id() {
        let cmd = SearchForward;
        assert_eq!(cmd.id().name(), "search-forward");
        assert_eq!(cmd.description(), "Search forward (/)");
    }

    #[test]
    fn test_search_backward_id() {
        let cmd = SearchBackward;
        assert_eq!(cmd.id().name(), "search-backward");
        assert_eq!(cmd.description(), "Search backward (?)");
    }

    #[test]
    fn test_search_next_id() {
        let cmd = SearchNext;
        assert_eq!(cmd.id().name(), "search-next");
        assert_eq!(cmd.description(), "Go to next search match (n)");
    }

    #[test]
    fn test_search_previous_id() {
        let cmd = SearchPrevious;
        assert_eq!(cmd.id().name(), "search-prev");
        assert_eq!(cmd.description(), "Go to previous search match (N)");
    }

    #[test]
    fn test_search_word_forward_id() {
        let cmd = SearchWordForward;
        assert_eq!(cmd.id().name(), "search-word-forward");
        assert_eq!(cmd.description(), "Search word under cursor forward (*)");
    }

    #[test]
    fn test_search_word_backward_id() {
        let cmd = SearchWordBackward;
        assert_eq!(cmd.id().name(), "search-word-backward");
        assert_eq!(cmd.description(), "Search word under cursor backward (#)");
    }

    #[test]
    fn test_clear_search_highlight_id() {
        let cmd = ClearSearchHighlight;
        assert_eq!(cmd.id().name(), "clear-search-highlight");
        assert_eq!(cmd.description(), "Clear search highlighting (:noh)");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 7); // /, ?, n, N, *, #, :noh
    }

    #[test]
    fn test_search_forward_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchForward;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(
            result,
            CommandResult::SearchAction(SearchAction::EnterSearchMode {
                direction: reovim_driver_command::SearchDirection::Forward
            })
        ));
    }

    #[test]
    fn test_search_backward_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchBackward;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(
            result,
            CommandResult::SearchAction(SearchAction::EnterSearchMode {
                direction: reovim_driver_command::SearchDirection::Backward
            })
        ));
    }

    #[test]
    fn test_search_next_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchNext;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(result, CommandResult::SearchAction(SearchAction::Next)));
    }

    #[test]
    fn test_search_previous_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchPrevious;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(result, CommandResult::SearchAction(SearchAction::Previous)));
    }

    #[test]
    fn test_search_word_forward_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchWordForward;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(
            result,
            CommandResult::SearchAction(SearchAction::WordUnderCursor {
                direction: reovim_driver_command::SearchDirection::Forward
            })
        ));
    }

    #[test]
    fn test_search_word_backward_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = SearchWordBackward;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(
            result,
            CommandResult::SearchAction(SearchAction::WordUnderCursor {
                direction: reovim_driver_command::SearchDirection::Backward
            })
        ));
    }

    #[test]
    fn test_clear_search_highlight_returns_search_action() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = ClearSearchHighlight;
        let result = cmd.execute(&mut ctx, &args);

        assert!(matches!(result, CommandResult::SearchAction(SearchAction::ClearHighlight)));
    }
}
