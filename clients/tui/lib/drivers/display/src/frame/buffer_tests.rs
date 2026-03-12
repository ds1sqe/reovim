use super::*;

#[test]
fn test_new() {
    let buf = FrameBuffer::new(10, 5);
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 5);
}

#[test]
fn test_get_set() {
    let mut buf = FrameBuffer::new(10, 10);

    let cell = Cell::from_char('x');
    buf.set(5, 5, cell.clone());

    assert_eq!(buf.get(5, 5), Some(&cell));
    assert_eq!(buf.get(0, 0), Some(&Cell::empty()));
}

#[test]
fn test_get_out_of_bounds() {
    let buf = FrameBuffer::new(10, 10);
    assert_eq!(buf.get(10, 0), None);
    assert_eq!(buf.get(0, 10), None);
    assert_eq!(buf.get(100, 100), None);
}

#[test]
fn test_put_char_ascii() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(0, 0, 'a', &Style::default());

    let cell = buf.get(0, 0).unwrap();
    assert_eq!(cell.char, 'a');
    assert_eq!(cell.width, 1);
    assert!(!cell.is_continuation);
}

#[test]
fn test_put_char_wide() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(0, 0, '中', &Style::default());

    let cell0 = buf.get(0, 0).unwrap();
    assert_eq!(cell0.char, '中');
    assert_eq!(cell0.width, 2);
    assert!(!cell0.is_continuation);

    let cell1 = buf.get(1, 0).unwrap();
    assert!(cell1.is_continuation);
}

#[test]
fn test_write_str() {
    let mut buf = FrameBuffer::new(20, 10);
    let written = buf.write_str(0, 0, "Hello", &Style::default());

    assert_eq!(written, 5);
    assert_eq!(buf.get(0, 0).unwrap().char, 'H');
    assert_eq!(buf.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_write_str_with_wide() {
    let mut buf = FrameBuffer::new(20, 10);
    let written = buf.write_str(0, 0, "Hi中文", &Style::default());

    // H(1) + i(1) + 中(2) + 文(2) = 6 columns
    assert_eq!(written, 6);
    assert_eq!(buf.get(0, 0).unwrap().char, 'H');
    assert_eq!(buf.get(1, 0).unwrap().char, 'i');
    assert_eq!(buf.get(2, 0).unwrap().char, '中');
    assert!(buf.get(3, 0).unwrap().is_continuation);
    assert_eq!(buf.get(4, 0).unwrap().char, '文');
    assert!(buf.get(5, 0).unwrap().is_continuation);
}

#[test]
fn test_fill_rect() {
    let mut buf = FrameBuffer::new(10, 10);
    let cell = Cell::from_char('#');
    buf.fill_rect(2, 2, 3, 3, &cell);

    assert_eq!(buf.get(2, 2), Some(&cell));
    assert_eq!(buf.get(4, 4), Some(&cell));
    assert_eq!(buf.get(1, 1), Some(&Cell::empty()));
    assert_eq!(buf.get(5, 5), Some(&Cell::empty()));
}

#[test]
fn test_clear() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.set(5, 5, Cell::from_char('x'));
    buf.clear();

    assert!(buf.get(5, 5).unwrap().is_empty());
}

#[test]
fn test_clear_rect() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.fill_rect(0, 0, 10, 10, &Cell::from_char('#'));
    buf.clear_rect(2, 2, 3, 3);

    assert_eq!(buf.get(0, 0).unwrap().char, '#');
    assert!(buf.get(3, 3).unwrap().is_empty());
}

#[test]
fn test_row() {
    let mut buf = FrameBuffer::new(5, 3);
    buf.write_str(0, 1, "Hello", &Style::default());

    let row = buf.row(1).unwrap();
    assert_eq!(row.len(), 5);
    assert_eq!(row[0].char, 'H');
    assert_eq!(row[4].char, 'o');

    assert!(buf.row(10).is_none());
}

#[test]
fn test_resize_grow() {
    let mut buf = FrameBuffer::new(5, 5);
    buf.set(2, 2, Cell::from_char('x'));
    buf.resize(10, 10);

    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 10);
    assert_eq!(buf.get(2, 2).unwrap().char, 'x');
    assert!(buf.get(9, 9).unwrap().is_empty());
}

#[test]
fn test_resize_shrink() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.set(2, 2, Cell::from_char('x'));
    buf.set(8, 8, Cell::from_char('y'));
    buf.resize(5, 5);

    assert_eq!(buf.width(), 5);
    assert_eq!(buf.height(), 5);
    assert_eq!(buf.get(2, 2).unwrap().char, 'x');
    assert!(buf.get(8, 8).is_none()); // Out of bounds now
}

#[test]
fn test_copy_from() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(10, 10);

    buf1.set(5, 5, Cell::from_char('x'));
    buf2.copy_from(&buf1);

    assert_eq!(buf2.get(5, 5).unwrap().char, 'x');
}

#[test]
fn test_swap_with() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(10, 10);

    buf1.set(0, 0, Cell::from_char('a'));
    buf2.set(0, 0, Cell::from_char('b'));

    buf1.swap_with(&mut buf2);

    assert_eq!(buf1.get(0, 0).unwrap().char, 'b');
    assert_eq!(buf2.get(0, 0).unwrap().char, 'a');
}

#[test]
fn test_default() {
    let buf = FrameBuffer::default();
    assert_eq!(buf.width(), 80);
    assert_eq!(buf.height(), 24);
}

// =========================================================================
// Coverage tests for uncovered lines
// =========================================================================

/// Test `resize` with same dimensions is a no-op (line 42).
#[test]
fn test_resize_same_dimensions_noop() {
    let mut buf = FrameBuffer::new(10, 5);
    buf.set(3, 3, Cell::from_char('x'));

    // Resize to same dimensions - should early return
    buf.resize(10, 5);

    // Content should be preserved (no reallocation)
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 5);
    assert_eq!(buf.get(3, 3).unwrap().char, 'x');
}

/// Test `apply_style` with bg and fg overlays (lines 132, 135).
#[test]
fn test_apply_style_with_bg_and_fg() {
    use reovim_arch::Color;

    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(0, 0, 'A', &Style::default());

    // Apply a style with both bg and fg set
    let overlay = Style::new().bg(Color::Blue).fg(Color::Yellow);
    buf.apply_style(0, 0, &overlay);

    let cell = buf.get(0, 0).unwrap();
    assert_eq!(cell.char, 'A'); // Character preserved
    assert_eq!(cell.style.bg, Some(Color::Blue));
    assert_eq!(cell.style.fg, Some(Color::Yellow));
}

/// Test `get_mut` returns `None` for out-of-bounds coordinates (line 96).
#[test]
fn test_get_mut_out_of_bounds() {
    let mut buf = FrameBuffer::new(10, 10);
    assert!(buf.get_mut(10, 0).is_none());
    assert!(buf.get_mut(0, 10).is_none());
    assert!(buf.get_mut(100, 100).is_none());
}

/// Test `apply_style` with `underline_color` (line 139).
#[test]
fn test_apply_style_underline_color() {
    use reovim_arch::Color;

    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(0, 0, 'A', &Style::default());

    // Apply a style with underline_color set
    let overlay = Style::new().underline_color(Color::Red);
    buf.apply_style(0, 0, &overlay);

    let cell = buf.get(0, 0).unwrap();
    assert_eq!(cell.char, 'A'); // Character preserved
    assert_eq!(cell.style.underline_color, Some(Color::Red));
}

/// Test `apply_style` on out-of-bounds position (line 141 - else branch).
#[test]
fn test_apply_style_out_of_bounds() {
    let mut buf = FrameBuffer::new(5, 5);
    // This should be a no-op (no panic) since (10, 10) is out of bounds
    buf.apply_style(10, 10, &Style::default());
}

/// Test `write_str` truncation when string exceeds buffer width (line 152).
#[test]
fn test_write_str_truncation() {
    let mut buf = FrameBuffer::new(5, 1);
    let written = buf.write_str(0, 0, "Hello World", &Style::default());

    // Only first 5 chars should fit in a 5-wide buffer
    assert_eq!(written, 5);
    assert_eq!(buf.get(0, 0).unwrap().char, 'H');
    assert_eq!(buf.get(4, 0).unwrap().char, 'o');
}

/// Test `write_str` with wide char at buffer edge (lines 160-162).
/// When a wide char would go past the edge, a space is written instead.
#[test]
fn test_write_str_wide_char_at_edge() {
    // Buffer is 5 columns wide
    let mut buf = FrameBuffer::new(5, 1);

    // "ABCD" takes 4 columns, then '中' needs 2 columns but only 1 remains
    let written = buf.write_str(0, 0, "ABCD中", &Style::default());

    // Should write "ABCD" (4 cols) + space (1 col) = 5 columns
    assert_eq!(written, 5);
    assert_eq!(buf.get(0, 0).unwrap().char, 'A');
    assert_eq!(buf.get(3, 0).unwrap().char, 'D');
    // Position 4 should be a space (wide char didn't fit)
    assert_eq!(buf.get(4, 0).unwrap().char, ' ');
}

// MC/DC tests for && conditions

#[test]
fn test_resize_same_width_different_height() {
    // width == self.width (true) && height == self.height (false)
    let mut buf = FrameBuffer::new(10, 5);
    buf.set(3, 3, Cell::from_char('x'));
    buf.resize(10, 8);
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 8);
    assert_eq!(buf.get(3, 3).unwrap().char, 'x');
}

#[test]
fn test_resize_different_width_same_height() {
    // width == self.width (false) && height == self.height (true)
    let mut buf = FrameBuffer::new(10, 5);
    buf.set(3, 3, Cell::from_char('x'));
    buf.resize(8, 5);
    assert_eq!(buf.width(), 8);
    assert_eq!(buf.height(), 5);
    assert_eq!(buf.get(3, 3).unwrap().char, 'x');
}

#[test]
fn test_get_x_in_bounds_y_out_of_bounds() {
    // x < self.width (true) && y < self.height (false)
    let buf = FrameBuffer::new(10, 5);
    assert!(buf.get(5, 10).is_none());
}

#[test]
fn test_get_x_out_of_bounds_y_in_bounds() {
    // x < self.width (false) && y < self.height (true)
    let buf = FrameBuffer::new(10, 5);
    assert!(buf.get(15, 2).is_none());
}

#[test]
fn test_set_x_in_bounds_y_out_of_bounds() {
    // set: x < width (true) && y < height (false)
    let mut buf = FrameBuffer::new(10, 5);
    buf.set(5, 10, Cell::from_char('x')); // no-op, no panic
}

#[test]
fn test_set_x_out_of_bounds_y_in_bounds() {
    // set: x < width (false) && y < height (not evaluated)
    let mut buf = FrameBuffer::new(10, 5);
    buf.set(15, 2, Cell::from_char('x')); // no-op, no panic
}

#[test]
fn test_put_char_wide_at_last_column() {
    // width == 2 (true) && x + 1 < self.width (false -> x+1 == width)
    let mut buf = FrameBuffer::new(5, 1);
    buf.put_char(4, 0, '中', &Style::default());
    // Wide char at position 4 in a 5-wide buffer: x+1=5 >= 5, no continuation
    let cell = buf.get(4, 0).unwrap();
    assert_eq!(cell.char, '中');
}

#[test]
fn test_copy_from_different_sizes() {
    // copy_from uses get() which returns Some for in-bounds
    let mut dst = FrameBuffer::new(3, 3);
    let mut src = FrameBuffer::new(5, 5);
    src.set(1, 1, Cell::from_char('x'));
    dst.copy_from(&src);
    assert_eq!(dst.get(1, 1).unwrap().char, 'x');
}
