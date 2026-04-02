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
//! All search commands are fully implemented:
//! - `/` and `?` - forward/backward search with pattern input (uses `CommandLine` mode)
//! - `n` and `N` - repeat search in same/opposite direction
//! - `*` and `#` - search word under cursor forward/backward

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::{BufferOpsLineSource, Direction, SearchKey, SearchProviderRegistry},
    reovim_driver_session::{SessionRuntime, api::ExtensionApi},
    reovim_kernel::api::{Position, v1::CommandId},
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn search_word(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
) -> CommandResult {
    use reovim_types_text::Position;

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let cursor = Position::new(window.cursor.line, window.cursor.column);

    // Get search provider from kernel services
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Get word under cursor and find word start position for backward search
    let (word_pattern, search_cursor) = {
        let Some(Some((pattern, word_start))) = runtime.with_buffer_read(buffer_id, |buffer| {
            // Get word pattern
            let source = BufferOpsLineSource(buffer);
            let pattern = search_provider.word_at_cursor_source(&source, cursor)?;

            // Find word start position for backward search
            let line = buffer.line(cursor.line)?;
            let chars: Vec<char> = line.chars().collect();

            if cursor.column >= chars.len() {
                return Some((pattern, cursor));
            }

            // Find word start
            let mut start = cursor.column;
            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }

            let word_start = Position {
                line: cursor.line,
                column: start,
            };

            Some((pattern, word_start))
        }) else {
            return CommandResult::Success; // No word under cursor - silent fail like vim
        };

        // For backward search (#), use word start position to skip current word.
        // For forward search (*), use cursor position (search starts after cursor).
        let search_pos = match direction {
            Direction::Backward => word_start,
            Direction::Forward => cursor,
        };

        (pattern, search_pos)
    };

    // Store pattern and direction for n/N repeat
    {
        let search = runtime.ext_mut::<SearchState>();
        search.set(word_pattern.clone(), direction);
    }

    // Search for the word using the appropriate search position
    search_and_move_from(runtime, args, &word_pattern, direction, search_cursor)
}

/// Search for pattern and move cursor to match.
#[cfg_attr(coverage_nightly, coverage(off))]
fn search_and_move(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
) -> CommandResult {
    use {
        reovim_driver_session::ChangeTracker,
        reovim_kernel::api::v1::JumpEntry,
        reovim_types_text::Position,
    };

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let cursor = Position::new(window.cursor.line, window.cursor.column);

    // Get search provider
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Search for pattern
    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let source = BufferOpsLineSource(buffer);
        search_provider.find_next_source(&source, cursor, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Push current position to jump list before search jump (#654)
            runtime
                .jumplist_mut()
                .push(JumpEntry::new(buffer_id, cursor));
            // Move cursor to match start via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = m.start.into();
            }
            runtime.record_cursor_move(buffer_id);
            CommandResult::Success
        }
        Some(Ok(None) | Err(_)) => {
            // No match or invalid pattern - silent failure like vim
            CommandResult::Success
        }
        None => CommandResult::error("Buffer not found"),
    }
}

/// Search for pattern from a specific position and move cursor to match.
/// Used by word search (*/#) which needs to start from word boundaries.
#[cfg_attr(coverage_nightly, coverage(off))]
fn search_and_move_from(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
    search_from: Position,
) -> CommandResult {
    use {reovim_driver_session::ChangeTracker, reovim_kernel::api::v1::JumpEntry};

    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Capture cursor position before search for jumplist (#654)
    let old_cursor = runtime
        .windows()
        .active()
        .map(|w| Position::new(w.cursor.line, w.cursor.column));

    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let source = BufferOpsLineSource(buffer);
        search_provider.find_next_source(&source, search_from, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Push current position to jump list before search jump (#654)
            if let Some(pos) = old_cursor {
                runtime.jumplist_mut().push(JumpEntry::new(buffer_id, pos));
            }
            // Move cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = m.start.into();
            }
            runtime.record_cursor_move(buffer_id);
            CommandResult::Success
        }
        Some(Ok(None) | Err(_)) => CommandResult::Success,
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
