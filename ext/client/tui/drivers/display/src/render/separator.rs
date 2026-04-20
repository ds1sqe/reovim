//! Window separator rendering.
//!
//! Renders the separator lines between split windows, including
//! proper intersection characters where separators meet.

use crate::{WindowAdjacency, compositor::Style, frame::FrameBuffer};

/// Separator character set for window dividers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparatorChars {
    /// Vertical separator character.
    pub vertical: char,
    /// Horizontal separator character.
    pub horizontal: char,
    /// Cross intersection (all four directions).
    pub cross: char,
    /// Top tee (vertical with branch down).
    pub top_tee: char,
    /// Bottom tee (vertical with branch up).
    pub bottom_tee: char,
    /// Left tee (horizontal with branch right).
    pub left_tee: char,
    /// Right tee (horizontal with branch left).
    pub right_tee: char,
}

impl Default for SeparatorChars {
    fn default() -> Self {
        Self::SINGLE
    }
}

impl SeparatorChars {
    /// Single line separators (default).
    pub const SINGLE: Self = Self {
        vertical: '│',
        horizontal: '─',
        cross: '┼',
        top_tee: '┬',
        bottom_tee: '┴',
        left_tee: '├',
        right_tee: '┤',
    };

    /// Double line separators.
    pub const DOUBLE: Self = Self {
        vertical: '║',
        horizontal: '═',
        cross: '╬',
        top_tee: '╦',
        bottom_tee: '╩',
        left_tee: '╠',
        right_tee: '╣',
    };

    /// Bold line separators.
    pub const BOLD: Self = Self {
        vertical: '┃',
        horizontal: '━',
        cross: '╋',
        top_tee: '┳',
        bottom_tee: '┻',
        left_tee: '┣',
        right_tee: '┫',
    };

    /// ASCII separators (for terminals without Unicode support).
    pub const ASCII: Self = Self {
        vertical: '|',
        horizontal: '-',
        cross: '+',
        top_tee: '+',
        bottom_tee: '+',
        left_tee: '+',
        right_tee: '+',
    };
}

/// Render a vertical separator line.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `x` - X position of the separator
/// * `y` - Starting Y position
/// * `height` - Height of the separator
/// * `chars` - Separator character set
/// * `style` - Style for the separator
pub fn render_vseparator(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    height: u16,
    chars: &SeparatorChars,
    style: &Style,
) {
    for row in 0..height {
        buffer.put_char(x, y + row, chars.vertical, style);
    }
}

/// Render a horizontal separator line.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `x` - Starting X position
/// * `y` - Y position of the separator
/// * `width` - Width of the separator
/// * `chars` - Separator character set
/// * `style` - Style for the separator
pub fn render_hseparator(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    width: u16,
    chars: &SeparatorChars,
    style: &Style,
) {
    for col in 0..width {
        buffer.put_char(x + col, y, chars.horizontal, style);
    }
}

/// Render an intersection point where separators meet.
///
/// The intersection character is chosen based on which directions
/// have connecting separators, described by [`WindowAdjacency`].
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `x` - X position
/// * `y` - Y position
/// * `adj` - Which directions have connecting separators
/// * `chars` - Separator character set
/// * `style` - Style for the intersection
pub fn render_intersection(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    adj: WindowAdjacency,
    chars: &SeparatorChars,
    style: &Style,
) {
    let ch = select_intersection_char(adj, chars);
    buffer.put_char(x, y, ch, style);
}

/// Select the appropriate intersection character based on connected directions.
#[allow(clippy::match_same_arms)]
const fn select_intersection_char(adj: WindowAdjacency, chars: &SeparatorChars) -> char {
    match (adj.left, adj.right, adj.top, adj.bottom) {
        // Full cross
        (true, true, true, true) => chars.cross,

        // T-junctions
        (true, true, false, true) => chars.top_tee, // ┬
        (true, true, true, false) => chars.bottom_tee, // ┴
        (false, true, true, true) => chars.left_tee, // ├
        (true, false, true, true) => chars.right_tee, // ┤

        // Corners (use appropriate tee or corner logic)
        (false, true, false, true) => chars.vertical, // └ approximated
        (true, false, false, true) => chars.vertical, // ┘ approximated
        (false, true, true, false) => chars.vertical, // ┌ approximated
        (true, false, true, false) => chars.vertical, // ┐ approximated

        // Straight lines
        (true, true, false, false) => chars.horizontal,
        (false, false, true, true) => chars.vertical,

        // Single direction (endpoints)
        (true, false, false, false) => chars.horizontal,
        (false, true, false, false) => chars.horizontal,
        (false, false, true, false) => chars.vertical,
        (false, false, false, true) => chars.vertical,

        // No connections
        (false, false, false, false) => ' ',
    }
}

/// Render separators between a grid of windows.
///
/// This is a convenience function for rendering all separators
/// in a tiled layout.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `cols` - Column positions (x coordinates of vertical separators)
/// * `rows` - Row positions (y coordinates of horizontal separators)
/// * `width` - Total width
/// * `height` - Total height
/// * `chars` - Separator character set
/// * `style` - Style for separators
pub fn render_grid_separators(
    buffer: &mut FrameBuffer,
    cols: &[u16],
    rows: &[u16],
    width: u16,
    height: u16,
    chars: &SeparatorChars,
    style: &Style,
) {
    // Render vertical separators
    for &x in cols {
        if x < width {
            render_vseparator(buffer, x, 0, height, chars, style);
        }
    }

    // Render horizontal separators
    for &y in rows {
        if y < height {
            render_hseparator(buffer, 0, y, width, chars, style);
        }
    }

    // Render intersections
    for &x in cols {
        for &y in rows {
            if x < width && y < height {
                render_intersection(
                    buffer,
                    x,
                    y,
                    WindowAdjacency {
                        left: x > 0,
                        right: x < width - 1,
                        top: y > 0,
                        bottom: y < height - 1,
                    },
                    chars,
                    style,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "separator_tests.rs"]
mod tests;
