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
