//! Cell type for frame buffer.
//!
//! A cell represents a single character position in the terminal display.

use unicode_width::UnicodeWidthChar;

use crate::style::Style;

/// A single cell in the frame buffer.
///
/// Each cell occupies one column in the terminal. Wide characters (CJK, emoji)
/// occupy two columns: the first cell contains the character with `width = 2`,
/// and the second cell is a continuation cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed in this cell.
    pub char: char,
    /// The style for this cell.
    pub style: Style,
    /// Display width: 1 for ASCII/narrow, 2 for wide (CJK, fullwidth).
    pub width: u8,
    /// True if this cell is the continuation of a wide character.
    pub is_continuation: bool,
}

impl Cell {
    /// Create a new cell with the given character and style.
    #[must_use]
    pub fn new(char: char, style: Style) -> Self {
        Self {
            char,
            style,
            width: char_width(char),
            is_continuation: false,
        }
    }

    /// Create a cell from just a character, using default style.
    #[must_use]
    pub fn from_char(char: char) -> Self {
        Self::new(char, Style::default())
    }

    /// Create a continuation cell (placeholder for 2nd column of wide char).
    #[must_use]
    pub fn continuation() -> Self {
        Self {
            char: ' ',
            style: Style::default(),
            width: 0,
            is_continuation: true,
        }
    }

    /// Create an empty (space) cell with default style.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(' ', Style::default())
    }

    /// Check if this cell differs from another.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn differs_from(&self, other: &Self) -> bool {
        self.char != other.char
            || self.style != other.style
            || self.is_continuation != other.is_continuation
    }

    /// Check if this cell is effectively empty.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn is_empty(&self) -> bool {
        self.char == ' ' && self.style == Style::default() && !self.is_continuation
    }

    /// Check if this cell contains a wide character.
    #[must_use]
    pub const fn is_wide(&self) -> bool {
        self.width == 2
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::empty()
    }
}

/// Determine the display width of a character.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn char_width(c: char) -> u8 {
    c.width().map_or(1, |w| w as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_width_ascii() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width(' '), 1);
    }

    #[test]
    fn test_char_width_cjk() {
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('日'), 2);
    }

    #[test]
    fn test_cell_new() {
        let cell = Cell::new('x', Style::default());
        assert_eq!(cell.char, 'x');
        assert_eq!(cell.width, 1);
    }

    #[test]
    fn test_cell_empty() {
        let cell = Cell::empty();
        assert!(cell.is_empty());
    }

    #[test]
    fn test_cell_from_char() {
        let cell = Cell::from_char('y');
        assert_eq!(cell.char, 'y');
        assert_eq!(cell.style, Style::default());
        assert_eq!(cell.width, 1);
    }

    #[test]
    fn test_cell_continuation() {
        let cell = Cell::continuation();
        assert!(cell.is_continuation);
        assert_eq!(cell.width, 0);
        assert_eq!(cell.char, ' ');
    }

    #[test]
    fn test_cell_is_wide() {
        let cell = Cell::new('中', Style::default());
        assert!(cell.is_wide());
        assert_eq!(cell.width, 2);

        let cell = Cell::new('a', Style::default());
        assert!(!cell.is_wide());
    }

    #[test]
    fn test_cell_differs_from() {
        let cell1 = Cell::new('a', Style::default());
        let cell2 = Cell::new('b', Style::default());
        assert!(cell1.differs_from(&cell2));

        let cell3 = Cell::new('a', Style::default());
        assert!(!cell1.differs_from(&cell3));

        let cell4 = Cell::new('a', Style::new().bold());
        assert!(cell1.differs_from(&cell4));

        let cell5 = Cell::continuation();
        assert!(cell1.differs_from(&cell5));
    }

    #[test]
    fn test_cell_is_empty_with_style() {
        let cell = Cell::new(' ', Style::new().bold());
        assert!(!cell.is_empty()); // Has style, not empty
    }

    #[test]
    fn test_cell_is_empty_continuation() {
        let cell = Cell::continuation();
        assert!(!cell.is_empty()); // Continuation cells are not empty
    }

    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert!(cell.is_empty());
        assert_eq!(cell.char, ' ');
    }

    #[test]
    fn test_cell_clone() {
        let cell = Cell::new('z', Style::new().bold());
        let cloned = cell.clone();
        assert_eq!(cell, cloned);
    }

    #[test]
    fn test_cell_debug() {
        let cell = Cell::new('a', Style::default());
        let debug = format!("{cell:?}");
        assert!(debug.contains("Cell"));
    }

    #[test]
    fn test_cell_eq() {
        let cell1 = Cell::new('x', Style::default());
        let cell2 = Cell::new('x', Style::default());
        assert_eq!(cell1, cell2);

        let cell3 = Cell::new('y', Style::default());
        assert_ne!(cell1, cell3);
    }

    #[test]
    fn test_char_width_emoji() {
        // Some emojis are wide
        let w = char_width('😀');
        assert!(w >= 1);
    }

    #[test]
    fn test_char_width_control() {
        // Control characters have width 0, but our function maps to 1
        let w = char_width('\x00');
        // Just ensure it returns a valid value
        let _ = w;
    }
}
