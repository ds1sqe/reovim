use super::*;

#[test]
fn test_render_state_default() {
    let state = RenderState::default();
    assert_eq!(state.width, 0);
    assert_eq!(state.height, 0);
    assert!(state.mode_display.is_none());
    assert_eq!(state.cursor_line, 0);
    assert_eq!(state.cursor_column, 0);
    assert!(state.modules.is_empty());
}

#[test]
fn test_render_state_set_size() {
    let mut state = RenderState::new();
    state.set_size(80, 24);
    assert_eq!(state.width, 80);
    assert_eq!(state.height, 24);
}

#[test]
fn test_render_state_set_mode() {
    let mut state = RenderState::new();
    state.set_mode(Some("NORMAL".to_string()));
    assert_eq!(state.mode_display, Some("NORMAL".to_string()));
}

#[test]
fn test_render_state_set_cursor() {
    let mut state = RenderState::new();
    state.set_cursor(10, 5);
    assert_eq!(state.cursor_line, 10);
    assert_eq!(state.cursor_column, 5);
}

#[test]
fn test_build_frame_content_empty_size() {
    let state = RenderState::default();
    let buffer = FrameBuffer::new(0, 0);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);
    assert!(content.is_empty());
}

#[test]
fn test_build_frame_content_has_header() {
    let mut state = RenderState::new();
    state.set_size(80, 24);
    state.set_mode(Some("NORMAL".to_string()));
    state.server_address = "127.0.0.1:12521".to_string();

    let buffer = FrameBuffer::new(80, 24);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);

    assert!(content.contains("=== FRAME CAPTURE ==="));
    assert!(content.contains("screen_size: 80x24"));
    assert!(content.contains("mode: NORMAL"));
    assert!(content.contains("server: 127.0.0.1:12521"));
    assert!(content.contains("=== END FRAME ==="));
}

#[test]
fn test_build_frame_content_has_line_numbers() {
    let mut state = RenderState::new();
    state.set_size(80, 5);

    let buffer = FrameBuffer::new(80, 5);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);

    assert!(content.contains("[1]"));
    assert!(content.contains("[5]"));
}

#[test]
fn test_build_frame_content_zero_width() {
    let mut state = RenderState::new();
    state.set_size(0, 24);

    let buffer = FrameBuffer::new(0, 24);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);
    assert!(content.is_empty());
}

#[test]
fn test_build_frame_content_zero_height() {
    let mut state = RenderState::new();
    state.set_size(80, 0);

    let buffer = FrameBuffer::new(80, 0);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);
    assert!(content.is_empty());
}

#[test]
fn test_build_frame_content_raw_ansi_format() {
    let mut state = RenderState::new();
    state.set_size(10, 2);
    state.set_mode(Some("NORMAL".to_string()));
    state.server_address = "localhost:50051".to_string();

    let buffer = FrameBuffer::new(10, 2);
    let content = build_frame_content(&state, &buffer, ScreenFormat::RawAnsi);

    assert!(content.contains("=== FRAME CAPTURE ==="));
    assert!(content.contains("=== SCREEN CONTENT"));
    assert!(content.contains("=== END FRAME ==="));
    assert!(content.contains("[1]"));
    assert!(content.contains("[2]"));
}

#[test]
fn test_build_frame_content_cell_grid_format_fallback() {
    let mut state = RenderState::new();
    state.set_size(10, 2);

    let buffer = FrameBuffer::new(10, 2);
    // CellGrid falls back to plain text
    let content = build_frame_content(&state, &buffer, ScreenFormat::CellGrid);
    assert!(content.contains("[1]"));
}

#[test]
fn test_build_frame_content_metadata() {
    let mut state = RenderState::new();
    state.set_size(40, 5);
    state.set_mode(None);
    state.server_address = "10.0.0.1:9999".to_string();
    state.modules = vec!["vim".to_string(), "motions".to_string()];
    state.log_panel_visible = true;
    state.set_cursor(10, 5);

    let buffer = FrameBuffer::new(40, 5);
    let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);

    assert!(content.contains("mode: UNKNOWN"));
    assert!(content.contains("server: 10.0.0.1:9999"));
    assert!(content.contains("modules: 2"));
    assert!(content.contains("log_panel_visible: true"));
    assert!(content.contains("cursor: line=10, col=5"));
}

#[test]
fn test_render_state_clone() {
    let mut state = RenderState::new();
    state.set_size(80, 24);
    state.set_mode(Some("INSERT".to_string()));
    state.set_cursor(5, 10);
    state.server_address = "addr".to_string();

    let cloned = state.clone();
    assert_eq!(cloned.width, 80);
    assert_eq!(cloned.height, 24);
    assert_eq!(cloned.mode_display, Some("INSERT".to_string()));
    assert_eq!(cloned.cursor_line, 5);
    assert_eq!(cloned.cursor_column, 10);
}

#[test]
fn test_render_state_debug() {
    let state = RenderState::new();
    let debug = format!("{state:?}");
    assert!(debug.contains("RenderState"));
}

#[test]
fn test_parse_cell_style_empty() {
    let cell = serde_json::json!({"char": "a"});
    let style = parse_cell_style(&cell);
    assert!(style.fg.is_none());
    assert!(style.bg.is_none());
}

#[test]
fn test_parse_cell_style_with_colors() {
    let cell = serde_json::json!({"char": "a", "fg": "#ff0000", "bg": "#0000ff"});
    let style = parse_cell_style(&cell);
    assert!(style.fg.is_some());
    assert!(style.bg.is_some());
}

#[test]
fn test_write_cell_grid_to_buffer_valid_json() {
    let mut buffer = FrameBuffer::new(3, 2);
    let grid = serde_json::json!([
        [{"char": "H"}, {"char": "i"}, {"char": "!"}],
        [{"char": "x"}, {"char": "y"}, {"char": "z"}]
    ]);
    let content = serde_json::to_string(&grid).unwrap();

    write_cell_grid_to_buffer(&mut buffer, &content, 3, 2);
    assert_eq!(buffer.get(0, 0).map(|c| c.char), Some('H'));
    assert_eq!(buffer.get(1, 0).map(|c| c.char), Some('i'));
    assert_eq!(buffer.get(2, 0).map(|c| c.char), Some('!'));
    assert_eq!(buffer.get(0, 1).map(|c| c.char), Some('x'));
}

#[test]
fn test_write_cell_grid_to_buffer_invalid_json_fallback() {
    let mut buffer = FrameBuffer::new(10, 2);
    let content = "hello\nworld";

    write_cell_grid_to_buffer(&mut buffer, content, 10, 2);
    assert_eq!(buffer.get(0, 0).map(|c| c.char), Some('h'));
    assert_eq!(buffer.get(4, 0).map(|c| c.char), Some('o'));
    assert_eq!(buffer.get(0, 1).map(|c| c.char), Some('w'));
}

#[test]
fn test_write_cell_grid_to_buffer_missing_char_defaults_to_space() {
    let mut buffer = FrameBuffer::new(2, 1);
    let grid = serde_json::json!([
        [{"fg": "#ff0000"}, {"char": "a"}]
    ]);
    let content = serde_json::to_string(&grid).unwrap();

    write_cell_grid_to_buffer(&mut buffer, &content, 2, 1);
    assert_eq!(buffer.get(0, 0).map(|c| c.char), Some(' ')); // Missing char defaults to space
    assert_eq!(buffer.get(1, 0).map(|c| c.char), Some('a'));
}

#[test]
fn test_build_frame_content_ansi_with_styled_cells() {
    let mut state = RenderState::new();
    state.set_size(5, 1);

    let mut buffer = FrameBuffer::new(5, 1);
    let style = Style::default().fg(reovim_arch::Color::Red);
    buffer.put_char(0, 0, 'R', &style);
    buffer.put_char(1, 0, 'e', &style);
    buffer.put_char(2, 0, 'd', &style);

    let content = build_frame_content(&state, &buffer, ScreenFormat::RawAnsi);
    assert!(content.contains("[1]"));
    // Should contain ANSI escape codes for red foreground
    assert!(content.contains("\x1b["));
}
