//! CLI panel rendering.
//!
//! Renders the CLI panel with command history, results, and input line.

use crate::cli_panel::{CliPanelState, CliResult};

/// Render the CLI panel header.
#[must_use]
pub fn render_header(width: u16) -> String {
    let header = "-- CLI (<C-b>; to close) ";
    let padding = (width as usize).saturating_sub(header.len());
    let dashes = "-".repeat(padding);
    format!("\x1b[7m{header}{dashes}\x1b[0m")
}

/// Render the CLI panel content.
///
/// Returns a vector of lines to display.
///
/// # Arguments
///
/// * `state` - Panel state including history and input
/// * `width` - Terminal width
/// * `height` - Panel height in rows
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn render_panel(state: &CliPanelState, width: u16, height: u16) -> Vec<String> {
    let mut lines = Vec::with_capacity(height as usize);

    // Header
    lines.push(render_header(width));

    let content_height = (height as usize).saturating_sub(2); // -1 header, -1 input

    // Build history lines
    let mut history_lines: Vec<String> = Vec::new();
    for entry in &state.history {
        // Command line (green prompt)
        history_lines.push(format!("\x1b[32m>\x1b[0m {}", entry.command));
        // Result line(s)
        match &entry.result {
            CliResult::Ok(output) => {
                if output == "__CLEAR__" {
                    // Special clear marker - don't show
                    continue;
                }
                for line in output.lines() {
                    if !line.is_empty() {
                        history_lines.push(format!("  {line}"));
                    }
                }
            }
            CliResult::Err(msg) => {
                history_lines.push(format!("\x1b[31m  Error: {msg}\x1b[0m"));
            }
            CliResult::Pending(msg) => {
                // Yellow for pending state
                history_lines.push(format!("\x1b[33m  {msg}\x1b[0m"));
            }
        }
    }

    // Apply scroll and take visible lines
    // scroll_offset=0 means showing latest (bottom)
    let total_lines = history_lines.len();
    let start = total_lines.saturating_sub(content_height + state.scroll_offset);
    let end = total_lines.saturating_sub(state.scroll_offset);

    for line in history_lines
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
    {
        lines.push(truncate_line(line, width));
    }

    // Pad to fill height
    while lines.len() < height as usize - 1 {
        lines.push(String::new());
    }

    // Input line with cursor
    let input_line = render_input_line(state, width);
    lines.push(input_line);

    lines
}

/// Render the input line with cursor.
///
/// Uses character-based indexing for proper Unicode support.
fn render_input_line(state: &CliPanelState, width: u16) -> String {
    let prompt = "> ";
    let max_input_width = (width as usize).saturating_sub(prompt.len() + 1);

    let char_count = state.input.chars().count();

    // Simple cursor rendering: show underscore at cursor position
    if state.cursor_pos >= char_count {
        // Cursor at end
        let input: String = if char_count > max_input_width {
            // Truncate from start to show cursor
            state
                .input
                .chars()
                .skip(char_count.saturating_sub(max_input_width))
                .collect()
        } else {
            state.input.clone()
        };
        format!("{prompt}{input}\x1b[7m \x1b[0m")
    } else {
        // Cursor in middle - highlight character at cursor
        let before: String = state.input.chars().take(state.cursor_pos).collect();
        let cursor_char = state.input.chars().nth(state.cursor_pos).unwrap_or(' ');
        let after: String = state.input.chars().skip(state.cursor_pos + 1).collect();

        // Truncate if needed (count visible characters, not bytes)
        let visible_len = before.chars().count() + 1 + after.chars().count();
        if visible_len > max_input_width {
            let display = format!("{before}\x1b[7m{cursor_char}\x1b[0m{after}");
            // Simple truncation for now - could be improved
            format!("{prompt}...{display}")
        } else {
            format!("{prompt}{before}\x1b[7m{cursor_char}\x1b[0m{after}")
        }
    }
}

/// Truncate line to fit width, accounting for ANSI codes.
fn truncate_line(line: &str, width: u16) -> String {
    // Simple approach: count visible characters
    let mut visible_len = 0;
    let mut result = String::new();
    let mut in_escape = false;

    for c in line.chars() {
        if c == '\x1b' {
            in_escape = true;
            result.push(c);
        } else if in_escape {
            result.push(c);
            if c == 'm' {
                in_escape = false;
            }
        } else {
            if visible_len >= width as usize {
                break;
            }
            result.push(c);
            visible_len += 1;
        }
    }

    // Close any open ANSI sequences
    if result.contains("\x1b[") && !result.ends_with("\x1b[0m") {
        result.push_str("\x1b[0m");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_header() {
        let header = render_header(40);
        assert!(header.contains("CLI"));
        assert!(header.contains("\x1b[7m")); // Inverse video
        assert!(header.contains("\x1b[0m")); // Reset
    }

    #[test]
    fn test_render_panel_empty() {
        let state = CliPanelState::new();
        let lines = render_panel(&state, 80, 10);
        assert_eq!(lines.len(), 10);
        // First line is header
        assert!(lines[0].contains("CLI"));
        // Last line is input
        assert!(lines[9].starts_with("> "));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_render_panel_with_history() {
        let mut state = CliPanelState::new();
        state.add_result("keys hello".to_string(), CliResult::Ok("Sent".to_string()));

        let lines = render_panel(&state, 80, 10);
        // Should contain the command and result
        let combined = lines.join("\n");
        assert!(combined.contains("keys hello"));
        assert!(combined.contains("Sent"));
    }

    #[test]
    fn test_truncate_line() {
        let line = "hello world";
        let truncated = truncate_line(line, 5);
        assert_eq!(truncated, "hello");

        // With ANSI codes
        let ansi_line = "\x1b[32mhello\x1b[0m world";
        let truncated = truncate_line(ansi_line, 5);
        assert!(truncated.contains("hello"));
    }

    #[test]
    fn test_input_line_cursor_at_end() {
        let mut state = CliPanelState::new();
        state.input = "test".to_string();
        state.cursor_pos = 4;

        let line = render_input_line(&state, 80);
        assert!(line.starts_with("> test"));
        assert!(line.contains("\x1b[7m")); // Cursor highlight
    }

    #[test]
    fn test_input_line_cursor_in_middle() {
        let mut state = CliPanelState::new();
        state.input = "hello".to_string();
        state.cursor_pos = 2;

        let line = render_input_line(&state, 80);
        assert!(line.contains("he\x1b[7ml\x1b[0mlo")); // 'l' highlighted
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_render_panel_with_pending() {
        let mut state = CliPanelState::new();
        state.add_result("mode".to_string(), CliResult::Pending("Querying...".to_string()));

        let lines = render_panel(&state, 80, 10);
        let combined = lines.join("\n");

        // Should contain the command
        assert!(combined.contains("mode"));
        // Should contain yellow ANSI code for pending
        assert!(combined.contains("\x1b[33m"));
        // Should contain the pending message
        assert!(combined.contains("Querying..."));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_render_panel_with_error() {
        let mut state = CliPanelState::new();
        state.add_result("bad-cmd".to_string(), CliResult::Err("Unknown command".to_string()));

        let lines = render_panel(&state, 80, 10);
        let combined = lines.join("\n");

        // Should contain red ANSI code for error
        assert!(combined.contains("\x1b[31m"));
        assert!(combined.contains("Error: Unknown command"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_render_panel_with_clear_marker() {
        let mut state = CliPanelState::new();
        state.add_result("clear".to_string(), CliResult::Ok("__CLEAR__".to_string()));

        let lines = render_panel(&state, 80, 10);
        let combined = lines.join("\n");

        // __CLEAR__ marker should be skipped
        assert!(!combined.contains("__CLEAR__"));
        // But the command should still be there
        assert!(combined.contains("clear"));
    }

    #[test]
    fn test_input_line_long_input_truncation_at_end() {
        let mut state = CliPanelState::new();
        // Create input longer than available width
        state.input = "a".repeat(200);
        state.cursor_pos = 200; // Cursor at end

        let line = render_input_line(&state, 40);
        // Should still render without panic
        assert!(line.contains("> "));
    }

    #[test]
    fn test_input_line_long_input_cursor_in_middle_truncation() {
        let mut state = CliPanelState::new();
        // Create long input with cursor in middle
        state.input = "a".repeat(200);
        state.cursor_pos = 100;

        let line = render_input_line(&state, 40);
        // When visible_len > max_input_width, should use "..." prefix
        assert!(line.contains("..."));
    }

    #[test]
    fn test_truncate_line_with_unclosed_ansi() {
        // Line with ANSI code that doesn't end with reset
        let line = "\x1b[32mhello world";
        let truncated = truncate_line(line, 20);
        // Should auto-close the ANSI escape
        assert!(truncated.ends_with("\x1b[0m"));
    }
}
