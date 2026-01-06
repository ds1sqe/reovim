//! Text utilities for UI rendering.
//!
//! Provides Unicode-aware text manipulation functions for truncation,
//! alignment, and padding.

use {std::borrow::Cow, unicode_width::UnicodeWidthStr};

/// Text alignment options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    /// Align text to the left (default).
    #[default]
    Left,
    /// Center text.
    Center,
    /// Align text to the right.
    Right,
}

/// Returns the display width of a string (Unicode-aware).
///
/// Handles CJK characters (width 2) and zero-width characters correctly.
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::display_width;
///
/// assert_eq!(display_width("hello"), 5);
/// assert_eq!(display_width(""), 0);
/// ```
#[inline]
#[must_use]
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncates text at the end with an ellipsis if it exceeds `max_width`.
///
/// Returns `"Hello..."` style truncation. If the text fits within `max_width`,
/// returns the original string unchanged.
///
/// # Arguments
///
/// * `text` - The text to truncate
/// * `max_width` - Maximum display width (must be >= 3 for ellipsis)
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::truncate_end;
///
/// assert_eq!(truncate_end("Hello, World!", 8), "Hello...");
/// assert_eq!(truncate_end("Hi", 10), "Hi");
/// ```
#[must_use]
pub fn truncate_end(text: &str, max_width: usize) -> Cow<'_, str> {
    let width = display_width(text);
    if width <= max_width {
        return Cow::Borrowed(text);
    }

    // Need at least 3 chars for "..."
    if max_width < 3 {
        return Cow::Borrowed(&text[..max_width.min(text.len())]);
    }

    let target_width = max_width - 3; // Reserve space for "..."
    let mut result = String::with_capacity(max_width);
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
    Cow::Owned(result)
}

/// Truncates text at the start with an ellipsis if it exceeds `max_width`.
///
/// Returns `"...file.rs"` style truncation. Useful for file paths where
/// the end is more important than the beginning.
///
/// # Arguments
///
/// * `text` - The text to truncate
/// * `max_width` - Maximum display width (must be >= 3 for ellipsis)
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::truncate_start;
///
/// assert_eq!(truncate_start("/very/long/path/file.rs", 11), ".../file.rs");
/// assert_eq!(truncate_start("short", 10), "short");
/// ```
#[must_use]
pub fn truncate_start(text: &str, max_width: usize) -> Cow<'_, str> {
    let width = display_width(text);
    if width <= max_width {
        return Cow::Borrowed(text);
    }

    // Need at least 3 chars for "..."
    if max_width < 3 {
        // Take last max_width chars by byte (imperfect but safe)
        let start = text.len().saturating_sub(max_width);
        return Cow::Borrowed(&text[start..]);
    }

    let target_width = max_width - 3; // Reserve space for "..."

    // Collect chars with their widths from the end
    let chars: Vec<(char, usize)> = text
        .chars()
        .map(|ch| (ch, unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)))
        .collect();

    let mut current_width = 0;
    let mut start_idx = chars.len();

    for (i, (_, ch_width)) in chars.iter().enumerate().rev() {
        if current_width + ch_width > target_width {
            break;
        }
        current_width += ch_width;
        start_idx = i;
    }

    let mut result = String::with_capacity(max_width);
    result.push_str("...");
    for (ch, _) in &chars[start_idx..] {
        result.push(*ch);
    }

    Cow::Owned(result)
}

/// Aligns text within a fixed width, padding with spaces.
///
/// If the text is wider than `width`, it is returned unchanged (not truncated).
///
/// # Arguments
///
/// * `text` - The text to align
/// * `width` - The target width
/// * `alignment` - How to align the text
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::{align, Alignment};
///
/// assert_eq!(align("Hi", 6, Alignment::Left), "Hi    ");
/// assert_eq!(align("Hi", 6, Alignment::Right), "    Hi");
/// assert_eq!(align("Hi", 6, Alignment::Center), "  Hi  ");
/// ```
#[must_use]
pub fn align(text: &str, width: usize, alignment: Alignment) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let padding = width - text_width;

    match alignment {
        Alignment::Left => pad_right(text, width, ' '),
        Alignment::Right => pad_left(text, width, ' '),
        Alignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            let mut result = String::with_capacity(width);
            for _ in 0..left_pad {
                result.push(' ');
            }
            result.push_str(text);
            for _ in 0..right_pad {
                result.push(' ');
            }
            result
        }
    }
}

/// Pads text on the right with a fill character to reach `width`.
///
/// If the text is already wider than or equal to `width`, returns it unchanged.
///
/// # Arguments
///
/// * `text` - The text to pad
/// * `width` - The target width
/// * `fill` - The character to use for padding
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::pad_right;
///
/// assert_eq!(pad_right("Hi", 6, '.'), "Hi....");
/// assert_eq!(pad_right("Hello", 3, '.'), "Hello");
/// ```
#[must_use]
pub fn pad_right(text: &str, width: usize, fill: char) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let fill_width = unicode_width::UnicodeWidthChar::width(fill).unwrap_or(1);
    let padding_needed = width - text_width;
    let fill_count = padding_needed / fill_width;

    let mut result = String::with_capacity(text.len() + fill_count);
    result.push_str(text);
    for _ in 0..fill_count {
        result.push(fill);
    }
    result
}

/// Pads text on the left with a fill character to reach `width`.
///
/// If the text is already wider than or equal to `width`, returns it unchanged.
///
/// # Arguments
///
/// * `text` - The text to pad
/// * `width` - The target width
/// * `fill` - The character to use for padding
///
/// # Examples
///
/// ```
/// use reovim_core::ui::text::pad_left;
///
/// assert_eq!(pad_left("Hi", 6, '.'), "....Hi");
/// assert_eq!(pad_left("Hello", 3, '.'), "Hello");
/// ```
#[must_use]
pub fn pad_left(text: &str, width: usize, fill: char) -> String {
    let text_width = display_width(text);
    if text_width >= width {
        return text.to_string();
    }

    let fill_width = unicode_width::UnicodeWidthChar::width(fill).unwrap_or(1);
    let padding_needed = width - text_width;
    let fill_count = padding_needed / fill_width;

    let mut result = String::with_capacity(text.len() + fill_count);
    for _ in 0..fill_count {
        result.push(fill);
    }
    result.push_str(text);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width(" "), 1);
    }

    #[test]
    fn test_truncate_end_no_truncation() {
        assert_eq!(truncate_end("hello", 10), "hello");
        assert_eq!(truncate_end("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_end_truncation() {
        assert_eq!(truncate_end("hello world", 8), "hello...");
        assert_eq!(truncate_end("hello", 3), "...");
    }

    #[test]
    fn test_truncate_end_small_width() {
        assert_eq!(truncate_end("hello", 2), "he");
        assert_eq!(truncate_end("hello", 1), "h");
        assert_eq!(truncate_end("hello", 0), "");
    }

    #[test]
    fn test_truncate_start_no_truncation() {
        assert_eq!(truncate_start("hello", 10), "hello");
        assert_eq!(truncate_start("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_start_truncation() {
        assert_eq!(truncate_start("hello world", 8), "...world");
        assert_eq!(truncate_start("/path/to/file.rs", 11), ".../file.rs");
    }

    #[test]
    fn test_align_left() {
        assert_eq!(align("Hi", 6, Alignment::Left), "Hi    ");
        assert_eq!(align("Hello", 5, Alignment::Left), "Hello");
        assert_eq!(align("Hello", 3, Alignment::Left), "Hello");
    }

    #[test]
    fn test_align_right() {
        assert_eq!(align("Hi", 6, Alignment::Right), "    Hi");
        assert_eq!(align("Hello", 5, Alignment::Right), "Hello");
    }

    #[test]
    fn test_align_center() {
        assert_eq!(align("Hi", 6, Alignment::Center), "  Hi  ");
        assert_eq!(align("Hi", 7, Alignment::Center), "  Hi   ");
        assert_eq!(align("Hello", 5, Alignment::Center), "Hello");
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(pad_right("Hi", 6, '.'), "Hi....");
        assert_eq!(pad_right("Hello", 5, '.'), "Hello");
        assert_eq!(pad_right("Hello", 3, '.'), "Hello");
    }

    #[test]
    fn test_pad_left() {
        assert_eq!(pad_left("Hi", 6, '.'), "....Hi");
        assert_eq!(pad_left("Hello", 5, '.'), "Hello");
        assert_eq!(pad_left("Hello", 3, '.'), "Hello");
    }

    #[test]
    fn test_alignment_default() {
        assert_eq!(Alignment::default(), Alignment::Left);
    }
}
