use super::*;

#[test]
fn test_new() {
    let buf = FrameBuffer::new(10, 5);
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 5);
}

#[test]
fn test_put_char() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(0, 0, 'a', &Style::default());
    assert_eq!(buf.get(0, 0).unwrap().char, 'a');
}

#[test]
fn test_write_str() {
    let mut buf = FrameBuffer::new(20, 10);
    buf.write_str(0, 0, "Hello", &Style::default());
    assert_eq!(buf.get(0, 0).unwrap().char, 'H');
    assert_eq!(buf.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_clear() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.set(5, 5, Cell::from_char('x'));
    buf.clear();
    assert!(buf.get(5, 5).unwrap().is_empty());
}

#[test]
fn test_resize_same_size() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(5, 5, 'X', &Style::default());
    buf.resize(10, 10);
    // Should preserve content
    assert_eq!(buf.get(5, 5).unwrap().char, 'X');
}

#[test]
fn test_resize_larger() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(5, 5, 'Y', &Style::default());
    buf.resize(20, 20);
    assert_eq!(buf.width(), 20);
    assert_eq!(buf.height(), 20);
    assert_eq!(buf.get(5, 5).unwrap().char, 'Y');
}

#[test]
fn test_resize_smaller() {
    let mut buf = FrameBuffer::new(20, 20);
    buf.put_char(5, 5, 'Z', &Style::default());
    buf.resize(10, 10);
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 10);
    assert_eq!(buf.get(5, 5).unwrap().char, 'Z');
}

#[test]
fn test_get_out_of_bounds() {
    let buf = FrameBuffer::new(10, 10);
    assert!(buf.get(100, 100).is_none());
    assert!(buf.get(10, 5).is_none());
    assert!(buf.get(5, 10).is_none());
}

#[test]
fn test_get_mut_out_of_bounds() {
    let mut buf = FrameBuffer::new(10, 10);
    assert!(buf.get_mut(100, 100).is_none());
}

#[test]
fn test_set_out_of_bounds() {
    let mut buf = FrameBuffer::new(10, 10);
    // Should not crash
    buf.set(100, 100, Cell::from_char('A'));
}

#[test]
fn test_put_char_wide() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(5, 5, '中', &Style::default());
    assert_eq!(buf.get(5, 5).unwrap().char, '中');
    assert_eq!(buf.get(5, 5).unwrap().width, 2);
    // Second column should be continuation
    assert!(buf.get(6, 5).unwrap().is_continuation);
}

#[test]
fn test_put_char_wide_at_edge() {
    let mut buf = FrameBuffer::new(10, 10);
    // Wide char at last column should not crash
    buf.put_char(9, 5, '中', &Style::default());
    assert_eq!(buf.get(9, 5).unwrap().char, '中');
}

#[test]
fn test_apply_style_fg() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(3, 3, 'C', &Style::default());
    let style = Style::new().with_fg(reovim_arch::Color::Green);
    buf.apply_style(3, 3, &style);

    let cell = buf.get(3, 3).unwrap();
    assert_eq!(cell.char, 'C');
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Green));
}

#[test]
fn test_apply_style_bg() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(2, 2, 'D', &Style::default());
    let style = Style::new().with_bg(reovim_arch::Color::Yellow);
    buf.apply_style(2, 2, &style);

    let cell = buf.get(2, 2).unwrap();
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Yellow));
}

#[test]
fn test_apply_style_out_of_bounds() {
    let mut buf = FrameBuffer::new(10, 10);
    let style = Style::new().with_bg(reovim_arch::Color::Red);
    // Should not crash
    buf.apply_style(100, 100, &style);
}

#[test]
fn test_write_str_returns_width() {
    let mut buf = FrameBuffer::new(20, 10);
    let width = buf.write_str(0, 0, "Hello", &Style::default());
    assert_eq!(width, 5);
}

#[test]
fn test_write_str_overflow() {
    let mut buf = FrameBuffer::new(5, 10);
    buf.write_str(0, 0, "HelloWorld", &Style::default());
    // Should only write "Hello"
    assert_eq!(buf.get(0, 0).unwrap().char, 'H');
    assert_eq!(buf.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_write_str_wide_chars() {
    let mut buf = FrameBuffer::new(20, 10);
    buf.write_str(0, 0, "中文", &Style::default());
    assert_eq!(buf.get(0, 0).unwrap().char, '中');
    assert!(buf.get(1, 0).unwrap().is_continuation);
    assert_eq!(buf.get(2, 0).unwrap().char, '文');
}

#[test]
fn test_write_str_wide_char_at_boundary() {
    let mut buf = FrameBuffer::new(5, 10);
    // Write wide char that would overflow
    buf.write_str(4, 0, "中", &Style::default());
    // Should replace with space
    assert_eq!(buf.get(4, 0).unwrap().char, ' ');
}

#[test]
fn test_fill_rect() {
    let mut buf = FrameBuffer::new(20, 20);
    let cell = Cell::new('*', Style::default());
    buf.fill_rect(5, 5, 3, 2, &cell);

    assert_eq!(buf.get(5, 5).unwrap().char, '*');
    assert_eq!(buf.get(7, 6).unwrap().char, '*');
    assert_eq!(buf.get(8, 5).unwrap().char, ' '); // Outside rect
}

#[test]
fn test_fill_rect_overflow() {
    let mut buf = FrameBuffer::new(10, 10);
    let cell = Cell::new('#', Style::default());
    // Should not crash, just clip
    buf.fill_rect(8, 8, 10, 10, &cell);
    assert_eq!(buf.get(8, 8).unwrap().char, '#');
}

#[test]
fn test_row_out_of_bounds() {
    let buf = FrameBuffer::new(10, 10);
    assert!(buf.row(100).is_none());
    assert!(buf.row(10).is_none());
}

#[test]
fn test_row_valid() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(5, 3, 'R', &Style::default());
    let row = buf.row(3).unwrap();
    assert_eq!(row.len(), 10);
    assert_eq!(row[5].char, 'R');
}

#[test]
fn test_copy_from_same_size() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(10, 10);
    buf1.put_char(5, 5, 'S', &Style::default());
    buf2.copy_from(&buf1);
    assert_eq!(buf2.get(5, 5).unwrap().char, 'S');
}

#[test]
fn test_copy_from_smaller() {
    let mut buf1 = FrameBuffer::new(5, 5);
    let mut buf2 = FrameBuffer::new(10, 10);
    buf1.put_char(3, 3, 'T', &Style::default());
    buf2.copy_from(&buf1);
    assert_eq!(buf2.get(3, 3).unwrap().char, 'T');
}

#[test]
fn test_copy_from_larger() {
    let mut buf1 = FrameBuffer::new(20, 20);
    let mut buf2 = FrameBuffer::new(10, 10);
    buf1.put_char(5, 5, 'U', &Style::default());
    buf1.put_char(15, 15, 'V', &Style::default());
    buf2.copy_from(&buf1);
    assert_eq!(buf2.get(5, 5).unwrap().char, 'U');
    // (15, 15) should not be copied (out of bounds)
    assert!(buf2.get(15, 15).is_none());
}

#[test]
fn test_swap_with() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(10, 10);
    buf1.put_char(1, 1, 'A', &Style::default());
    buf2.put_char(2, 2, 'B', &Style::default());

    buf1.swap_with(&mut buf2);

    assert_eq!(buf1.get(2, 2).unwrap().char, 'B');
    assert_eq!(buf2.get(1, 1).unwrap().char, 'A');
}

#[test]
#[should_panic(expected = "Buffer widths must match")]
fn test_swap_with_different_width() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(20, 10);
    buf1.swap_with(&mut buf2);
}

#[test]
#[should_panic(expected = "Buffer heights must match")]
fn test_swap_with_different_height() {
    let mut buf1 = FrameBuffer::new(10, 10);
    let mut buf2 = FrameBuffer::new(10, 20);
    buf1.swap_with(&mut buf2);
}

#[test]
fn test_default() {
    let buf = FrameBuffer::default();
    assert_eq!(buf.width(), 80);
    assert_eq!(buf.height(), 24);
}

#[test]
fn test_clone() {
    let mut buf = FrameBuffer::new(10, 10);
    buf.put_char(5, 5, 'W', &Style::default());
    let cloned = buf.clone();
    assert_eq!(cloned.get(5, 5).unwrap().char, 'W');
    assert_eq!(cloned.width(), buf.width());
    assert_eq!(cloned.height(), buf.height());
}

#[test]
fn test_debug() {
    let buf = FrameBuffer::new(10, 10);
    let debug = format!("{buf:?}");
    assert!(debug.contains("FrameBuffer"));
}
