use super::*;

#[test]
fn test_new() {
    let renderer = FrameRenderer::new(80, 24);
    assert_eq!(renderer.dimensions(), (80, 24));
}

#[test]
fn test_resize() {
    let mut renderer = FrameRenderer::new(80, 24);
    renderer.resize(100, 50);
    assert_eq!(renderer.dimensions(), (100, 50));
}

#[test]
fn test_flush() {
    let mut renderer = FrameRenderer::new(5, 1);
    renderer
        .buffer_mut()
        .write_str(0, 0, "Hello", &Style::default());

    let mut output = Vec::new();
    renderer.flush(&mut output).unwrap();

    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.contains("Hello"));
}

#[test]
fn test_default() {
    let renderer = FrameRenderer::default();
    assert_eq!(renderer.dimensions(), (80, 24));
}

#[test]
fn test_buffer_read_only() {
    let renderer = FrameRenderer::new(10, 5);
    let buf = renderer.buffer();
    assert_eq!(buf.width(), 10);
    assert_eq!(buf.height(), 5);
}

#[test]
fn test_flush_twice_diff_rendering() {
    let mut renderer = FrameRenderer::new(5, 1);

    // First flush: full render (not initialized)
    renderer
        .buffer_mut()
        .write_str(0, 0, "Hello", &Style::default());
    let mut output1 = Vec::new();
    renderer.flush(&mut output1).unwrap();
    let out1 = String::from_utf8_lossy(&output1);
    assert!(out1.contains("Hello"));

    // Second flush: same content, should be minimal
    renderer
        .buffer_mut()
        .write_str(0, 0, "Hello", &Style::default());
    let mut output2 = Vec::new();
    renderer.flush(&mut output2).unwrap();
    // Second output should be smaller (diff detects no changes)
    assert!(output2.len() <= output1.len());
}

#[test]
fn test_flush_with_style_changes() {
    let mut renderer = FrameRenderer::new(10, 1);
    let styled = Style::new().with_fg(reovim_arch::Color::Red);
    renderer.buffer_mut().write_str(0, 0, "Red", &styled);
    renderer
        .buffer_mut()
        .write_str(3, 0, "Def", &Style::default());

    let mut output = Vec::new();
    renderer.flush(&mut output).unwrap();
    let out = String::from_utf8_lossy(&output);
    // Should contain both text segments and style commands
    assert!(out.contains("Red"));
    assert!(out.contains("Def"));
    // Should contain reset style between different styles
    assert!(out.contains("\x1b[0m"));
}

#[test]
fn test_flush_only_changed_cells() {
    let mut renderer = FrameRenderer::new(10, 1);

    // First flush: write "Hello     "
    renderer
        .buffer_mut()
        .write_str(0, 0, "Hello", &Style::default());
    let mut out1 = Vec::new();
    renderer.flush(&mut out1).unwrap();

    // Second flush: change only first char
    renderer
        .buffer_mut()
        .write_str(0, 0, "Jello", &Style::default());
    let mut out2 = Vec::new();
    renderer.flush(&mut out2).unwrap();

    let out2_str = String::from_utf8_lossy(&out2);
    // Should contain "Jello" (diff will detect first character changed,
    // but batching may include subsequent unchanged chars on same row)
    assert!(out2_str.contains('J'));
}

#[test]
fn test_resize_clears_initialized() {
    let mut renderer = FrameRenderer::new(5, 1);

    // First flush sets initialized = true
    renderer
        .buffer_mut()
        .write_str(0, 0, "Hello", &Style::default());
    let mut out = Vec::new();
    renderer.flush(&mut out).unwrap();

    // Resize resets initialized
    renderer.resize(10, 2);
    assert_eq!(renderer.dimensions(), (10, 2));

    // Next flush should do full render
    renderer
        .buffer_mut()
        .write_str(0, 0, "World", &Style::default());
    let mut out2 = Vec::new();
    renderer.flush(&mut out2).unwrap();
    let out2_str = String::from_utf8_lossy(&out2);
    assert!(out2_str.contains("World"));
}

#[test]
fn test_flush_multi_row() {
    let mut renderer = FrameRenderer::new(5, 3);
    renderer
        .buffer_mut()
        .write_str(0, 0, "Row0", &Style::default());
    renderer
        .buffer_mut()
        .write_str(0, 1, "Row1", &Style::default());
    renderer
        .buffer_mut()
        .write_str(0, 2, "Row2", &Style::default());

    let mut output = Vec::new();
    renderer.flush(&mut output).unwrap();
    let out = String::from_utf8_lossy(&output);
    assert!(out.contains("Row0"));
    assert!(out.contains("Row1"));
    assert!(out.contains("Row2"));
}

#[test]
fn test_flush_styled_content() {
    let mut renderer = FrameRenderer::new(5, 1);
    let bold_style = Style::new().bold();
    renderer.buffer_mut().write_str(0, 0, "Bold", &bold_style);

    let mut output = Vec::new();
    renderer.flush(&mut output).unwrap();
    let out = String::from_utf8_lossy(&output);
    assert!(out.contains("Bold"));
    // Should have style set command
    assert!(out.contains("\x1b["));
}

#[test]
fn test_style_to_ansi_with_empty_style() {
    let style = Style::default();
    let ansi = style_to_ansi(&style);
    // Empty style should produce reset
    assert_eq!(ansi, "\x1b[0m");
}

#[test]
fn test_style_to_ansi_with_color() {
    let style = Style::new().with_fg(reovim_arch::Color::Green);
    let ansi = style_to_ansi(&style);
    // Should have actual color codes, not reset
    assert_ne!(ansi, "\x1b[0m");
    assert!(ansi.starts_with("\x1b["));
}

#[test]
fn test_render_command_eq() {
    let cmd1 = RenderCommand::MoveTo { x: 1, y: 2 };
    let cmd2 = RenderCommand::MoveTo { x: 1, y: 2 };
    assert_eq!(cmd1, cmd2);

    let cmd3 = RenderCommand::Print("abc".to_string());
    let cmd4 = RenderCommand::Print("abc".to_string());
    assert_eq!(cmd3, cmd4);

    assert_ne!(cmd1, cmd3);

    let cmd5 = RenderCommand::ResetStyle;
    let cmd6 = RenderCommand::ResetStyle;
    assert_eq!(cmd5, cmd6);

    let cmd7 = RenderCommand::SetStyle("test".to_string());
    let cmd8 = RenderCommand::SetStyle("test".to_string());
    assert_eq!(cmd7, cmd8);
}

#[test]
fn test_render_command_clone() {
    let cmd = RenderCommand::Print("hello".to_string());
    let cloned = cmd.clone();
    assert_eq!(cmd, cloned);
}

#[test]
fn test_render_command_debug() {
    let cmd = RenderCommand::MoveTo { x: 5, y: 10 };
    let debug = format!("{cmd:?}");
    assert!(debug.contains("MoveTo"));
}

#[test]
fn test_write_command_move_to() {
    let mut output = Vec::new();
    let cmd = RenderCommand::MoveTo { x: 3, y: 7 };
    FrameRenderer::write_command(&mut output, &cmd).unwrap();
    let out = String::from_utf8_lossy(&output);
    // MoveTo uses 1-based indexing: y+1, x+1
    assert!(out.contains("\x1b[8;4H"));
}

#[test]
fn test_write_command_print() {
    let mut output = Vec::new();
    let cmd = RenderCommand::Print("test_text".to_string());
    FrameRenderer::write_command(&mut output, &cmd).unwrap();
    let out = String::from_utf8_lossy(&output);
    assert_eq!(out, "test_text");
}

#[test]
fn test_write_command_set_style() {
    let mut output = Vec::new();
    let cmd = RenderCommand::SetStyle("\x1b[1m".to_string());
    FrameRenderer::write_command(&mut output, &cmd).unwrap();
    let out = String::from_utf8_lossy(&output);
    assert_eq!(out, "\x1b[1m");
}

#[test]
fn test_write_command_reset_style() {
    let mut output = Vec::new();
    let cmd = RenderCommand::ResetStyle;
    FrameRenderer::write_command(&mut output, &cmd).unwrap();
    let out = String::from_utf8_lossy(&output);
    assert_eq!(out, "\x1b[0m");
}
