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
mod tests {
    use super::*;

    #[test]
    fn test_separator_chars_default() {
        let chars = SeparatorChars::default();
        assert_eq!(chars.vertical, '│');
        assert_eq!(chars.horizontal, '─');
        assert_eq!(chars.cross, '┼');
    }

    #[test]
    fn test_separator_chars_single() {
        let chars = SeparatorChars::SINGLE;
        assert_eq!(chars.vertical, '│');
        assert_eq!(chars.top_tee, '┬');
        assert_eq!(chars.bottom_tee, '┴');
    }

    #[test]
    fn test_separator_chars_double() {
        let chars = SeparatorChars::DOUBLE;
        assert_eq!(chars.vertical, '║');
        assert_eq!(chars.horizontal, '═');
        assert_eq!(chars.cross, '╬');
    }

    #[test]
    fn test_separator_chars_bold() {
        let chars = SeparatorChars::BOLD;
        assert_eq!(chars.vertical, '┃');
        assert_eq!(chars.horizontal, '━');
        assert_eq!(chars.cross, '╋');
    }

    #[test]
    fn test_separator_chars_ascii() {
        let chars = SeparatorChars::ASCII;
        assert_eq!(chars.vertical, '|');
        assert_eq!(chars.horizontal, '-');
        assert_eq!(chars.cross, '+');
    }

    #[test]
    fn test_render_vseparator() {
        let mut buffer = FrameBuffer::new(80, 24);
        let chars = SeparatorChars::SINGLE;

        render_vseparator(&mut buffer, 10, 5, 10, &chars, &Style::default());

        for y in 5..15 {
            assert_eq!(buffer.get(10, y).unwrap().char, '│');
        }
    }

    #[test]
    fn test_render_hseparator() {
        let mut buffer = FrameBuffer::new(80, 24);
        let chars = SeparatorChars::SINGLE;

        render_hseparator(&mut buffer, 5, 10, 20, &chars, &Style::default());

        for x in 5..25 {
            assert_eq!(buffer.get(x, 10).unwrap().char, '─');
        }
    }

    use crate::WindowAdjacency as Adj;

    #[test]
    fn test_select_intersection_cross() {
        let chars = SeparatorChars::SINGLE;
        assert_eq!(select_intersection_char(Adj::ALL, &chars), '┼');
    }

    #[test]
    fn test_select_intersection_top_tee() {
        let chars = SeparatorChars::SINGLE;
        // Top tee: left, right, down (no up)
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::RIGHT | Adj::BOTTOM, &chars), '┬');
    }

    #[test]
    fn test_select_intersection_bottom_tee() {
        let chars = SeparatorChars::SINGLE;
        // Bottom tee: left, right, up (no down)
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::RIGHT | Adj::TOP, &chars), '┴');
    }

    #[test]
    fn test_select_intersection_left_tee() {
        let chars = SeparatorChars::SINGLE;
        // Left tee: right, up, down (no left)
        assert_eq!(select_intersection_char(Adj::RIGHT | Adj::TOP | Adj::BOTTOM, &chars), '├');
    }

    #[test]
    fn test_select_intersection_right_tee() {
        let chars = SeparatorChars::SINGLE;
        // Right tee: left, up, down (no right)
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::TOP | Adj::BOTTOM, &chars), '┤');
    }

    #[test]
    fn test_select_intersection_horizontal() {
        let chars = SeparatorChars::SINGLE;
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::RIGHT, &chars), '─');
    }

    #[test]
    fn test_select_intersection_vertical() {
        let chars = SeparatorChars::SINGLE;
        assert_eq!(select_intersection_char(Adj::TOP | Adj::BOTTOM, &chars), '│');
    }

    #[test]
    fn test_select_intersection_none() {
        let chars = SeparatorChars::SINGLE;
        assert_eq!(select_intersection_char(Adj::NONE, &chars), ' ');
    }

    #[test]
    fn test_render_intersection() {
        let mut buffer = FrameBuffer::new(80, 24);
        let chars = SeparatorChars::SINGLE;

        render_intersection(&mut buffer, 10, 10, Adj::ALL, &chars, &Style::default());

        assert_eq!(buffer.get(10, 10).unwrap().char, '┼');
    }

    #[test]
    fn test_render_grid_separators() {
        let mut buffer = FrameBuffer::new(80, 24);
        let chars = SeparatorChars::SINGLE;

        // Grid with one vertical separator at x=40 and one horizontal at y=12
        render_grid_separators(&mut buffer, &[40], &[12], 80, 24, &chars, &Style::default());

        // Check vertical separator
        assert_eq!(buffer.get(40, 5).unwrap().char, '│');

        // Check horizontal separator
        assert_eq!(buffer.get(20, 12).unwrap().char, '─');

        // Check intersection
        assert_eq!(buffer.get(40, 12).unwrap().char, '┼');
    }

    #[test]
    fn test_grid_separators_col_at_edge() {
        // Line 222: x >= width (column position at buffer edge, skipped)
        let mut buffer = FrameBuffer::new(10, 10);
        let chars = SeparatorChars::SINGLE;
        let style = Style::default();

        // Column at x=10 is >= width=10, should be skipped
        render_grid_separators(&mut buffer, &[10], &[], 10, 10, &chars, &style);
        // No vertical separator should be drawn
        // All cells should still be spaces
        assert_eq!(buffer.get(9, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_grid_separators_row_at_edge() {
        // Line 229: y >= height (row position at buffer edge, skipped)
        let mut buffer = FrameBuffer::new(10, 10);
        let chars = SeparatorChars::SINGLE;
        let style = Style::default();

        // Row at y=10 is >= height=10, should be skipped
        render_grid_separators(&mut buffer, &[], &[10], 10, 10, &chars, &style);
        assert_eq!(buffer.get(0, 9).unwrap().char, ' ');
    }

    #[test]
    fn test_grid_separators_intersection_partial_bounds() {
        // Line 237: x < width && y < height with one condition false
        // (intersection skipped when col or row is at edge)
        let mut buffer = FrameBuffer::new(10, 10);
        let chars = SeparatorChars::SINGLE;
        let style = Style::default();

        // col at x=10 (>= width) and row at y=5 (< height)
        // x < width is false, so intersection is skipped
        render_grid_separators(&mut buffer, &[10], &[5], 10, 10, &chars, &style);

        // Only the horizontal separator at y=5 should be drawn (row is valid)
        assert_eq!(buffer.get(0, 5).unwrap().char, chars.horizontal);
        // No intersection at (10, 5) since x >= width
    }

    #[test]
    fn test_grid_separators_intersection_x_ok_y_out() {
        // Line 237: x < width (true) && y < height (false)
        let mut buffer = FrameBuffer::new(10, 10);
        let chars = SeparatorChars::SINGLE;
        let style = Style::default();

        // col at x=5 (< width=10) and row at y=10 (>= height=10)
        // x < width is true, y < height is false -> intersection skipped
        render_grid_separators(&mut buffer, &[5], &[10], 10, 10, &chars, &style);

        // Vertical separator at x=5 should be drawn, but no intersection
        assert_eq!(buffer.get(5, 0).unwrap().char, chars.vertical);
    }

    #[test]
    fn test_select_intersection_corners() {
        let chars = SeparatorChars::SINGLE;

        assert_eq!(select_intersection_char(Adj::RIGHT | Adj::BOTTOM, &chars), chars.vertical);
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::BOTTOM, &chars), chars.vertical);
        assert_eq!(select_intersection_char(Adj::RIGHT | Adj::TOP, &chars), chars.vertical);
        assert_eq!(select_intersection_char(Adj::LEFT | Adj::TOP, &chars), chars.vertical);
    }

    #[test]
    fn test_select_intersection_single_direction() {
        let chars = SeparatorChars::SINGLE;

        assert_eq!(select_intersection_char(Adj::LEFT, &chars), chars.horizontal);
        assert_eq!(select_intersection_char(Adj::RIGHT, &chars), chars.horizontal);
        assert_eq!(select_intersection_char(Adj::TOP, &chars), chars.vertical);
        assert_eq!(select_intersection_char(Adj::BOTTOM, &chars), chars.vertical);
    }
}
