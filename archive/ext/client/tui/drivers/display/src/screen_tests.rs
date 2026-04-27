use {
    super::*,
    crate::{WindowId, window::Rect},
};

#[test]
fn test_new() {
    let screen = Screen::new(80, 24);
    assert_eq!(screen.size(), (80, 24));
    assert_eq!(screen.width(), 80);
    assert_eq!(screen.height(), 24);
}

#[test]
fn test_resize() {
    let mut screen = Screen::new(80, 24);
    screen.resize(100, 50);
    assert_eq!(screen.size(), (100, 50));
}

#[test]
fn test_clear() {
    let mut screen = Screen::new(10, 10);
    screen.draw_str(0, 0, "Hello", &Style::default());
    screen.clear();

    // All cells should be empty
    let buf = screen.frame_buffer();
    assert!(buf.get(0, 0).unwrap().is_empty());
}

#[test]
fn test_draw_str() {
    let mut screen = Screen::new(80, 24);
    let written = screen.draw_str(0, 0, "Hello", &Style::default());
    assert_eq!(written, 5);
    assert_eq!(screen.frame_buffer().get(0, 0).unwrap().char, 'H');
}

#[test]
fn test_fill() {
    let mut screen = Screen::new(10, 10);
    let style = Style::default();
    screen.fill(&style);

    // All cells should be spaces with the style
    let buf = screen.frame_buffer();
    for y in 0..10 {
        for x in 0..10 {
            assert_eq!(buf.get(x, y).unwrap().char, ' ');
        }
    }
}

#[test]
fn test_content_area() {
    let screen = Screen::new(80, 24);
    let area = screen.content_area();
    assert_eq!(area.x, 0);
    assert_eq!(area.y, 0);
    assert_eq!(area.width, 80);
    assert_eq!(area.height, 24);
}

#[test]
fn test_flush() {
    let mut screen = Screen::new(5, 1);
    screen.draw_str(0, 0, "Test", &Style::default());

    let mut output = Vec::new();
    screen.flush(&mut output).unwrap();

    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.contains("Test"));
}

#[test]
fn test_enable_capture() {
    let mut screen = Screen::new(80, 24);
    let handle = screen.enable_capture();

    screen.draw_str(0, 0, "Captured", &Style::default());
    let mut output = Vec::new();
    screen.flush(&mut output).unwrap();

    let text = handle.to_plain_text().unwrap();
    assert!(text.starts_with("Captured"));
}

#[test]
fn test_render_views() {
    let mut screen = Screen::new(80, 24);
    let views = vec![
        WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 40, 24)),
        WindowView::new(WindowId::from_raw(2), Rect::new(40, 0, 40, 24)),
    ];

    screen.render_views(&views, &Style::default());

    // Check that window borders were drawn
    let buf = screen.frame_buffer();
    assert_eq!(buf.get(0, 0).unwrap().char, '┌');
    assert_eq!(buf.get(40, 0).unwrap().char, '┌');
}

#[test]
fn test_default() {
    let screen = Screen::default();
    assert_eq!(screen.size(), (80, 24));
}

// =========================================================================
// Coverage tests for uncovered lines
// =========================================================================

/// Test `frame_buffer_mut` returns mutable back buffer (lines 99-101).
#[test]
fn test_frame_buffer_mut() {
    let mut screen = Screen::new(10, 10);
    let buf = screen.frame_buffer_mut();
    buf.set(0, 0, crate::frame::Cell::from_char('X'));
    // Verify through the immutable accessor
    assert_eq!(screen.frame_buffer().get(0, 0).unwrap().char, 'X');
}

/// Test `clear_rect` clears a rectangular region (lines 115-117).
#[test]
fn test_clear_rect() {
    let mut screen = Screen::new(10, 10);
    // Fill the screen with non-empty cells
    screen.fill(&Style::default());
    screen.draw_str(2, 2, "Hello", &Style::default());

    // Clear a rect over that region
    screen.clear_rect(2, 2, 5, 1);

    // The cleared cells should be empty
    let buf = screen.frame_buffer();
    assert!(buf.get(2, 2).unwrap().is_empty());
    assert!(buf.get(3, 2).unwrap().is_empty());
}

/// Test `disable_capture` (lines 128-130).
#[test]
fn test_disable_capture() {
    let mut screen = Screen::new(80, 24);
    let _handle = screen.enable_capture();
    screen.disable_capture();

    // After disabling capture, flush should still work without panicking
    screen.draw_str(0, 0, "Test", &Style::default());
    let mut output = Vec::new();
    screen.flush(&mut output).unwrap();
}

/// Test `render_views` with window too narrow for ID text (line 216).
#[test]
fn test_render_views_small_window() {
    let mut screen = Screen::new(80, 24);

    // Create a window too narrow for the "Win N" text indicator
    let views = vec![WindowView::new(
        WindowId::from_raw(999),
        Rect::new(0, 0, 4, 4),
    )];

    // Should not panic even if window is too narrow for the text
    screen.render_views(&views, &Style::default());

    // Check that borders were still drawn
    let buf = screen.frame_buffer();
    assert_eq!(buf.get(0, 0).unwrap().char, '┌');
}

/// Test `render_views` with one zero dimension (width=0, height>0).
#[test]
fn test_render_views_zero_width_nonzero_height() {
    let mut screen = Screen::new(80, 24);

    let views = vec![WindowView::new(
        WindowId::from_raw(0),
        Rect::new(0, 0, 0, 10),
    )];

    screen.render_views(&views, &Style::default());

    // Zero-width window should not draw anything
    let buf = screen.frame_buffer();
    assert_eq!(buf.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_views_nonzero_width_zero_height() {
    // Line 191: b.width > 0 (true) && b.height > 0 (false)
    let mut screen = Screen::new(80, 24);

    let views = vec![WindowView::new(
        WindowId::from_raw(0),
        Rect::new(0, 0, 10, 0),
    )];

    screen.render_views(&views, &Style::default());

    // Width>0 but height=0: should not draw anything
    let buf = screen.frame_buffer();
    assert_eq!(buf.get(0, 0).unwrap().char, ' ');
}

/// Test `render_views` with zero-size window (line 216 else branch).
#[test]
fn test_render_views_zero_size_window() {
    let mut screen = Screen::new(80, 24);

    // Create a window with zero dimensions
    let views = vec![WindowView::new(
        WindowId::from_raw(0),
        Rect::new(0, 0, 0, 0),
    )];

    screen.render_views(&views, &Style::default());

    // Zero-size window should not draw anything (cells remain as fill)
    let buf = screen.frame_buffer();
    assert_eq!(buf.get(0, 0).unwrap().char, ' ');
}
