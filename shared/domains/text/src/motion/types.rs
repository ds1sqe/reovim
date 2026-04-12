//! Motion type definitions.

use crate::direction::{Direction, LinePosition, WordBoundary};

/// Motion types for cursor movement.
///
/// Each variant represents a different type of cursor motion that vim supports.
/// Motions can be used with operators (d, y, c) or on their own for navigation.
///
/// # Example
///
/// ```
/// use reovim_domain_text::*;
///
/// // Character motion (h, l)
/// let left = Motion::Char(Direction::Backward);
/// let right = Motion::Char(Direction::Forward);
///
/// // Word motion (w, b, e)
/// let word_forward = Motion::Word {
///     direction: Direction::Forward,
///     boundary: WordBoundary::Word,
///     end: false,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Character motion (h, l)
    Char(Direction),

    /// Line motion (j, k)
    Line(Direction),

    /// Word motion (w, b, e, ge, W, B, E, gE)
    Word {
        /// Direction of movement
        direction: Direction,
        /// Word boundary type (word vs WORD)
        boundary: WordBoundary,
        /// If true, move to end of word (e/E/ge/gE), else to start (w/W/b/B)
        end: bool,
    },

    /// Line position (0, ^, $, g_)
    LinePosition(LinePosition),

    /// Paragraph motion ({, })
    Paragraph(Direction),

    /// Find character on line (f, F, t, T)
    FindChar {
        /// Character to find
        char: char,
        /// Direction to search
        direction: Direction,
        /// If true, stop before the character (t/T), else on it (f/F)
        till: bool,
    },

    /// Jump to line (G, gg)
    ///
    /// - `None` - jump to last line (G) or first line (gg)
    /// - `Some(n)` - jump to line n (1-indexed in vim, 0-indexed internally)
    JumpLine(Option<usize>),

    /// Match bracket (%)
    MatchBracket,
}

impl Motion {
    /// Check if this motion is linewise.
    ///
    /// Linewise motions operate on whole lines rather than character ranges.
    /// This affects how operators (d, y, c) interpret the motion.
    ///
    /// # Example
    ///
    /// - `dj` deletes current line and next line (linewise)
    /// - `dw` deletes to start of next word (characterwise)
    #[must_use]
    pub const fn is_linewise(&self) -> bool {
        matches!(self, Self::Line(_) | Self::JumpLine(_) | Self::Paragraph(_))
    }

    /// Check if this motion is inclusive.
    ///
    /// Inclusive motions include the character at the target position.
    /// This affects operators like delete and yank.
    ///
    /// # Example
    ///
    /// - `d$` deletes to end of line including the last character (inclusive)
    /// - `dw` deletes to start of next word excluding the first character (exclusive)
    #[must_use]
    pub const fn is_inclusive(&self) -> bool {
        matches!(
            self,
            Self::LinePosition(LinePosition::End | LinePosition::LastNonBlank)
                | Self::Word { end: true, .. }
                | Self::MatchBracket
                | Self::FindChar { till: false, .. }
        )
    }
}
