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
#[path = "log_render_tests.rs"]
mod tests;
