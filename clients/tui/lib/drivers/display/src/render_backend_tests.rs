use super::*;

// =========================================================================
// RenderBackend for FrameBuffer tests
// =========================================================================

#[test]
fn test_framebuffer_set_cell() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'H', &style);
    fb.set_cell(1, 0, 'i', &style);
    assert_eq!(fb.get(0, 0).unwrap().char, 'H');
    assert_eq!(fb.get(1, 0).unwrap().char, 'i');
}

#[test]
fn test_framebuffer_write_str() {
    let mut fb = FrameBuffer::new(20, 5);
    let style = Style::default();
    let cols = RenderBackend::write_str(&mut fb, 0, 0, "Hello", &style);
    assert_eq!(cols, 5);
    assert_eq!(fb.get(0, 0).unwrap().char, 'H');
    assert_eq!(fb.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_framebuffer_size() {
    let fb = FrameBuffer::new(80, 24);
    assert_eq!(RenderBackend::size(&fb), (80, 24));
}

#[test]
fn test_framebuffer_clear() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'X', &style);
    RenderBackend::clear(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_apply_style() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'A', &style);
    let highlight = Style::default().bg(Color::Yellow);
    RenderBackend::apply_style(&mut fb, 0, 0, &highlight);
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'A'); // Character preserved
    assert_eq!(cell.style.bg, Some(Color::Yellow));
}

#[test]
fn test_framebuffer_overlay_bg() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default().fg(Color::White);
    fb.set_cell(0, 0, 'B', &style);
    RenderBackend::overlay_bg(&mut fb, 0, 0, Color::Blue);
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'B');
    assert_eq!(cell.style.bg, Some(Color::Blue));
}

#[test]
fn test_framebuffer_fill_horizontal() {
    let mut fb = FrameBuffer::new(20, 5);
    let style = Style::default();
    RenderBackend::fill_horizontal(&mut fb, 2, 1, 5, '-', &style);
    assert_eq!(fb.get(2, 1).unwrap().char, '-');
    assert_eq!(fb.get(6, 1).unwrap().char, '-');
    assert_eq!(fb.get(7, 1).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_fill_vertical() {
    let mut fb = FrameBuffer::new(10, 10);
    let style = Style::default();
    RenderBackend::fill_vertical(&mut fb, 3, 1, 4, '|', &style);
    assert_eq!(fb.get(3, 1).unwrap().char, '|');
    assert_eq!(fb.get(3, 4).unwrap().char, '|');
    assert_eq!(fb.get(3, 5).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_fill_region() {
    let mut fb = FrameBuffer::new(20, 10);
    let style = Style::default();
    RenderBackend::fill_region(&mut fb, 1, 1, 3, 2, '#', &style);
    assert_eq!(fb.get(1, 1).unwrap().char, '#');
    assert_eq!(fb.get(3, 2).unwrap().char, '#');
    assert_eq!(fb.get(4, 1).unwrap().char, ' ');
}

