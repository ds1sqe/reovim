//! Single cell representation in the frame buffer

use crate::highlight::Style;

/// A single cell in the frame buffer representing one character position
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed (default: ' ')
    pub char: char,
    /// The styling applied to this cell
    pub style: Style,
    /// Display width of the character (1 for ASCII, 2 for wide chars like CJK)
    pub width: u8,
    /// Whether this cell is a continuation of a wide character (width=2)
    pub is_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            style: Style::default(),
            width: 1,
            is_continuation: false,
        }
    }
}

impl Cell {
    /// Create a new cell with the given character and style
    #[must_use]
    pub fn new(char: char, style: Style) -> Self {
        let width = unicode_display_width(char);
        Self {
            char,
            style,
            width,
            is_continuation: false,
        }
    }

    /// Create a cell with just a character (default style)
    #[must_use]
    pub fn from_char(char: char) -> Self {
        Self::new(char, Style::default())
    }

    /// Create a continuation cell for wide characters
    ///
    /// Continuation cells occupy the second column of a wide character (width=2).
    /// They have width=0 and are skipped during diff calculation and rendering.
    #[must_use]
    pub const fn continuation(style: Style) -> Self {
        Self {
            char: ' ',
            style,
            width: 0,
            is_continuation: true,
        }
    }

    /// Check if this cell differs from another (for diff-based rendering)
    #[must_use]
    pub fn differs_from(&self, other: &Self) -> bool {
        self.char != other.char || self.style != other.style
    }

    /// Check if this is an empty/blank cell
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.char == ' ' && self.style == Style::default()
    }
}

/// Get the display width of a character
/// Returns 2 for wide characters (CJK, etc.), 1 for others
#[must_use]
fn unicode_display_width(c: char) -> u8 {
    // Simple heuristic: CJK characters are typically double-width
    // Full implementation would use unicode-width crate
    if is_wide_char(c) { 2 } else { 1 }
}

/// Check if a character is a wide character (double-width display)
#[must_use]
pub fn is_wide_char(c: char) -> bool {
    let cp = u32::from(c);
    // CJK Unified Ideographs and common wide character ranges
    // NOTE: Nerd Font icons (PUA) are NOT included because terminal rendering varies:
    // - Nerd Font "Mono" renders them as width=1
    // - Nerd Font "Propo" renders them as width=2
    // We treat them as width=1 for compatibility with Mono fonts.
    matches!(
        cp,
        0x1100..=0x115F       // Hangul Jamo
        | 0x2E80..=0x9FFF     // CJK Radicals through CJK Unified Ideographs
        | 0xAC00..=0xD7A3     // Hangul Syllables
        | 0xF900..=0xFAFF     // CJK Compatibility Ideographs
        | 0xFE10..=0xFE1F     // Vertical Forms
        | 0xFE30..=0xFE6F     // CJK Compatibility Forms
        | 0xFF00..=0xFF60     // Fullwidth Forms
        | 0xFFE0..=0xFFE6     // Fullwidth Symbol Variants
        | 0x20000..=0x2_FFFF  // CJK Extension B-F
        | 0x30000..=0x3_FFFF  // CJK Extension G+
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_empty() {
        let cell = Cell::default();
        assert!(cell.is_empty());
        assert_eq!(cell.char, ' ');
        assert_eq!(cell.width, 1);
    }

    #[test]
    fn test_cell_differs() {
        let a = Cell::from_char('a');
        let b = Cell::from_char('b');
        let a2 = Cell::from_char('a');

        assert!(a.differs_from(&b));
        assert!(!a.differs_from(&a2));
    }

    #[test]
    fn test_wide_char_detection() {
        assert!(!is_wide_char('a'));
        assert!(!is_wide_char('Z'));
        assert!(is_wide_char('中'));
        assert!(is_wide_char('日'));
        assert!(is_wide_char('한'));
    }
}
