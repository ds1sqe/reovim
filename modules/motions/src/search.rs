//! Search motion commands.
//!
//! Implements vim search commands: `/`, `?`, `n`, `N`, `*`, `#`, `:noh`.
//!
//! # Architecture (Epic #385)
//!
//! These commands are stubs that will be implemented when `SessionContext`
//! provides direct access to search state. The actual search functionality
//! will be handled by the vim resolver or a dedicated search module.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
        // TODO: Implement via SessionContext when available
        CommandResult::Success
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
}
