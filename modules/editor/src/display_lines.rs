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
//! # Limitations
//!
//! Current implementation:
//! - Assumes 1 column per character (no CJK double-width support)
//! - Assumes tabs render as 1 column (actual tab width not considered)
//! - Simple division-based wrapping (no word wrap)

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
}
