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
#[path = "cell_tests.rs"]
mod tests;
