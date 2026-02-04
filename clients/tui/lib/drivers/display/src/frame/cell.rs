//! Cell type for frame buffer.
//!
//! A cell represents a single character position in the terminal display,
//! including the character, its style, display width, and continuation status.

use unicode_width::UnicodeWidthChar;

use crate::compositor::Style;

/// A single cell in the frame buffer.
///
/// Each cell occupies one column in the terminal. Wide characters (CJK, emoji)
/// occupy two columns: the first cell contains the character with `width = 2`,
/// and the second cell is a continuation cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed in this cell.
    pub char: char,
    /// The style (foreground, background, attributes) for this cell.
    pub style: Style,
    /// Display width: 1 for ASCII/narrow, 2 for wide (CJK, fullwidth).
    pub width: u8,
    /// True if this cell is the continuation of a wide character.
    /// Continuation cells should not be rendered directly.
    pub is_continuation: bool,
}

impl Cell {
    /// Create a new cell with the given character and style.
    ///
    /// Automatically computes the display width based on Unicode properties.
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

    /// Check if this cell differs from another (for diff rendering).
    ///
    /// Two cells are considered different if their character OR style differs.
    /// Continuation status is also compared.
    #[must_use]
    pub fn differs_from(&self, other: &Self) -> bool {
        self.char != other.char
            || self.style != other.style
            || self.is_continuation != other.is_continuation
    }

    /// Check if this cell is effectively empty (space with default style).
    #[must_use]
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
///
/// Returns:
/// - 2 for wide characters (CJK, fullwidth, some emoji)
/// - 1 for regular ASCII and narrow characters
/// - 1 for control characters (fallback)
///
/// Uses the `unicode-width` crate for accurate width detection.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn char_width(c: char) -> u8 {
    // Safe: character widths are always 0, 1, or 2 (never exceed u8)
    c.width().map_or(1, |w| w as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_width_ascii() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('Z'), 1);
        assert_eq!(char_width('0'), 1);
        assert_eq!(char_width(' '), 1);
        assert_eq!(char_width('!'), 1);
    }

    #[test]
    fn test_char_width_cjk() {
        // CJK characters are width 2
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('日'), 2);
        assert_eq!(char_width('本'), 2);
        assert_eq!(char_width('語'), 2);
        assert_eq!(char_width('한'), 2); // Korean
        assert_eq!(char_width('あ'), 2); // Hiragana
        assert_eq!(char_width('ア'), 2); // Katakana
    }

    #[test]
    fn test_char_width_fullwidth() {
        // Fullwidth ASCII variants
        assert_eq!(char_width('Ａ'), 2); // Fullwidth A
        assert_eq!(char_width('１'), 2); // Fullwidth 1
    }

    #[test]
    fn test_cell_new() {
        let cell = Cell::new('x', Style::default());
        assert_eq!(cell.char, 'x');
        assert_eq!(cell.width, 1);
        assert!(!cell.is_continuation);
    }

    #[test]
    fn test_cell_from_char() {
        let cell = Cell::from_char('中');
        assert_eq!(cell.char, '中');
        assert_eq!(cell.width, 2);
        assert!(!cell.is_continuation);
    }

    #[test]
    fn test_cell_continuation() {
        let cell = Cell::continuation();
        assert_eq!(cell.char, ' ');
        assert_eq!(cell.width, 0);
        assert!(cell.is_continuation);
    }

    #[test]
    fn test_cell_empty() {
        let cell = Cell::empty();
        assert_eq!(cell.char, ' ');
        assert_eq!(cell.width, 1);
        assert!(!cell.is_continuation);
        assert!(cell.is_empty());
    }

    #[test]
    fn test_cell_differs_from() {
        let cell1 = Cell::from_char('a');
        let cell2 = Cell::from_char('a');
        let cell3 = Cell::from_char('b');

        assert!(!cell1.differs_from(&cell2));
        assert!(cell1.differs_from(&cell3));
    }

    #[test]
    fn test_cell_is_wide() {
        assert!(!Cell::from_char('a').is_wide());
        assert!(Cell::from_char('中').is_wide());
    }

    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert!(cell.is_empty());
    }
}
