//! Unicode-aware text utilities for chrome rendering.
//!
//! Provides functions for text width calculation and truncation that
//! correctly handle CJK characters and zero-width marks. Used by
//! native `ClientModule` chrome implementations.

use unicode_width::UnicodeWidthStr;

/// Calculate the display width of a string.
///
/// Handles CJK characters (width = 2), zero-width characters, and
/// control characters.
#[must_use]
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate text with ellipsis at the end.
///
/// If the text is longer than `max_width`, it's truncated and "..." is appended.
/// The total width will not exceed `max_width`.
#[must_use]
pub fn truncate_end(text: &str, max_width: usize) -> String {
    let width = display_width(text);
    if width <= max_width {
        return text.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let target_width = max_width - 3; // Space for "..."
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    result.push_str("...");
    result
}

/// Truncate text with ellipsis at the start.
///
/// Useful for file paths where the end is more important.
#[must_use]
pub fn truncate_start(text: &str, max_width: usize) -> String {
    let width = display_width(text);
    if width <= max_width {
        return text.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let target_width = max_width - 3;
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars().rev() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        result.insert(0, ch);
        current_width += ch_width;
    }

    let mut final_result = String::from("...");
    final_result.push_str(&result);
    final_result
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
