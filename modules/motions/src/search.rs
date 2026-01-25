//! Search motion commands.
//!
//! Implements vim search commands: `/`, `?`, `n`, `N`, `*`, `#`, `:noh`.
//!
//! # Architecture
//!
//! - **Mechanism**: `SearchProvider` trait (in `reovim-driver-search`)
//! - **Policy**: These commands wire keys to search functionality
//!
//! # Search State
//!
//! The `SearchState` session extension stores:
//! - Last search pattern (for n/N repeat)
//! - Last search direction (for correct n/N behavior)
//!
//! # Current Status
//!
//! - `*`, `#`, `n`, `N` are fully implemented
//! - `/`, `?` require command-line mode (#435)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::{Direction, SearchKey, SearchProviderRegistry},
    reovim_driver_session::{SessionRuntime, api::ExtensionApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, search_state::SearchState};

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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#338): Implement via command-line mode
        // Currently requires command-line input mode which is deferred to Phase 8
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#338): Implement via command-line mode
        // Currently requires command-line input mode which is deferred to Phase 8
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last search pattern and direction
        let (pattern, direction) = {
            let search = runtime.ext_mut::<SearchState>();
            match search.pattern_for_repeat() {
                Some(p) => (p.to_string(), search.direction_for_repeat()),
                None => return CommandResult::Success, // No pattern to repeat
            }
        };

        search_and_move(runtime, args, &pattern, direction)
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last search pattern and REVERSED direction
        let (pattern, direction) = {
            let search = runtime.ext_mut::<SearchState>();
            match search.pattern_for_repeat() {
                Some(p) => (p.to_string(), search.direction_for_opposite()),
                None => return CommandResult::Success, // No pattern to repeat
            }
        };

        search_and_move(runtime, args, &pattern, direction)
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        search_word(runtime, args, Direction::Forward)
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        search_word(runtime, args, Direction::Backward)
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO: When highlighting is implemented, clear it here
        // For now, this is a no-op since highlighting isn't implemented yet
        CommandResult::Success
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Search for word under cursor in the given direction.
fn search_word(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
) -> CommandResult {
    use reovim_driver_session::api::BufferApi;

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(cursor) = runtime.buffer_position(buffer_id) else {
        return CommandResult::error("No cursor position");
    };

    // Get search provider from kernel services
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Get word under cursor
    let Some(Some(word_pattern)) = runtime
        .with_buffer_read(buffer_id, |buffer| search_provider.word_at_cursor(buffer, cursor))
    else {
        return CommandResult::Success; // No word under cursor - silent fail like vim
    };

    // Store pattern and direction for n/N repeat
    {
        let search = runtime.ext_mut::<SearchState>();
        search.set(word_pattern.clone(), direction);
    }

    // Search for the word
    search_and_move(runtime, args, &word_pattern, direction)
}

/// Search for pattern and move cursor to match.
fn search_and_move(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
) -> CommandResult {
    use reovim_driver_session::api::{BufferApi, ChangeTracker};

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(cursor) = runtime.buffer_position(buffer_id) else {
        return CommandResult::error("No cursor position");
    };

    // Get search provider
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Search for pattern
    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        search_provider.find_next(buffer, cursor, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Move cursor to match start - update BOTH buffer position and window cursor
            runtime.set_buffer_position(buffer_id, m.start);
            runtime.move_cursor(buffer_id, m.start);
            runtime.record_cursor_move(buffer_id);
            CommandResult::Success
        }
        Some(Ok(None)) => {
            // No match found - silent, like vim
            CommandResult::Success
        }
        Some(Err(_)) => {
            // Invalid pattern - silent failure like vim
            CommandResult::Success
        }
        None => CommandResult::error("Buffer not found"),
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
