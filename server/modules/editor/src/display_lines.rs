//! Display line calculations for wrapped text.
//!
//! This module provides utilities for calculating display line positions
//! when text wraps across multiple terminal lines. Used by `gj`/`gk` commands
//! for display-line-aware cursor movement.
//!
//! # Design Philosophy
//!
//! Display line calculations are a **policy** decision - the kernel provides
//! buffer primitives, but how to interpret "display lines" based on terminal
//! width is up to the editor module.
//!
//! # Tab Handling
//!
//! Tabs are contextual - their display width depends on the current column:
//! - Tab at column 0 with tabstop=4 takes 4 columns (advances to column 4)
//! - Tab at column 2 with tabstop=4 takes 2 columns (advances to column 4)
//! - Formula: `tab_width = tabstop - (column % tabstop)`
//!
//! # CJK / Unicode Width
//!
//! East Asian characters (CJK) typically render as 2 columns ("fullwidth").
//! This module uses the `unicode-width` crate to determine character widths:
//! - ASCII and most Latin characters: 1 column
//! - CJK ideographs, fullwidth forms: 2 columns
//! - Combining marks, zero-width chars: 0 columns
//!
//! # Limitations
//!
//! Current implementation:
//! - Simple division-based wrapping (no word wrap)
//! - Terminal must support Unicode for CJK to display correctly

use unicode_width::UnicodeWidthChar;

/// Calculate number of display lines for a buffer line.
///
/// A display line is a single row on the terminal. Long lines wrap to
/// multiple display lines based on terminal width.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `terminal_width` - Terminal width in columns
///
/// # Returns
///
/// Number of display lines (minimum 1, even for empty lines)
///
/// # Example
///
/// ```ignore
/// // "hello world" on 80-column terminal = 1 display line
/// assert_eq!(display_line_count("hello world", 80), 1);
///
/// // 100 chars on 80-column terminal = 2 display lines
/// assert_eq!(display_line_count(&"x".repeat(100), 80), 2);
/// ```
#[must_use]
pub fn display_line_count(line: &str, terminal_width: usize) -> usize {
    if terminal_width == 0 {
        return 1;
    }

    let char_count = line.chars().count();
    if char_count == 0 {
        return 1;
    }

    char_count.div_ceil(terminal_width)
}

/// Get display line position within a wrapped line.
///
/// Given a column position in a buffer line and terminal width, returns
/// which display line the column falls on and the column within that
/// display line.
///
/// # Arguments
///
/// * `column` - Column position in the buffer line (0-indexed)
/// * `terminal_width` - Terminal width in columns
///
/// # Returns
///
/// Tuple of (`display_line_index`, `column_within_display_line`)
///
/// # Example
///
/// ```ignore
/// // Column 5 on 80-column terminal = display line 0, column 5
/// assert_eq!(display_position(5, 80), (0, 5));
///
/// // Column 85 on 80-column terminal = display line 1, column 5
/// assert_eq!(display_position(85, 80), (1, 5));
/// ```
#[must_use]
pub const fn display_position(column: usize, terminal_width: usize) -> (usize, usize) {
    if terminal_width == 0 {
        return (0, column);
    }

    let display_line = column / terminal_width;
    let display_column = column % terminal_width;
    (display_line, display_column)
}

/// Convert display position back to buffer column.
///
/// Given a display line index and column within that display line,
/// calculates the corresponding buffer column.
///
/// # Arguments
///
/// * `display_line` - Display line index (0-indexed)
/// * `display_column` - Column within the display line (0-indexed)
/// * `terminal_width` - Terminal width in columns
///
/// # Returns
///
/// Buffer column position
///
/// # Example
///
/// ```ignore
/// // Display line 1, column 5 on 80-column terminal = buffer column 85
/// assert_eq!(buffer_column(1, 5, 80), 85);
/// ```
#[must_use]
pub const fn buffer_column(
    display_line: usize,
    display_column: usize,
    terminal_width: usize,
) -> usize {
    display_line * terminal_width + display_column
}

// ============================================================================
// Tab-aware display calculations
// ============================================================================

/// Calculate display width of a line accounting for tabs.
///
/// Each tab character expands to fill until the next tab stop. The width
/// depends on the current display column position.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `tabstop` - Tab stop width (typically 4 or 8)
///
/// # Returns
///
/// Total display width in columns
///
/// # Example
///
/// ```ignore
/// // "a\tb" with tabstop=4: 'a' (1) + tab (3 to reach col 4) + 'b' (1) = 5
/// assert_eq!(display_width_with_tabs("a\tb", 4), 5);
/// ```
#[must_use]
pub fn display_width_with_tabs(line: &str, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1); // Prevent division by zero
    let mut display_col = 0;

    for c in line.chars() {
        if c == '\t' {
            // Advance to next tab stop
            let tab_width = tabstop - (display_col % tabstop);
            display_col += tab_width;
        } else {
            display_col += 1;
        }
    }

    display_col
}

/// Calculate number of display lines for a buffer line, accounting for tabs.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `terminal_width` - Terminal width in columns
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Number of display lines (minimum 1)
#[must_use]
pub fn display_line_count_with_tabs(line: &str, terminal_width: usize, tabstop: usize) -> usize {
    if terminal_width == 0 {
        return 1;
    }

    let width = display_width_with_tabs(line, tabstop);
    if width == 0 {
        return 1;
    }

    width.div_ceil(terminal_width)
}

/// Get display position for a buffer column, accounting for tabs.
///
/// Calculates which display line and column a buffer column falls on,
/// properly handling tab expansion.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `buffer_col` - Column position in the buffer (character index, 0-indexed)
/// * `terminal_width` - Terminal width in columns
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Tuple of (`display_line_index`, `column_within_display_line`)
#[must_use]
pub fn display_position_with_tabs(
    line: &str,
    buffer_col: usize,
    terminal_width: usize,
    tabstop: usize,
) -> (usize, usize) {
    let tabstop = tabstop.max(1);
    if terminal_width == 0 {
        // Fallback: calculate display column without wrapping
        let display_col = display_col_from_buffer_col(line, buffer_col, tabstop);
        return (0, display_col);
    }

    let display_col = display_col_from_buffer_col(line, buffer_col, tabstop);
    let display_line = display_col / terminal_width;
    let col_in_line = display_col % terminal_width;

    (display_line, col_in_line)
}

/// Convert buffer column to display column, accounting for tabs.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `buffer_col` - Column position in buffer (character index)
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Display column position
#[must_use]
pub fn display_col_from_buffer_col(line: &str, buffer_col: usize, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1);
    let mut display_col = 0;

    for (i, c) in line.chars().enumerate() {
        if i >= buffer_col {
            break;
        }
        if c == '\t' {
            let tab_width = tabstop - (display_col % tabstop);
            display_col += tab_width;
        } else {
            display_col += 1;
        }
    }

    display_col
}

/// Convert display column to buffer column, accounting for tabs.
///
/// Given a target display column, finds the buffer column (character index)
/// that corresponds to that display position.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `target_display_col` - Target display column
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Buffer column (character index)
#[must_use]
pub fn buffer_col_from_display_col(line: &str, target_display_col: usize, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1);
    let mut display_col = 0;
    let mut buffer_col = 0;

    for c in line.chars() {
        if display_col >= target_display_col {
            break;
        }

        let char_width = if c == '\t' {
            tabstop - (display_col % tabstop)
        } else {
            1
        };

        // Check if we'd overshoot
        if display_col + char_width > target_display_col {
            // We're inside a tab or at the target
            break;
        }

        display_col += char_width;
        buffer_col += 1;
    }

    buffer_col
}

// ============================================================================
// Unicode-aware display calculations (tabs + CJK)
// ============================================================================

/// Get the display width of a single character.
///
/// Returns the number of terminal columns the character occupies:
/// - Tab: contextual (use `char_display_width_at` instead)
/// - Most characters: 1 column
/// - CJK / fullwidth: 2 columns
/// - Combining marks, zero-width: 0 columns
#[must_use]
pub fn char_display_width(c: char) -> usize {
    if c == '\t' {
        // Tab width is contextual - caller should use char_display_width_at
        1
    } else {
        c.width().unwrap_or(0)
    }
}

/// Get the display width of a character at a specific column.
///
/// Handles both tabs (contextual width) and Unicode characters (intrinsic width).
#[must_use]
pub fn char_display_width_at(c: char, display_col: usize, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1);
    if c == '\t' {
        tabstop - (display_col % tabstop)
    } else {
        c.width().unwrap_or(0)
    }
}

/// Calculate display width of a line accounting for tabs and Unicode width.
///
/// This is the most comprehensive width function - it handles:
/// - Tab expansion based on column position
/// - CJK double-width characters
/// - Combining marks (width 0)
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `tabstop` - Tab stop width (typically 4 or 8)
///
/// # Returns
///
/// Total display width in columns
#[must_use]
pub fn display_width_unicode(line: &str, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1);
    let mut display_col = 0;

    for c in line.chars() {
        display_col += char_display_width_at(c, display_col, tabstop);
    }

    display_col
}

/// Calculate number of display lines for a buffer line, with full Unicode support.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `terminal_width` - Terminal width in columns
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Number of display lines (minimum 1)
#[must_use]
pub fn display_line_count_unicode(line: &str, terminal_width: usize, tabstop: usize) -> usize {
    if terminal_width == 0 {
        return 1;
    }

    let width = display_width_unicode(line, tabstop);
    if width == 0 {
        return 1;
    }

    width.div_ceil(terminal_width)
}

/// Convert buffer column to display column, with full Unicode support.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `buffer_col` - Column position in buffer (character index)
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Display column position
#[must_use]
pub fn display_col_from_buffer_col_unicode(line: &str, buffer_col: usize, tabstop: usize) -> usize {
    let tabstop = tabstop.max(1);
    let mut display_col = 0;

    for (i, c) in line.chars().enumerate() {
        if i >= buffer_col {
            break;
        }
        display_col += char_display_width_at(c, display_col, tabstop);
    }

    display_col
}

/// Convert display column to buffer column, with full Unicode support.
///
/// Given a target display column, finds the buffer column (character index)
/// that corresponds to that display position. If the target falls inside
/// a wide character (like CJK), returns the buffer column of that character.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `target_display_col` - Target display column
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Buffer column (character index)
#[must_use]
pub fn buffer_col_from_display_col_unicode(
    line: &str,
    target_display_col: usize,
    tabstop: usize,
) -> usize {
    let tabstop = tabstop.max(1);
    let mut display_col = 0;
    let mut buffer_col = 0;

    for c in line.chars() {
        if display_col >= target_display_col {
            break;
        }

        let char_width = char_display_width_at(c, display_col, tabstop);

        // Check if we'd overshoot (landing inside a wide char)
        if display_col + char_width > target_display_col {
            // We're inside a wide character - stay at this buffer column
            break;
        }

        display_col += char_width;
        buffer_col += 1;
    }

    buffer_col
}

/// Get display position for a buffer column, with full Unicode support.
///
/// Calculates which display line and column a buffer column falls on,
/// properly handling tab expansion and CJK characters.
///
/// # Arguments
///
/// * `line` - The buffer line content
/// * `buffer_col` - Column position in the buffer (character index, 0-indexed)
/// * `terminal_width` - Terminal width in columns
/// * `tabstop` - Tab stop width
///
/// # Returns
///
/// Tuple of (`display_line_index`, `column_within_display_line`)
#[must_use]
pub fn display_position_unicode(
    line: &str,
    buffer_col: usize,
    terminal_width: usize,
    tabstop: usize,
) -> (usize, usize) {
    let tabstop = tabstop.max(1);
    if terminal_width == 0 {
        let display_col = display_col_from_buffer_col_unicode(line, buffer_col, tabstop);
        return (0, display_col);
    }

    let display_col = display_col_from_buffer_col_unicode(line, buffer_col, tabstop);
    let display_line = display_col / terminal_width;
    let col_in_line = display_col % terminal_width;

    (display_line, col_in_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_line_count_empty() {
        assert_eq!(display_line_count("", 80), 1);
    }

    #[test]
    fn test_display_line_count_short() {
        assert_eq!(display_line_count("hello", 80), 1);
    }

    #[test]
    fn test_display_line_count_exact() {
        let line = "x".repeat(80);
        assert_eq!(display_line_count(&line, 80), 1);
    }

    #[test]
    fn test_display_line_count_wrap() {
        let line = "x".repeat(81);
        assert_eq!(display_line_count(&line, 80), 2);
    }

    #[test]
    fn test_display_line_count_multiple_wraps() {
        let line = "x".repeat(241);
        assert_eq!(display_line_count(&line, 80), 4);
    }

    #[test]
    fn test_display_line_count_zero_width() {
        assert_eq!(display_line_count("hello", 0), 1);
    }

    #[test]
    fn test_display_position_first_line() {
        assert_eq!(display_position(5, 80), (0, 5));
        assert_eq!(display_position(79, 80), (0, 79));
    }

    #[test]
    fn test_display_position_wrapped() {
        assert_eq!(display_position(80, 80), (1, 0));
        assert_eq!(display_position(85, 80), (1, 5));
        assert_eq!(display_position(160, 80), (2, 0));
    }

    #[test]
    fn test_display_position_zero_width() {
        assert_eq!(display_position(50, 0), (0, 50));
    }

    #[test]
    fn test_buffer_column_first_line() {
        assert_eq!(buffer_column(0, 5, 80), 5);
    }

    #[test]
    fn test_buffer_column_wrapped() {
        assert_eq!(buffer_column(1, 5, 80), 85);
        assert_eq!(buffer_column(2, 0, 80), 160);
    }

    #[test]
    fn test_roundtrip() {
        for col in [0, 5, 79, 80, 85, 160, 165] {
            let (dl, dc) = display_position(col, 80);
            assert_eq!(buffer_column(dl, dc, 80), col);
        }
    }

    // ========================================================================
    // Tab-aware function tests
    // ========================================================================

    #[test]
    fn test_display_width_with_tabs_basic() {
        // "a\tb" with tabstop=4: 'a'(1) + tab(3 to col 4) + 'b'(1) = 5
        assert_eq!(display_width_with_tabs("a\tb", 4), 5);
    }

    #[test]
    fn test_display_width_with_tabs_double_tab() {
        // "\t\t" with tabstop=8: tab(8) + tab(8) = 16
        assert_eq!(display_width_with_tabs("\t\t", 8), 16);
    }

    #[test]
    fn test_display_width_with_tabs_at_start() {
        // Tab at column 0 with tabstop=4 takes full 4 columns
        assert_eq!(display_width_with_tabs("\t", 4), 4);
        assert_eq!(display_width_with_tabs("\tx", 4), 5);
    }

    #[test]
    fn test_display_width_with_tabs_contextual() {
        // Tab at column 2 with tabstop=4 takes 2 columns (to reach 4)
        assert_eq!(display_width_with_tabs("ab\t", 4), 4);
        // Tab at column 3 with tabstop=4 takes 1 column (to reach 4)
        assert_eq!(display_width_with_tabs("abc\t", 4), 4);
    }

    #[test]
    fn test_display_width_with_tabs_only_char() {
        // Single tab is the only character
        assert_eq!(display_width_with_tabs("\t", 4), 4);
        assert_eq!(display_width_with_tabs("\t", 8), 8);
        assert_eq!(display_width_with_tabs("\t", 2), 2);
    }

    #[test]
    fn test_display_width_with_tabs_consecutive() {
        // Multiple consecutive tabs at different positions
        assert_eq!(display_width_with_tabs("\t\t\t", 4), 12); // 4 + 4 + 4
        assert_eq!(display_width_with_tabs("x\t\t", 4), 8); // 1 + 3 + 4
    }

    #[test]
    fn test_display_width_with_tabs_mixed() {
        // Mixed tabs and spaces
        assert_eq!(display_width_with_tabs("  \t", 4), 4); // 2 + 2
        assert_eq!(display_width_with_tabs("\t  ", 4), 6); // 4 + 2
    }

    #[test]
    fn test_display_width_with_tabs_various_tabstops() {
        let line = "x\ty";
        // tabstop=1: each tab advances by 1
        assert_eq!(display_width_with_tabs(line, 1), 3); // 1 + 1 + 1
        // tabstop=2: x(1) + tab(1 to reach col 2) + y(1) = 3
        assert_eq!(display_width_with_tabs(line, 2), 3);
        // tabstop=4: x(1) + tab(3 to reach col 4) + y(1) = 5
        assert_eq!(display_width_with_tabs(line, 4), 5);
        // tabstop=8: x(1) + tab(7 to reach col 8) + y(1) = 9
        assert_eq!(display_width_with_tabs(line, 8), 9);
        // tabstop=16: x(1) + tab(15 to reach col 16) + y(1) = 17
        assert_eq!(display_width_with_tabs(line, 16), 17);
    }

    #[test]
    fn test_display_width_with_tabs_zero_tabstop() {
        // Zero tabstop should be treated as 1 (prevent division by zero)
        assert_eq!(display_width_with_tabs("a\tb", 0), 3);
    }

    #[test]
    fn test_display_width_with_tabs_empty() {
        assert_eq!(display_width_with_tabs("", 4), 0);
    }

    #[test]
    fn test_display_line_count_with_tabs() {
        // Line with tabs that causes wrapping
        let line = "\t\t\t"; // 12 columns with tabstop=4
        assert_eq!(display_line_count_with_tabs(line, 10, 4), 2);
        assert_eq!(display_line_count_with_tabs(line, 12, 4), 1);
        assert_eq!(display_line_count_with_tabs(line, 6, 4), 2);
    }

    #[test]
    fn test_display_col_from_buffer_col() {
        let line = "a\tb";
        // Buffer col 0 (before 'a') = display col 0
        assert_eq!(display_col_from_buffer_col(line, 0, 4), 0);
        // Buffer col 1 (after 'a', before tab) = display col 1
        assert_eq!(display_col_from_buffer_col(line, 1, 4), 1);
        // Buffer col 2 (after tab, before 'b') = display col 4
        assert_eq!(display_col_from_buffer_col(line, 2, 4), 4);
        // Buffer col 3 (after 'b') = display col 5
        assert_eq!(display_col_from_buffer_col(line, 3, 4), 5);
    }

    #[test]
    fn test_buffer_col_from_display_col() {
        let line = "a\tb";
        // Display col 0 = buffer col 0 (at 'a')
        assert_eq!(buffer_col_from_display_col(line, 0, 4), 0);
        // Display col 1 = buffer col 1 (after 'a', at start of tab)
        assert_eq!(buffer_col_from_display_col(line, 1, 4), 1);
        // Display col 2,3 = buffer col 1 (inside tab, can't land here)
        assert_eq!(buffer_col_from_display_col(line, 2, 4), 1);
        assert_eq!(buffer_col_from_display_col(line, 3, 4), 1);
        // Display col 4 = buffer col 2 (at 'b')
        assert_eq!(buffer_col_from_display_col(line, 4, 4), 2);
        // Display col 5 = buffer col 3 (after 'b')
        assert_eq!(buffer_col_from_display_col(line, 5, 4), 3);
    }

    #[test]
    fn test_display_position_with_tabs() {
        let line = "a\tb";
        // Buffer col 2 (at 'b') = display col 4, on display line 0 for 80-col terminal
        assert_eq!(display_position_with_tabs(line, 2, 80, 4), (0, 4));
    }

    #[test]
    fn test_display_position_with_tabs_wrapped() {
        // Create a line that wraps with tabs
        let line = "\t\t\t"; // 12 display columns with tabstop=4
        // Buffer col 0 = display col 0, display line 0
        assert_eq!(display_position_with_tabs(line, 0, 10, 4), (0, 0));
        // Buffer col 1 = display col 4 (after first tab), still line 0
        assert_eq!(display_position_with_tabs(line, 1, 10, 4), (0, 4));
        // Buffer col 2 = display col 8 (after second tab), still line 0
        assert_eq!(display_position_with_tabs(line, 2, 10, 4), (0, 8));
        // Buffer col 3 = display col 12, wraps to line 1, col 2
        assert_eq!(display_position_with_tabs(line, 3, 10, 4), (1, 2));
    }

    #[test]
    fn test_tab_roundtrip() {
        // Test that buffer_col -> display_col -> buffer_col round-trips
        let line = "abc\tdef\tghi";
        for buffer_col in 0..=line.chars().count() {
            let display_col = display_col_from_buffer_col(line, buffer_col, 4);
            let back = buffer_col_from_display_col(line, display_col, 4);
            assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
        }
    }

    #[test]
    fn test_tab_at_end_of_line() {
        let line = "hello\t";
        // "hello" = 5 cols, tab at col 5 with tabstop=4 advances to col 8 (width 3)
        assert_eq!(display_width_with_tabs(line, 4), 8);
    }

    // ========================================================================
    // Unicode-aware function tests (CJK + tabs)
    // ========================================================================

    #[test]
    fn test_char_display_width_ascii() {
        assert_eq!(char_display_width('a'), 1);
        assert_eq!(char_display_width('Z'), 1);
        assert_eq!(char_display_width(' '), 1);
        assert_eq!(char_display_width('!'), 1);
    }

    #[test]
    fn test_char_display_width_cjk() {
        // CJK ideographs are double-width
        assert_eq!(char_display_width('中'), 2);
        assert_eq!(char_display_width('日'), 2);
        assert_eq!(char_display_width('本'), 2);
        assert_eq!(char_display_width('語'), 2);
    }

    #[test]
    fn test_char_display_width_fullwidth() {
        // Fullwidth ASCII variants
        assert_eq!(char_display_width('Ａ'), 2); // Fullwidth A
        assert_eq!(char_display_width('１'), 2); // Fullwidth 1
    }

    #[test]
    fn test_char_display_width_combining() {
        // Combining marks have width 0
        assert_eq!(char_display_width('\u{0301}'), 0); // Combining acute accent
        assert_eq!(char_display_width('\u{0308}'), 0); // Combining diaeresis
    }

    #[test]
    fn test_display_width_unicode_ascii() {
        assert_eq!(display_width_unicode("Hello", 4), 5);
        assert_eq!(display_width_unicode("Hello World", 4), 11);
    }

    #[test]
    fn test_display_width_unicode_cjk() {
        // 5 CJK characters = 10 display columns
        assert_eq!(display_width_unicode("中文测试字", 4), 10);
        // 3 CJK = 6 columns
        assert_eq!(display_width_unicode("日本語", 4), 6);
    }

    #[test]
    fn test_display_width_unicode_mixed() {
        // "Hello中文" = 5 + 4 = 9 columns
        assert_eq!(display_width_unicode("Hello中文", 4), 9);
        // "a中b" = 1 + 2 + 1 = 4 columns
        assert_eq!(display_width_unicode("a中b", 4), 4);
    }

    #[test]
    fn test_display_width_unicode_with_tabs() {
        // "a\t中" with tabstop=4: a(1) + tab(3) + 中(2) = 6
        assert_eq!(display_width_unicode("a\t中", 4), 6);
        // "\t中" with tabstop=4: tab(4) + 中(2) = 6
        assert_eq!(display_width_unicode("\t中", 4), 6);
    }

    #[test]
    fn test_display_width_unicode_combining() {
        // "é" as e + combining accent = 1 + 0 = 1
        assert_eq!(display_width_unicode("e\u{0301}", 4), 1);
        // "naïve" with combining diaeresis
        assert_eq!(display_width_unicode("nai\u{0308}ve", 4), 5);
    }

    #[test]
    fn test_display_width_unicode_tabs_and_cjk_integration() {
        // Critical integration test: tabs + CJK in same line
        // "中\t日" with tabstop=4: 中(2) + tab(2 to reach 4) + 日(2) = 6
        assert_eq!(display_width_unicode("中\t日", 4), 6);
        // "a中\tb" with tabstop=4: a(1) + 中(2) + tab(1 to reach 4) + b(1) = 5
        assert_eq!(display_width_unicode("a中\tb", 4), 5);
    }

    #[test]
    fn test_display_col_from_buffer_col_unicode_cjk() {
        let line = "a中b";
        // Buffer col 0 = display col 0
        assert_eq!(display_col_from_buffer_col_unicode(line, 0, 4), 0);
        // Buffer col 1 (after 'a') = display col 1
        assert_eq!(display_col_from_buffer_col_unicode(line, 1, 4), 1);
        // Buffer col 2 (after '中') = display col 3 (1 + 2)
        assert_eq!(display_col_from_buffer_col_unicode(line, 2, 4), 3);
        // Buffer col 3 (after 'b') = display col 4
        assert_eq!(display_col_from_buffer_col_unicode(line, 3, 4), 4);
    }

    #[test]
    fn test_buffer_col_from_display_col_unicode_cjk() {
        let line = "a中b";
        // Display col 0 = buffer col 0
        assert_eq!(buffer_col_from_display_col_unicode(line, 0, 4), 0);
        // Display col 1 = buffer col 1 (at '中')
        assert_eq!(buffer_col_from_display_col_unicode(line, 1, 4), 1);
        // Display col 2 = buffer col 1 (inside '中', can't land here)
        assert_eq!(buffer_col_from_display_col_unicode(line, 2, 4), 1);
        // Display col 3 = buffer col 2 (at 'b')
        assert_eq!(buffer_col_from_display_col_unicode(line, 3, 4), 2);
        // Display col 4 = buffer col 3 (after 'b')
        assert_eq!(buffer_col_from_display_col_unicode(line, 4, 4), 3);
    }

    #[test]
    fn test_display_position_unicode_cjk() {
        let line = "a中b";
        // Buffer col 2 (at 'b') = display col 3
        assert_eq!(display_position_unicode(line, 2, 80, 4), (0, 3));
    }

    #[test]
    fn test_unicode_roundtrip_cjk() {
        let line = "Hello中文World";
        for buffer_col in 0..=line.chars().count() {
            let display_col = display_col_from_buffer_col_unicode(line, buffer_col, 4);
            let back = buffer_col_from_display_col_unicode(line, display_col, 4);
            assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
        }
    }

    #[test]
    fn test_unicode_roundtrip_tabs_and_cjk() {
        let line = "a\t中\tb";
        for buffer_col in 0..=line.chars().count() {
            let display_col = display_col_from_buffer_col_unicode(line, buffer_col, 4);
            let back = buffer_col_from_display_col_unicode(line, display_col, 4);
            assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
        }
    }

    #[test]
    fn test_cursor_never_lands_inside_wide_char() {
        let line = "中"; // Single CJK char, 2 columns wide
        // Display col 0 = buffer col 0 (at start of '中')
        assert_eq!(buffer_col_from_display_col_unicode(line, 0, 4), 0);
        // Display col 1 = buffer col 0 (inside '中', stays at start)
        assert_eq!(buffer_col_from_display_col_unicode(line, 1, 4), 0);
        // Display col 2 = buffer col 1 (after '中')
        assert_eq!(buffer_col_from_display_col_unicode(line, 2, 4), 1);
    }

    #[test]
    fn test_display_line_count_unicode() {
        // CJK text that wraps
        let line = "中文中文中"; // 5 chars * 2 = 10 columns
        assert_eq!(display_line_count_unicode(line, 8, 4), 2); // 10 / 8 = 2
        assert_eq!(display_line_count_unicode(line, 10, 4), 1); // 10 / 10 = 1
        assert_eq!(display_line_count_unicode(line, 4, 4), 3); // 10 / 4 = 3
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_char_display_width_tab() {
        // Tab is reported as 1 by char_display_width (contextual width
        // should use char_display_width_at instead)
        assert_eq!(char_display_width('\t'), 1);
    }

    #[test]
    fn test_char_display_width_at_tab() {
        // Tab width depends on position
        assert_eq!(char_display_width_at('\t', 0, 4), 4);
        assert_eq!(char_display_width_at('\t', 1, 4), 3);
        assert_eq!(char_display_width_at('\t', 2, 4), 2);
        assert_eq!(char_display_width_at('\t', 3, 4), 1);
        assert_eq!(char_display_width_at('\t', 4, 4), 4);
        // Zero tabstop treated as 1
        assert_eq!(char_display_width_at('\t', 0, 0), 1);
    }

    #[test]
    fn test_display_line_count_with_tabs_zero_width() {
        // Terminal width 0 always returns 1
        assert_eq!(display_line_count_with_tabs("hello\tworld", 0, 4), 1);
    }

    #[test]
    fn test_display_line_count_with_tabs_empty_line() {
        // Empty line returns 1 (display_width_with_tabs returns 0, so width == 0 branch)
        assert_eq!(display_line_count_with_tabs("", 80, 4), 1);
    }

    #[test]
    fn test_display_position_with_tabs_zero_width() {
        // Terminal width 0: returns (0, display_col)
        let line = "a\tb";
        let (dl, dc) = display_position_with_tabs(line, 2, 0, 4);
        assert_eq!(dl, 0);
        assert_eq!(dc, 4); // 'a' + tab(3) = 4
    }

    #[test]
    fn test_display_line_count_unicode_zero_width() {
        // Terminal width 0 always returns 1
        assert_eq!(display_line_count_unicode("中文", 0, 4), 1);
    }

    #[test]
    fn test_display_line_count_unicode_empty() {
        // Empty line returns 1
        assert_eq!(display_line_count_unicode("", 80, 4), 1);
    }

    #[test]
    fn test_display_position_unicode_zero_width() {
        // Terminal width 0: returns (0, display_col)
        let line = "a中b";
        let (dl, dc) = display_position_unicode(line, 2, 0, 4);
        assert_eq!(dl, 0);
        assert_eq!(dc, 3); // 'a'(1) + '中'(2) = 3
    }

    #[test]
    fn test_display_width_unicode_empty() {
        assert_eq!(display_width_unicode("", 4), 0);
    }

    #[test]
    fn test_buffer_col_from_display_col_past_end() {
        // Target display col beyond line length
        let line = "abc";
        assert_eq!(buffer_col_from_display_col(line, 100, 4), 3);
    }

    #[test]
    fn test_buffer_col_from_display_col_unicode_past_end() {
        // Target display col beyond line length
        let line = "a中";
        assert_eq!(buffer_col_from_display_col_unicode(line, 100, 4), 2);
    }

    #[test]
    fn test_display_col_from_buffer_col_past_end() {
        // Buffer col beyond line length
        let line = "abc";
        assert_eq!(display_col_from_buffer_col(line, 100, 4), 3);
    }

    #[test]
    fn test_display_col_from_buffer_col_unicode_past_end() {
        // Buffer col beyond line length
        let line = "a中";
        assert_eq!(display_col_from_buffer_col_unicode(line, 100, 4), 3);
    }
}
