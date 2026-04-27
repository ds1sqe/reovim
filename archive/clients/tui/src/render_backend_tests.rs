use super::*;

#[test]
fn test_screen_backend() {
    let mut screen = ScreenBackend::new(80, 24);
    let style = Style::default();

    // Test set_cell
    screen.set_cell(0, 0, 'H', &style);
    screen.set_cell(1, 0, 'i', &style);

    // Test size
    assert_eq!(screen.size(), (80, 24));
}

#[test]
fn test_screen_backend_new() {
    let screen = ScreenBackend::new(100, 50);
    assert_eq!(screen.width(), 100);
    assert_eq!(screen.height(), 50);
}

#[test]
fn test_screen_backend_resize() {
    let mut screen = ScreenBackend::new(80, 24);
    screen.resize(120, 40);
    assert_eq!(screen.size(), (120, 40));
}

#[test]
fn test_framebuffer_backend() {
    let mut fb = FrameBuffer::new(80, 24);
    let style = Style::default();

    // Test set_cell via trait
    RenderBackend::set_cell(&mut fb, 0, 0, 'H', &style);
    RenderBackend::set_cell(&mut fb, 1, 0, 'i', &style);

    // Verify
    assert_eq!(fb.get(0, 0).map(|c| c.char), Some('H'));
    assert_eq!(fb.get(1, 0).map(|c| c.char), Some('i'));

    // Test size
    assert_eq!(RenderBackend::size(&fb), (80, 24));
}

#[test]
fn test_fill_region() {
    let mut fb = FrameBuffer::new(10, 10);
    let style = Style::default();

    fb.fill_region(2, 2, 3, 3, '#', &style);

    // Check corners of filled region
    assert_eq!(fb.get(2, 2).map(|c| c.char), Some('#'));
    assert_eq!(fb.get(4, 4).map(|c| c.char), Some('#'));

    // Check outside region
    assert_eq!(fb.get(1, 1).map(|c| c.char), Some(' '));
    assert_eq!(fb.get(5, 5).map(|c| c.char), Some(' '));
}

#[test]
fn test_write_str() {
    let mut fb = FrameBuffer::new(80, 24);
    let style = Style::default();

    let written = RenderBackend::write_str(&mut fb, 0, 0, "Hello", &style);
    assert_eq!(written, 5);
    assert_eq!(fb.get(0, 0).map(|c| c.char), Some('H'));
    assert_eq!(fb.get(4, 0).map(|c| c.char), Some('o'));
}

#[test]
fn test_format_frame_buffer_plain() {
    let fb = FrameBuffer::new(5, 1);
    let plain = format_frame_buffer(&fb, "plain_text");
    assert_eq!(plain, "     ");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_format_frame_buffer_ansi_empty() {
    let fb = FrameBuffer::new(3, 1);
    let ansi = format_frame_buffer(&fb, "ansi");
    // Empty cells with default style — may include ANSI codes
    // but must contain the 3 space characters
    assert!(ansi.contains("   ") || ansi.matches(' ').count() >= 3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_format_frame_buffer_rawansi_alias() {
    let fb = FrameBuffer::new(3, 1);
    let raw = format_frame_buffer(&fb, "raw_ansi");
    let raw2 = format_frame_buffer(&fb, "rawansi");
    // Both raw_ansi aliases should produce the same output
    assert_eq!(raw, raw2);
}

#[test]
fn test_format_frame_buffer_unknown_format_is_plain() {
    let mut fb = FrameBuffer::new(5, 1);
    let style = Style::default();
    RenderBackend::set_cell(&mut fb, 0, 0, 'X', &style);

    let plain = format_frame_buffer(&fb, "unknown_format");
    // Unknown format falls back to plain text
    assert!(plain.contains('X'));
    // Should NOT contain ANSI escapes
    assert!(!plain.contains("\x1b["));
}

#[test]
fn test_format_frame_buffer_case_insensitive() {
    let fb = FrameBuffer::new(3, 1);
    let upper = format_frame_buffer(&fb, "ANSI");
    let lower = format_frame_buffer(&fb, "ansi");
    assert_eq!(upper, lower);
}

#[test]
fn test_framebuffer_clear() {
    let mut fb = FrameBuffer::new(5, 5);
    let style = Style::default();
    RenderBackend::set_cell(&mut fb, 2, 2, 'X', &style);
    assert_eq!(fb.get(2, 2).map(|c| c.char), Some('X'));

    RenderBackend::clear(&mut fb);
    assert_eq!(fb.get(2, 2).map(|c| c.char), Some(' '));
}

#[test]
fn test_framebuffer_apply_style() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    RenderBackend::set_cell(&mut fb, 3, 2, 'A', &style);

    let new_style = Style::default().fg(reovim_arch::Color::Red);
    RenderBackend::apply_style(&mut fb, 3, 2, &new_style);

    let cell = fb.get(3, 2).unwrap();
    assert_eq!(cell.char, 'A'); // Character preserved
}

#[test]
fn test_framebuffer_overlay_bg() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    RenderBackend::set_cell(&mut fb, 1, 1, 'B', &style);

    RenderBackend::overlay_bg(&mut fb, 1, 1, reovim_arch::Color::Blue);
    let cell = fb.get(1, 1).unwrap();
    assert_eq!(cell.char, 'B'); // Character preserved
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
}

#[test]
fn test_framebuffer_overlay_bg_out_of_bounds() {
    let mut fb = FrameBuffer::new(5, 5);
    // Should not panic on out-of-bounds
    RenderBackend::overlay_bg(&mut fb, 100, 100, reovim_arch::Color::Red);
}

#[test]
fn test_fill_horizontal() {
    let mut fb = FrameBuffer::new(20, 5);
    let style = Style::default();
    fb.fill_horizontal(5, 2, 10, '-', &style);

    for col in 5..15 {
        assert_eq!(fb.get(col, 2).map(|c| c.char), Some('-'));
    }
    // Before and after should be space
    assert_eq!(fb.get(4, 2).map(|c| c.char), Some(' '));
    assert_eq!(fb.get(15, 2).map(|c| c.char), Some(' '));
}

#[test]
fn test_fill_vertical() {
    let mut fb = FrameBuffer::new(10, 10);
    let style = Style::default();
    fb.fill_vertical(3, 1, 5, '|', &style);

    for row in 1..6 {
        assert_eq!(fb.get(3, row).map(|c| c.char), Some('|'));
    }
    // Before and after should be space
    assert_eq!(fb.get(3, 0).map(|c| c.char), Some(' '));
    assert_eq!(fb.get(3, 6).map(|c| c.char), Some(' '));
}

#[test]
fn test_fill_region_clamped_to_bounds() {
    let mut fb = FrameBuffer::new(5, 5);
    let style = Style::default();
    // Fill region that extends beyond bounds
    fb.fill_region(3, 3, 10, 10, '#', &style);

    // Should fill 3..5 x 3..5 (clamped to bounds)
    assert_eq!(fb.get(3, 3).map(|c| c.char), Some('#'));
    assert_eq!(fb.get(4, 4).map(|c| c.char), Some('#'));
    // Outside bounds should be untouched
    assert_eq!(fb.get(2, 2).map(|c| c.char), Some(' '));
}

#[test]
fn test_screen_backend_write_str() {
    let mut screen = ScreenBackend::new(80, 24);
    let style = Style::default();

    let written = RenderBackend::write_str(&mut screen, 0, 0, "Hello", &style);
    assert_eq!(written, 5);
}

#[test]
fn test_screen_backend_clear() {
    let mut screen = ScreenBackend::new(10, 10);
    let style = Style::default();
    screen.set_cell(0, 0, 'X', &style);

    RenderBackend::clear(&mut screen);
    // After clear, size should be preserved
    assert_eq!(screen.size(), (10, 10));
}

#[test]
fn test_to_tui_style_attributes() {
    use reovim_driver_display::Attributes as DisplayAttrs;

    let mut style = Style::default();
    style.attributes.set(DisplayAttrs::BOLD);
    style.attributes.set(DisplayAttrs::ITALIC);
    style.attributes.set(DisplayAttrs::UNDERLINE);

    let tui_style = to_tui_style(&style);
    assert!(tui_style.attrs.contains(TuiAttrs::BOLD));
    assert!(tui_style.attrs.contains(TuiAttrs::ITALIC));
    assert!(tui_style.attrs.contains(TuiAttrs::UNDERLINE));
}

#[test]
fn test_to_tui_style_more_attributes() {
    use reovim_driver_display::Attributes as DisplayAttrs;

    let mut style = Style::default();
    style.attributes.set(DisplayAttrs::STRIKETHROUGH);
    style.attributes.set(DisplayAttrs::REVERSE);
    style.attributes.set(DisplayAttrs::DIM);

    let tui_style = to_tui_style(&style);
    assert!(tui_style.attrs.contains(TuiAttrs::STRIKETHROUGH));
    assert!(tui_style.attrs.contains(TuiAttrs::REVERSE));
    assert!(tui_style.attrs.contains(TuiAttrs::DIM));
}

#[test]
fn test_to_tui_style_colors() {
    let style = Style {
        fg: Some(reovim_arch::Color::Red),
        bg: Some(reovim_arch::Color::Blue),
        ..Style::default()
    };

    let tui_style = to_tui_style(&style);
    assert_eq!(tui_style.fg, Some(reovim_arch::Color::Red));
    assert_eq!(tui_style.bg, Some(reovim_arch::Color::Blue));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_format_frame_buffer_multiline() {
    let mut fb = FrameBuffer::new(5, 3);
    let style = Style::default();
    for row in 0..3 {
        for col in 0..5 {
            RenderBackend::set_cell(&mut fb, col, row, 'a', &style);
        }
    }

    let plain = format_frame_buffer(&fb, "plain_text");
    let lines: Vec<&str> = plain.lines().collect();
    assert_eq!(lines.len(), 3);
    for line in &lines {
        assert!(line.contains("aaaaa"));
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_format_frame_buffer_ansi_with_styled_content() {
    let mut fb = FrameBuffer::new(3, 1);
    let style = Style::default().fg(reovim_arch::Color::Green);
    RenderBackend::set_cell(&mut fb, 0, 0, 'G', &style);
    RenderBackend::set_cell(&mut fb, 1, 0, 'o', &style);
    RenderBackend::set_cell(&mut fb, 2, 0, '!', &style);

    let ansi = format_frame_buffer(&fb, "ansi");
    // Should contain ANSI escape codes
    assert!(ansi.contains("\x1b["));
    assert!(ansi.contains("\x1b[0m")); // Reset code
}

#[test]
fn test_screen_backend_invalidate() {
    let mut screen = ScreenBackend::new(80, 24);
    // Should not panic
    screen.invalidate();
}
