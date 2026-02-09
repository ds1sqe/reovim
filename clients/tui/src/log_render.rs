//! Log panel rendering utilities.
//!
//! Provides functions to format and render log entries for the TUI panel.

use reovim_protocol::v1::LogLevel;

use crate::{
    log_buffer::{LevelColor, TuiLogEntry},
    log_panel::LogPanelState,
};

/// Maximum message length before truncation.
const MAX_MESSAGE_LENGTH: usize = 256;

/// Format a single log entry for display.
///
/// Format: `[HH:MM:SS] [LEVEL] [S/C] message`
#[must_use]
pub fn format_entry(entry: &TuiLogEntry, width: u16) -> String {
    let color = entry.level_color();
    let reset = LevelColor::reset_code();

    // Format level (5 chars, right-aligned)
    let level_str = match entry.level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => " WARN",
        LogLevel::Info => " INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    };

    // Source prefix
    let source = entry.source_prefix();

    // Calculate available width for message
    // Format: [HH:MM:SS] [LEVEL] [S] message
    // Width:  10 + 1 + 5 + 1 + 3 + 1 = 21 chars fixed overhead
    let overhead = 21_usize;
    let available = (width as usize).saturating_sub(overhead);

    // Truncate and sanitize message
    let message = sanitize_message(&entry.message, available.min(MAX_MESSAGE_LENGTH));

    format!(
        "{}[{}] {} {} {}{}",
        color.ansi_code(),
        entry.timestamp,
        level_str,
        source,
        message,
        reset
    )
}

/// Sanitize a message for display.
///
/// - Replaces newlines with spaces
/// - Escapes embedded ANSI codes
/// - Truncates to max length with ellipsis
#[must_use]
fn sanitize_message(message: &str, max_length: usize) -> String {
    if max_length == 0 {
        return String::new();
    }

    // Replace newlines and tabs with spaces
    let mut result: String = message
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            // Escape escape character to prevent ANSI injection
            '\x1b' => '^',
            c => c,
        })
        .collect();

    // Truncate if needed
    if result.len() > max_length {
        result.truncate(max_length.saturating_sub(3));
        result.push_str("...");
    }

    result
}

/// Generate the log panel header line.
#[must_use]
pub fn render_header(width: u16, level_filter: Option<LogLevel>) -> String {
    let filter_str = match level_filter {
        Some(LogLevel::Error) => " [ERROR+]",
        Some(LogLevel::Warn) => " [WARN+]",
        Some(LogLevel::Info) => " [INFO+]",
        Some(LogLevel::Debug) => " [DEBUG+]",
        _ => "",
    };

    let header = format!("-- Messages{filter_str} (q to close) ");
    let padding = (width as usize).saturating_sub(header.len());
    let dashes = "-".repeat(padding);

    format!("\x1b[7m{header}{dashes}\x1b[0m") // Reverse video for header
}

/// Render the complete log panel.
///
/// Returns a vector of lines to display.
#[must_use]
pub fn render_panel(
    entries: &[&TuiLogEntry],
    state: &LogPanelState,
    width: u16,
    height: u16,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(height as usize);

    // Header
    lines.push(render_header(width, state.level_filter));

    // Filter entries by level
    let filtered: Vec<_> = entries
        .iter()
        .filter(|e| state.passes_filter(e.level))
        .collect();

    // Calculate visible range
    let visible_height = (height as usize).saturating_sub(1); // -1 for header
    let total = filtered.len();

    // Apply scroll offset (scroll_offset = 0 means showing newest)
    let start = if total <= visible_height {
        0
    } else {
        total
            .saturating_sub(visible_height)
            .saturating_sub(state.scroll_offset)
    };
    let end = (start + visible_height).min(total);

    // Add entries
    for entry in filtered.iter().skip(start).take(end - start) {
        lines.push(format_entry(entry, width));
    }

    // Pad with empty lines if needed
    while lines.len() < height as usize {
        lines.push(String::new());
    }

    lines
}

/// Get the ANSI color code for a log level.
#[must_use]
pub const fn level_color_code(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "\x1b[31m",                   // Red
        LogLevel::Warn => "\x1b[33m",                    // Yellow
        LogLevel::Info => "\x1b[0m",                     // Default
        LogLevel::Debug | LogLevel::Trace => "\x1b[90m", // Dim
    }
}

#[cfg(test)]
mod tests {
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
}
