//! Window separator rendering.
//!
//! Renders the separator lines between split windows, including
//! proper intersection characters where separators meet.

use crate::{compositor::Style, frame::FrameBuffer};

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
/// have connecting separators.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `x` - X position
/// * `y` - Y position
/// * `has_left` - Has separator extending left
/// * `has_right` - Has separator extending right
/// * `has_up` - Has separator extending up
/// * `has_down` - Has separator extending down
/// * `chars` - Separator character set
/// * `style` - Style for the intersection
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn render_intersection(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    has_left: bool,
    has_right: bool,
    has_up: bool,
    has_down: bool,
    chars: &SeparatorChars,
    style: &Style,
) {
    let ch = select_intersection_char(has_left, has_right, has_up, has_down, chars);
    buffer.put_char(x, y, ch, style);
}

/// Select the appropriate intersection character based on connected directions.
#[allow(clippy::fn_params_excessive_bools, clippy::match_same_arms)]
const fn select_intersection_char(
    has_left: bool,
    has_right: bool,
    has_up: bool,
    has_down: bool,
    chars: &SeparatorChars,
) -> char {
    match (has_left, has_right, has_up, has_down) {
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
                    x > 0,          // has_left
                    x < width - 1,  // has_right
                    y > 0,          // has_up
                    y < height - 1, // has_down
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

    #[test]
    fn test_select_intersection_cross() {
        let chars = SeparatorChars::SINGLE;

        // Full cross: all directions connected
        let ch = select_intersection_char(true, true, true, true, &chars);
        assert_eq!(ch, '┼');
    }

    #[test]
    fn test_select_intersection_top_tee() {
        let chars = SeparatorChars::SINGLE;

        // Top tee: left, right, down (no up)
        let ch = select_intersection_char(true, true, false, true, &chars);
        assert_eq!(ch, '┬');
    }

    #[test]
    fn test_select_intersection_bottom_tee() {
        let chars = SeparatorChars::SINGLE;

        // Bottom tee: left, right, up (no down)
        let ch = select_intersection_char(true, true, true, false, &chars);
        assert_eq!(ch, '┴');
    }

    #[test]
    fn test_select_intersection_left_tee() {
        let chars = SeparatorChars::SINGLE;

        // Left tee: right, up, down (no left)
        let ch = select_intersection_char(false, true, true, true, &chars);
        assert_eq!(ch, '├');
    }

    #[test]
    fn test_select_intersection_right_tee() {
        let chars = SeparatorChars::SINGLE;

        // Right tee: left, up, down (no right)
        let ch = select_intersection_char(true, false, true, true, &chars);
        assert_eq!(ch, '┤');
    }

    #[test]
    fn test_select_intersection_horizontal() {
        let chars = SeparatorChars::SINGLE;

        // Just horizontal: left, right
        let ch = select_intersection_char(true, true, false, false, &chars);
        assert_eq!(ch, '─');
    }

    #[test]
    fn test_select_intersection_vertical() {
        let chars = SeparatorChars::SINGLE;

        // Just vertical: up, down
        let ch = select_intersection_char(false, false, true, true, &chars);
        assert_eq!(ch, '│');
    }

    #[test]
    fn test_select_intersection_none() {
        let chars = SeparatorChars::SINGLE;

        // No connections
        let ch = select_intersection_char(false, false, false, false, &chars);
        assert_eq!(ch, ' ');
    }

    #[test]
    fn test_render_intersection() {
        let mut buffer = FrameBuffer::new(80, 24);
        let chars = SeparatorChars::SINGLE;

        render_intersection(&mut buffer, 10, 10, true, true, true, true, &chars, &Style::default());

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
    fn test_select_intersection_corners() {
        let chars = SeparatorChars::SINGLE;

        // Bottom-left corner: right + down (no left, no up)
        let ch = select_intersection_char(false, true, false, true, &chars);
        assert_eq!(ch, chars.vertical);

        // Bottom-right corner: left + down (no right, no up)
        let ch = select_intersection_char(true, false, false, true, &chars);
        assert_eq!(ch, chars.vertical);

        // Top-left corner: right + up (no left, no down)
        let ch = select_intersection_char(false, true, true, false, &chars);
        assert_eq!(ch, chars.vertical);

        // Top-right corner: left + up (no right, no down)
        let ch = select_intersection_char(true, false, true, false, &chars);
        assert_eq!(ch, chars.vertical);
    }

    #[test]
    fn test_select_intersection_single_direction() {
        let chars = SeparatorChars::SINGLE;

        // Single left endpoint
        let ch = select_intersection_char(true, false, false, false, &chars);
        assert_eq!(ch, chars.horizontal);

        // Single right endpoint
        let ch = select_intersection_char(false, true, false, false, &chars);
        assert_eq!(ch, chars.horizontal);

        // Single up endpoint
        let ch = select_intersection_char(false, false, true, false, &chars);
        assert_eq!(ch, chars.vertical);

        // Single down endpoint
        let ch = select_intersection_char(false, false, false, true, &chars);
        assert_eq!(ch, chars.vertical);
    }
}
