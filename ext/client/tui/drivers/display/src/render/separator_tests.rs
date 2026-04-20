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
