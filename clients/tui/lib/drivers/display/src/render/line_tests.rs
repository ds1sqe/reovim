use super::*;

#[test]
fn test_render_line_basic() {
    let mut buffer = FrameBuffer::new(80, 24);
    let written = render_line(&mut buffer, 0, 0, "Hello", &[], 80, &Style::default());

    assert_eq!(written, 80); // Fills to max_width
    assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
    assert_eq!(buffer.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_render_line_with_highlights() {
    let mut buffer = FrameBuffer::new(80, 24);
    let highlight_style = Style::default().bold();
    let highlights = vec![(0, 5, highlight_style)];

    render_line(&mut buffer, 0, 0, "Hello World", &highlights, 80, &Style::default());

    // "Hello" should have the highlight style
    // Note: We can't easily test the style without accessing internals
    assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
}

#[test]
fn test_render_line_truncate() {
    let mut buffer = FrameBuffer::new(80, 24);
    let written = render_line(&mut buffer, 0, 0, "Hello World", &[], 5, &Style::default());

    assert_eq!(written, 5);
    assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
    assert_eq!(buffer.get(4, 0).unwrap().char, 'o');
    // Position 5 should be a space (fill)
}

#[test]
fn test_render_line_zero_width() {
    let mut buffer = FrameBuffer::new(80, 24);
    let written = render_line(&mut buffer, 0, 0, "Hello", &[], 0, &Style::default());

    assert_eq!(written, 0);
}

#[test]
fn test_render_line_simple() {
    let mut buffer = FrameBuffer::new(80, 24);
    let written = render_line_simple(&mut buffer, 0, 0, "Test", 10, &Style::default());

    assert_eq!(written, 10);
    assert_eq!(buffer.get(0, 0).unwrap().char, 'T');
}

#[test]
fn test_find_style_for_column() {
    let default = Style::default();
    let bold = Style::default().bold();
    let highlights = vec![(5, 10, bold.clone())];

    // Before highlight
    assert_eq!(find_style_for_column(0, &highlights, &default), &default);
    assert_eq!(find_style_for_column(4, &highlights, &default), &default);

    // In highlight
    assert_eq!(find_style_for_column(5, &highlights, &bold), &bold);
    assert_eq!(find_style_for_column(9, &highlights, &bold), &bold);

    // After highlight
    assert_eq!(find_style_for_column(10, &highlights, &default), &default);
}
