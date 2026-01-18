//! CLI panel rendering.
//!
//! Renders the CLI panel with command history, results, and input line.

use super::cli_panel::{CliPanelState, CliResult};

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
fn render_input_line(state: &CliPanelState, width: u16) -> String {
    let prompt = "> ";
    let max_input_width = (width as usize).saturating_sub(prompt.len() + 1);

    // Simple cursor rendering: show underscore at cursor position
    if state.cursor_pos >= state.input.len() {
        // Cursor at end
        let input = if state.input.len() > max_input_width {
            // Truncate from start to show cursor
            let start = state.input.len().saturating_sub(max_input_width);
            &state.input[start..]
        } else {
            &state.input
        };
        format!("{prompt}{input}\x1b[7m \x1b[0m")
    } else {
        // Cursor in middle - highlight character at cursor
        let (before, rest) = state.input.split_at(state.cursor_pos);
        let cursor_char = rest.chars().next().unwrap_or(' ');
        let after = &rest[cursor_char.len_utf8()..];

        // Truncate if needed
        let display = format!("{before}\x1b[7m{cursor_char}\x1b[0m{after}");
        if display.len() > max_input_width + 10 {
            // Account for ANSI codes
            format!("{prompt}...{}", &display[display.len().saturating_sub(max_input_width)..])
        } else {
            format!("{prompt}{display}")
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
}
