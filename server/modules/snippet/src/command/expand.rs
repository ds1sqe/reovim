//! `ExpandSnippet` command handler (#136).
//!
//! Expands the snippet whose prefix matches the word before the cursor.

use std::sync::Arc;

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
    },
    reovim_kernel::api::v1::{CommandId, Edit, Position},
};

use crate::{
    engine::ActiveSnippet, ids, parser, provider::SnippetRegistry, state::SnippetSessionState,
};

/// Expand snippet at cursor.
///
/// Reads the word before the cursor, looks up a snippet with that prefix,
/// parses and expands the snippet body, replaces the trigger word, and
/// enters snippet navigation mode.
pub struct ExpandSnippet {
    registry: Arc<SnippetRegistry>,
}

impl ExpandSnippet {
    /// Create a new expand command with access to the snippet registry.
    #[must_use]
    pub const fn new(registry: Arc<SnippetRegistry>) -> Self {
        Self { registry }
    }
}

impl Command for ExpandSnippet {
    fn id(&self) -> CommandId {
        ids::EXPAND
    }

    fn description(&self) -> &'static str {
        "Expand snippet at cursor"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ExpandSnippet {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("no active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("no active window");
        };
        let cursor = Position::new(window.cursor.line, window.cursor.column);

        // Extract word before cursor as the trigger prefix
        let prefix = runtime
            .buffer_line(buffer_id, cursor.line)
            .map(|line| extract_word_before(&line, cursor.column))
            .unwrap_or_default();

        if prefix.is_empty() {
            return CommandResult::Success; // nothing to expand
        }

        // Determine filetype (placeholder: "global" for now)
        let filetype = "global";

        // Look up snippet by prefix
        let Some(definition) = self.registry.find_by_prefix(filetype, &prefix) else {
            return CommandResult::Success; // no matching snippet
        };

        // Parse the snippet body (lazy: raw string → AST)
        let body = match parser::parse(&definition.body_raw) {
            Ok(body) => body,
            Err(e) => return CommandResult::error(&format!("snippet parse error: {e}")),
        };

        // Delete the trigger word from the buffer
        let trigger_start = Position::new(cursor.line, cursor.column - prefix.len());
        runtime.delete_range(buffer_id, trigger_start, cursor);

        // Expand the snippet body
        let (expanded_text, mut active_snippet) = ActiveSnippet::expand(&body, trigger_start);

        // Insert expanded text at trigger position
        runtime.insert_text(buffer_id, trigger_start, &expanded_text);

        // Extract first tab stop info and delete its placeholder if present.
        // This must happen before storing the snippet so positions are correct.
        let has_tab_stops = !active_snippet.is_done();
        let first_stop_range = active_snippet.current().map(|ts| {
            let start = ts.start;
            let end = ts.end;
            let placeholder = ts.placeholder.clone();
            (start, end, placeholder)
        });

        // If first tab stop has a placeholder, pre-adjust all positions
        if let Some((start, end, ref placeholder)) = first_stop_range
            && start != end
        {
            let edit = Edit::delete(start, placeholder);
            active_snippet.update_positions(&edit);
        }

        // Store active snippet state (with adjusted positions)
        let state = runtime.ext_mut::<SnippetSessionState>();
        state.active = Some(active_snippet);

        // If there are tab stops, enter snippet navigation mode
        if has_tab_stops {
            runtime.push_mode(ids::NAVIGATING_MODE, TransitionContext::new());
            if let Some((start, end, _)) = first_stop_range {
                // Delete placeholder text from buffer
                if start != end {
                    runtime.delete_range(buffer_id, start, end);
                }
                // Position cursor at tab stop
                if let Some(w) = runtime.windows_mut().active_mut() {
                    w.cursor.line = start.line;
                    w.cursor.column = start.column;
                }
            }
            runtime.record_cursor_move(buffer_id);
        }

        CommandResult::Success
    }
}

/// Extract the word immediately before the cursor column.
///
/// A "word" consists of consecutive `[a-zA-Z0-9_-]` characters.
fn extract_word_before(line: &str, col: usize) -> String {
    let bytes = line.as_bytes();
    let end = col.min(bytes.len());
    let mut start = end;

    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }

    line[start..end].to_string()
}

/// Check if a byte is a word character for snippet prefix matching.
const fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // extract_word_before
    // =========================================================================

    #[test]
    fn test_extract_word_at_end() {
        assert_eq!(extract_word_before("hello fn", 8), "fn");
    }

    #[test]
    fn test_extract_word_empty() {
        assert_eq!(extract_word_before("hello ", 6), "");
    }

    #[test]
    fn test_extract_word_at_beginning() {
        assert_eq!(extract_word_before("fn", 2), "fn");
    }

    #[test]
    fn test_extract_word_with_dash() {
        assert_eq!(extract_word_before("my-func", 7), "my-func");
    }

    #[test]
    fn test_extract_word_with_underscore() {
        assert_eq!(extract_word_before("my_func", 7), "my_func");
    }

    #[test]
    fn test_extract_word_middle_of_line() {
        assert_eq!(extract_word_before("abc fn xyz", 6), "fn");
    }

    #[test]
    fn test_extract_word_at_col_zero() {
        assert_eq!(extract_word_before("fn", 0), "");
    }

    #[test]
    fn test_extract_word_col_beyond_line() {
        assert_eq!(extract_word_before("fn", 100), "fn");
    }

    // =========================================================================
    // is_word_char
    // =========================================================================

    #[test]
    fn test_is_word_char() {
        assert!(is_word_char(b'a'));
        assert!(is_word_char(b'Z'));
        assert!(is_word_char(b'0'));
        assert!(is_word_char(b'_'));
        assert!(is_word_char(b'-'));
        assert!(!is_word_char(b' '));
        assert!(!is_word_char(b'('));
        assert!(!is_word_char(b'.'));
    }

    // =========================================================================
    // Command trait
    // =========================================================================

    #[test]
    fn test_command_id() {
        let cmd = ExpandSnippet::new(Arc::new(SnippetRegistry::new()));
        assert_eq!(cmd.id(), ids::EXPAND);
    }

    #[test]
    fn test_command_description() {
        let cmd = ExpandSnippet::new(Arc::new(SnippetRegistry::new()));
        assert!(!cmd.description().is_empty());
    }
}
