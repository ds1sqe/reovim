//! Unicode-aware text utilities for rendering.
//!
//! Provides functions for text width calculation, truncation, alignment,
//! and padding that correctly handle CJK characters and zero-width marks.

use unicode_width::UnicodeWidthStr;

/// Text alignment options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Alignment {
    /// Align text to the left (default)
    #[default]
    Left,
    /// Center text
    Center,
    /// Align text to the right
    Right,
}

/// Calculate the display width of a string.
///
/// Handles:
/// - CJK characters (width = 2)
/// - Zero-width characters (width = 0)
/// - Control characters
///
/// # Example
///
/// ```ignore
/// assert_eq!(display_width("Hello"), 5);
/// assert_eq!(display_width("你好"), 4);  // CJK: 2 chars × 2 width
/// ```
#[must_use]
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate text with ellipsis at the end.
///
/// If the text is longer than `max_width`, it's truncated and "..." is appended.
/// The total width will not exceed `max_width`.
///
/// # Example
///
/// ```ignore
/// assert_eq!(truncate_end("Hello, World!", 8), "Hello...");
/// assert_eq!(truncate_end("Hi", 10), "Hi");
/// ```
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
/// If the text is longer than `max_width`, the beginning is replaced with "...".
///
/// # Example
///
/// ```ignore
/// assert_eq!(truncate_start("/very/long/path/file.rs", 15), ".../path/file.rs");
/// ```
#[must_use]
pub fn truncate_start(text: &str, max_width: usize) -> String {
    let width = display_width(text);
    if width <= max_width {
        return text.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let target_width = max_width - 3; // Space for "..."

    // Work backwards from the end
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

/// Align text within a given width.
///
/// Pads the text with spaces according to the alignment.
/// If the text is wider than `width`, it's returned unchanged.
///
/// # Example
///
/// ```ignore
/// assert_eq!(align("Hi", 6, Alignment::Left), "Hi    ");
/// assert_eq!(align("Hi", 6, Alignment::Center), "  Hi  ");
/// assert_eq!(align("Hi", 6, Alignment::Right), "    Hi");
/// ```
#[must_use]
pub fn align(text: &str, width: usize, alignment: Alignment) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let padding = width - text_width;

    match alignment {
        Alignment::Left => {
            let mut result = text.to_string();
            result.push_str(&" ".repeat(padding));
            result
        }
        Alignment::Right => {
            let mut result = " ".repeat(padding);
            result.push_str(text);
            result
        }
        Alignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            let mut result = " ".repeat(left_pad);
            result.push_str(text);
            result.push_str(&" ".repeat(right_pad));
            result
        }
    }
}

/// Pad text on the left to reach target width.
///
/// If the text is already wider than `width`, it's returned unchanged.
///
/// # Example
///
/// ```ignore
/// assert_eq!(pad_left("42", 5, '0'), "00042");
/// assert_eq!(pad_left("42", 5, ' '), "   42");
/// ```
#[must_use]
pub fn pad_left(text: &str, width: usize, fill: char) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let padding = width - text_width;
    let fill_width = unicode_width::UnicodeWidthChar::width(fill).unwrap_or(1);

    // Calculate how many fill chars we need
    let fill_count = padding.div_ceil(fill_width);

    let mut result = String::new();
    for _ in 0..fill_count {
        result.push(fill);
    }
    result.push_str(text);

    // Trim to exact width if fill char is wide
    if display_width(&result) > width {
        // Remove excess from the beginning
        let mut trimmed = String::new();
        let mut current_width = 0;
        let chars: Vec<char> = result.chars().collect();
        for &ch in chars.iter().rev() {
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + ch_width > width {
                break;
            }
            trimmed.insert(0, ch);
            current_width += ch_width;
        }
        return trimmed;
    }

    result
}

/// Pad text on the right to reach target width.
///
/// If the text is already wider than `width`, it's returned unchanged.
///
/// # Example
///
/// ```ignore
/// assert_eq!(pad_right("Hi", 5, '.'), "Hi...");
/// assert_eq!(pad_right("Hi", 5, ' '), "Hi   ");
/// ```
#[must_use]
pub fn pad_right(text: &str, width: usize, fill: char) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let padding = width - text_width;
    let fill_width = unicode_width::UnicodeWidthChar::width(fill).unwrap_or(1);

    let fill_count = padding.div_ceil(fill_width);

    let mut result = text.to_string();
    for _ in 0..fill_count {
        result.push(fill);
    }

    // Trim to exact width if fill char is wide
    if display_width(&result) > width {
        let mut trimmed = String::new();
        let mut current_width = 0;
        for ch in result.chars() {
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + ch_width > width {
                break;
            }
            trimmed.push(ch);
            current_width += ch_width;
        }
        return trimmed;
    }

    result
}

/// Split text into lines respecting max width (word wrap).
///
/// Wraps at word boundaries when possible, or breaks mid-word if necessary.
///
/// # Example
///
/// ```ignore
/// let lines = wrap_text("Hello World, how are you?", 10);
/// assert_eq!(lines, vec!["Hello", "World, how", "are you?"]);
/// ```
#[must_use]
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = display_width(word);

        if current_width == 0 {
            // First word on line
            if word_width <= max_width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is too long, break it
                for ch in word.chars() {
                    let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if current_width + ch_width > max_width {
                        lines.push(current_line);
                        current_line = String::new();
                        current_width = 0;
                    }
                    current_line.push(ch);
                    current_width += ch_width;
                }
            }
        } else if current_width + 1 + word_width <= max_width {
            // Word fits with space
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line);
            if word_width <= max_width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is too long, break it
                current_line = String::new();
                current_width = 0;
                for ch in word.chars() {
                    let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if current_width + ch_width > max_width {
                        lines.push(current_line);
                        current_line = String::new();
                        current_width = 0;
                    }
                    current_line.push(ch);
                    current_width += ch_width;
                }
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("Hello"), 5);
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("  "), 2);
    }

    #[test]
    fn test_display_width_cjk() {
        // CJK characters are typically width 2
        assert_eq!(display_width("你好"), 4);
        assert_eq!(display_width("日本語"), 6);
    }

    #[test]
    fn test_truncate_end_no_truncation() {
        assert_eq!(truncate_end("Hi", 10), "Hi");
        assert_eq!(truncate_end("Hello", 5), "Hello");
    }

    #[test]
    fn test_truncate_end_truncation() {
        assert_eq!(truncate_end("Hello, World!", 8), "Hello...");
        assert_eq!(truncate_end("Hello, World!", 5), "He...");
    }

    #[test]
    fn test_truncate_end_very_short() {
        assert_eq!(truncate_end("Hello", 3), "...");
        assert_eq!(truncate_end("Hello", 2), "..");
        assert_eq!(truncate_end("Hello", 1), ".");
    }

    #[test]
    fn test_truncate_start_no_truncation() {
        assert_eq!(truncate_start("Hi", 10), "Hi");
    }

    #[test]
    fn test_truncate_start_truncation() {
        let result = truncate_start("/very/long/path/file.rs", 12);
        assert!(result.starts_with("..."));
        assert_eq!(display_width(&result), 12);
    }

    #[test]
    fn test_align_left() {
        assert_eq!(align("Hi", 6, Alignment::Left), "Hi    ");
    }

    #[test]
    fn test_align_right() {
        assert_eq!(align("Hi", 6, Alignment::Right), "    Hi");
    }

    #[test]
    fn test_align_center() {
        assert_eq!(align("Hi", 6, Alignment::Center), "  Hi  ");
        assert_eq!(align("Hi", 7, Alignment::Center), "  Hi   "); // Odd padding
    }

    #[test]
    fn test_align_no_change() {
        assert_eq!(align("Hello!", 6, Alignment::Left), "Hello!");
        assert_eq!(align("Hello!", 4, Alignment::Left), "Hello!");
    }

    #[test]
    fn test_pad_left() {
        assert_eq!(pad_left("42", 5, '0'), "00042");
        assert_eq!(pad_left("42", 5, ' '), "   42");
        assert_eq!(pad_left("Hello", 3, ' '), "Hello"); // Already wider
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(pad_right("Hi", 5, '.'), "Hi...");
        assert_eq!(pad_right("Hi", 5, ' '), "Hi   ");
        assert_eq!(pad_right("Hello", 3, ' '), "Hello"); // Already wider
    }

    #[test]
    fn test_wrap_text_simple() {
        let lines = wrap_text("Hello World", 20);
        assert_eq!(lines, vec!["Hello World"]);
    }

    #[test]
    fn test_wrap_text_multiple_lines() {
        let lines = wrap_text("Hello World", 6);
        assert_eq!(lines, vec!["Hello", "World"]);
    }

    #[test]
    fn test_wrap_text_long_word() {
        let lines = wrap_text("Supercalifragilisticexpialidocious", 10);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(display_width(line) <= 10);
        }
    }

    #[test]
    fn test_wrap_text_empty() {
        let lines = wrap_text("", 10);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_wrap_text_zero_width() {
        let lines = wrap_text("Hello", 0);
        assert!(lines.is_empty());
    }
}
