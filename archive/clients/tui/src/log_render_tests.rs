use reovim_protocol::v1::LogSource;

use super::*;

fn test_entry(level: LogLevel, message: &str) -> TuiLogEntry {
    TuiLogEntry::new(
        "12:34:56".to_string(),
        level,
        "test".to_string(),
        message.to_string(),
        LogSource::Server,
    )
}

#[test]
fn test_color_code_for_each_level() {
    assert_eq!(level_color_code(LogLevel::Error), "\x1b[31m");
    assert_eq!(level_color_code(LogLevel::Warn), "\x1b[33m");
    assert_eq!(level_color_code(LogLevel::Info), "\x1b[0m");
    assert_eq!(level_color_code(LogLevel::Debug), "\x1b[90m");
    assert_eq!(level_color_code(LogLevel::Trace), "\x1b[90m");
}

#[test]
fn test_entry_formatting_with_timestamp() {
    let entry = test_entry(LogLevel::Info, "test message");
    let formatted = format_entry(&entry, 80);
    assert!(formatted.contains("[12:34:56]"));
    assert!(formatted.contains("INFO"));
    assert!(formatted.contains("[S]"));
    assert!(formatted.contains("test message"));
}

#[test]
fn test_scroll_offset_application() {
    let entries: Vec<TuiLogEntry> = (0..20)
        .map(|i| test_entry(LogLevel::Info, &format!("msg{i}")))
        .collect();
    let entry_refs: Vec<_> = entries.iter().collect();

    let mut state = LogPanelState::new();
    state.visible = true;
    state.height = 5; // 1 header + 4 entries

    // No scroll - should show newest (msg16-19)
    let lines = render_panel(&entry_refs, &state, 80, 5);
    assert_eq!(lines.len(), 5);
    assert!(lines[4].contains("msg19")); // Bottom line has newest

    // Scroll up 5 - should show older messages
    state.scroll_up(5);
    let lines = render_panel(&entry_refs, &state, 80, 5);
    assert!(lines[4].contains("msg14")); // Now showing older
}

#[test]
fn test_panel_height_limiting() {
    let entries: Vec<TuiLogEntry> = (0..5)
        .map(|i| test_entry(LogLevel::Info, &format!("msg{i}")))
        .collect();
    let entry_refs: Vec<_> = entries.iter().collect();

    let state = LogPanelState::new();

    // With height 3: 1 header + 2 entries max
    let lines = render_panel(&entry_refs, &state, 80, 3);
    assert_eq!(lines.len(), 3);
}

#[test]
fn test_source_prefix_formatting() {
    let server_entry = TuiLogEntry::new(
        "12:00:00".to_string(),
        LogLevel::Info,
        "test".to_string(),
        "msg".to_string(),
        LogSource::Server,
    );
    let client_entry = TuiLogEntry::new(
        "12:00:00".to_string(),
        LogLevel::Info,
        "test".to_string(),
        "msg".to_string(),
        LogSource::Client,
    );

    let server_fmt = format_entry(&server_entry, 80);
    let client_fmt = format_entry(&client_entry, 80);

    assert!(server_fmt.contains("[S]"));
    assert!(client_fmt.contains("[C]"));
}

#[test]
fn test_long_message_truncation() {
    let long_msg = "a".repeat(500);
    let entry = test_entry(LogLevel::Info, &long_msg);
    let formatted = format_entry(&entry, 80);

    // Should be truncated with ellipsis
    assert!(formatted.len() < 500);
    assert!(formatted.contains("..."));
}

#[test]
fn test_embedded_ansi_codes_escaped() {
    let entry = test_entry(LogLevel::Info, "test \x1b[31mred\x1b[0m text");
    let formatted = format_entry(&entry, 80);

    // Escape characters should be replaced with ^
    assert!(!formatted.contains("\x1b[31m")); // Original codes should not appear twice
    assert!(formatted.contains("^[31m")); // Escaped
}

#[test]
fn test_newlines_in_message() {
    let entry = test_entry(LogLevel::Info, "line1\nline2\nline3");
    let formatted = format_entry(&entry, 80);

    // Newlines should be replaced with spaces
    assert!(!formatted.contains('\n'));
    assert!(formatted.contains("line1 line2 line3"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_empty_buffer_renders_blank() {
    let entries: Vec<&TuiLogEntry> = vec![];
    let state = LogPanelState::new();

    let lines = render_panel(&entries, &state, 80, 5);
    assert_eq!(lines.len(), 5);
    assert!(lines[0].contains("Messages")); // Header still present
    // Rest should be empty
    assert!(lines[1].is_empty() || lines[1].trim().is_empty());
}

#[test]
fn test_format_entry_all_levels() {
    let error_entry = test_entry(LogLevel::Error, "err");
    let warn_entry = test_entry(LogLevel::Warn, "wrn");
    let debug_entry = test_entry(LogLevel::Debug, "dbg");
    let trace_entry = test_entry(LogLevel::Trace, "trc");

    let error_fmt = format_entry(&error_entry, 80);
    let warn_fmt = format_entry(&warn_entry, 80);
    let debug_fmt = format_entry(&debug_entry, 80);
    let trace_fmt = format_entry(&trace_entry, 80);

    assert!(error_fmt.contains("ERROR"));
    assert!(warn_fmt.contains("WARN"));
    assert!(debug_fmt.contains("DEBUG"));
    assert!(trace_fmt.contains("TRACE"));
}

#[test]
fn test_sanitize_zero_width() {
    // sanitize_message with max_length=0 returns empty string
    let entry = test_entry(LogLevel::Info, "hello");
    // Use a very narrow width that forces available=0
    let formatted = format_entry(&entry, 0);
    // Should not panic
    assert!(!formatted.is_empty());
}

#[test]
fn test_render_header_with_all_filters() {
    let error_hdr = render_header(80, Some(LogLevel::Error));
    let warn_hdr = render_header(80, Some(LogLevel::Warn));
    let info_hdr = render_header(80, Some(LogLevel::Info));
    let debug_hdr = render_header(80, Some(LogLevel::Debug));
    let none_hdr = render_header(80, None);

    assert!(error_hdr.contains("[ERROR+]"));
    assert!(warn_hdr.contains("[WARN+]"));
    assert!(info_hdr.contains("[INFO+]"));
    assert!(debug_hdr.contains("[DEBUG+]"));
    assert!(!none_hdr.contains("[ERROR+]"));
    assert!(!none_hdr.contains("[WARN+]"));
}

#[test]
fn test_single_entry_in_large_panel() {
    let entry = test_entry(LogLevel::Info, "single message");
    let entries: Vec<&TuiLogEntry> = vec![&entry];

    let state = LogPanelState::new();

    let lines = render_panel(&entries, &state, 80, 10);
    assert_eq!(lines.len(), 10);
    assert!(lines[0].contains("Messages")); // Header
    assert!(lines[1].contains("single message")); // Entry
    // Rest should be empty padding
}
