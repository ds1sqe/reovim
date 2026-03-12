use super::*;

#[test]
fn test_screen_new() {
    let screen = Screen::new(80, 24);
    assert_eq!(screen.width(), 80);
    assert_eq!(screen.height(), 24);
}

#[test]
fn test_screen_resize() {
    let mut screen = Screen::new(80, 24);
    screen.resize(120, 40);
    assert_eq!(screen.width(), 120);
    assert_eq!(screen.height(), 40);
}

#[test]
fn test_screen_write_str() {
    let mut screen = Screen::new(80, 24);
    screen.write_str(0, 0, "Hello", &Style::default());

    // Verify via buffer
    let row = screen.buffer().row(0).expect("row exists");
    assert_eq!(row[0].char, 'H');
    assert_eq!(row[1].char, 'e');
    assert_eq!(row[2].char, 'l');
    assert_eq!(row[3].char, 'l');
    assert_eq!(row[4].char, 'o');
}

#[test]
fn test_screen_clear() {
    let mut screen = Screen::new(80, 24);
    screen.write_str(0, 0, "Hello", &Style::default());
    screen.clear();

    let row = screen.buffer().row(0).expect("row exists");
    assert_eq!(row[0].char, ' ');
}

#[test]
fn test_screen_put_char() {
    let mut screen = Screen::new(80, 24);
    screen.put_char(5, 3, 'X', &Style::default());

    let row = screen.buffer().row(3).expect("row exists");
    assert_eq!(row[5].char, 'X');
}

#[test]
fn test_screen_draw_box() {
    let mut screen = Screen::new(80, 24);
    screen.draw_box(0, 0, 5, 3, &Style::default());

    let row0 = screen.buffer().row(0).expect("row exists");
    let row1 = screen.buffer().row(1).expect("row exists");
    let row2 = screen.buffer().row(2).expect("row exists");

    // Corners
    assert_eq!(row0[0].char, '┌');
    assert_eq!(row0[4].char, '┐');
    assert_eq!(row2[0].char, '└');
    assert_eq!(row2[4].char, '┘');

    // Edges
    assert_eq!(row0[1].char, '─');
    assert_eq!(row1[0].char, '│');
}

#[test]
fn test_screen_draw_box_too_small() {
    let mut screen = Screen::new(80, 24);
    // Box too small (width < 2)
    screen.draw_box(0, 0, 1, 3, &Style::default());
    // Should not crash, just do nothing
    let row0 = screen.buffer().row(0).expect("row exists");
    assert_eq!(row0[0].char, ' ');
}

#[test]
fn test_screen_write_centered() {
    let mut screen = Screen::new(80, 24);
    screen.write_centered(0, "Hello", &Style::default());

    let row = screen.buffer().row(0).expect("row exists");
    // Should be centered: (80 - 5) / 2 = 37
    assert_eq!(row[37].char, 'H');
    assert_eq!(row[38].char, 'e');
}

#[test]
fn test_screen_fill_region() {
    let mut screen = Screen::new(80, 24);
    screen.fill_region(5, 3, 4, 2, 'X', &Style::default());

    let row3 = screen.buffer().row(3).expect("row exists");
    let row4 = screen.buffer().row(4).expect("row exists");
    assert_eq!(row3[5].char, 'X');
    assert_eq!(row3[8].char, 'X');
    assert_eq!(row4[5].char, 'X');
}

#[test]
fn test_screen_fill_horizontal() {
    let mut screen = Screen::new(80, 24);
    screen.fill_horizontal(10, 5, 5, '-', &Style::default());

    let row = screen.buffer().row(5).expect("row exists");
    assert_eq!(row[10].char, '-');
    assert_eq!(row[14].char, '-');
    assert_eq!(row[15].char, ' '); // Should not fill beyond width
}

#[test]
fn test_screen_fill_vertical() {
    let mut screen = Screen::new(80, 24);
    screen.fill_vertical(10, 5, 3, '|', &Style::default());

    let row5 = screen.buffer().row(5).expect("row exists");
    let row6 = screen.buffer().row(6).expect("row exists");
    let row7 = screen.buffer().row(7).expect("row exists");
    assert_eq!(row5[10].char, '|');
    assert_eq!(row6[10].char, '|');
    assert_eq!(row7[10].char, '|');
}

#[test]
fn test_screen_buffer_mut() {
    let mut screen = Screen::new(80, 24);
    screen.buffer_mut().put_char(5, 5, 'Z', &Style::default());

    let row = screen.buffer().row(5).expect("row exists");
    assert_eq!(row[5].char, 'Z');
}

#[test]
fn test_screen_overlay_bg() {
    let mut screen = Screen::new(80, 24);
    screen.put_char(5, 5, 'A', &Style::default());
    screen.overlay_bg(5, 5, reovim_arch::Color::Red);

    let cell = screen.buffer().get(5, 5).expect("cell exists");
    assert_eq!(cell.char, 'A');
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Red));
}

#[test]
fn test_screen_overlay_bg_out_of_bounds() {
    let mut screen = Screen::new(10, 10);
    // Should not crash
    screen.overlay_bg(100, 100, reovim_arch::Color::Red);
}

#[test]
fn test_screen_apply_style() {
    let mut screen = Screen::new(80, 24);
    screen.put_char(3, 3, 'B', &Style::default());
    let style = Style::new().with_bg(reovim_arch::Color::Blue);
    screen.apply_style(3, 3, &style);

    let cell = screen.buffer().get(3, 3).expect("cell exists");
    assert_eq!(cell.char, 'B');
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
}

#[test]
fn test_screen_invalidate() {
    let mut screen = Screen::new(80, 24);
    screen.write_str(0, 0, "Test", &Style::default());
    screen.invalidate();
    // Should not crash
    assert_eq!(screen.width(), 80);
}

#[test]
fn test_screen_debug() {
    let screen = Screen::new(80, 24);
    let debug = format!("{screen:?}");
    assert!(debug.contains("Screen"));
    assert!(debug.contains("80"));
    assert!(debug.contains("24"));
}

#[test]
fn test_screen_from_terminal_size_fallback() {
    // In CI without terminal, this might fail
    let result = Screen::from_terminal_size();
    // Just ensure it doesn't panic
    let _ = result;
}

#[test]
fn test_screen_render_to() {
    let mut screen = Screen::new(10, 2);
    screen.write_str(0, 0, "Hello", &Style::default());
    screen.write_str(0, 1, "World", &Style::default());

    let mut output = Vec::new();
    screen.render_to(&mut output).unwrap();

    let out = String::from_utf8_lossy(&output);
    assert!(out.contains("Hello"));
    assert!(out.contains("World"));
}

#[test]
fn test_screen_render_to_twice() {
    let mut screen = Screen::new(10, 1);
    screen.write_str(0, 0, "First", &Style::default());

    let mut out1 = Vec::new();
    screen.render_to(&mut out1).unwrap();
    assert!(!out1.is_empty());

    // Render same content again (diff should detect no changes)
    screen.write_str(0, 0, "First", &Style::default());
    let mut out2 = Vec::new();
    screen.render_to(&mut out2).unwrap();
    assert!(out2.len() <= out1.len());
}

#[test]
fn test_screen_render_to_with_style() {
    let mut screen = Screen::new(10, 1);
    let styled = Style::new().with_fg(reovim_arch::Color::Green).bold();
    screen.write_str(0, 0, "Styled", &styled);

    let mut output = Vec::new();
    screen.render_to(&mut output).unwrap();

    let out = String::from_utf8_lossy(&output);
    assert!(out.contains("Styled"));
    // Should include ANSI escape codes
    assert!(out.contains("\x1b["));
}

#[test]
fn test_screen_draw_box_height_too_small() {
    let mut screen = Screen::new(80, 24);
    // Height < 2 should do nothing
    screen.draw_box(0, 0, 5, 1, &Style::default());
    let row0 = screen.buffer().row(0).expect("row exists");
    assert_eq!(row0[0].char, ' ');
}

#[test]
fn test_screen_draw_box_both_too_small() {
    let mut screen = Screen::new(80, 24);
    // Both width and height < 2
    screen.draw_box(0, 0, 1, 1, &Style::default());
    let row0 = screen.buffer().row(0).expect("row exists");
    assert_eq!(row0[0].char, ' ');
}

#[test]
fn test_screen_draw_box_larger() {
    let mut screen = Screen::new(80, 24);
    screen.draw_box(1, 1, 6, 4, &Style::default());

    let row1 = screen.buffer().row(1).expect("row exists");
    let row2 = screen.buffer().row(2).expect("row exists");
    let row3 = screen.buffer().row(3).expect("row exists");
    let row4 = screen.buffer().row(4).expect("row exists");

    // Top-left corner
    assert_eq!(row1[1].char, '\u{250c}'); // ┌
    // Top-right corner
    assert_eq!(row1[6].char, '\u{2510}'); // ┐
    // Bottom-left corner
    assert_eq!(row4[1].char, '\u{2514}'); // └
    // Bottom-right corner
    assert_eq!(row4[6].char, '\u{2518}'); // ┘
    // Vertical edges
    assert_eq!(row2[1].char, '\u{2502}'); // │
    assert_eq!(row2[6].char, '\u{2502}');
    assert_eq!(row3[1].char, '\u{2502}');
    assert_eq!(row3[6].char, '\u{2502}');
    // Horizontal edges
    assert_eq!(row1[2].char, '\u{2500}'); // ─
    assert_eq!(row1[5].char, '\u{2500}');
    assert_eq!(row4[2].char, '\u{2500}');
}

#[test]
fn test_screen_write_centered_long_text() {
    let mut screen = Screen::new(10, 1);
    // Text longer than screen width
    screen.write_centered(0, "VeryLongTextString", &Style::default());
    // x = (10 - 18) / 2 = 0 (saturating_sub makes it 0)
    let row = screen.buffer().row(0).expect("row exists");
    assert_eq!(row[0].char, 'V');
}

#[test]
fn test_screen_fill_region_clamped() {
    let mut screen = Screen::new(5, 5);
    // Fill region that extends beyond screen bounds
    screen.fill_region(3, 3, 10, 10, 'X', &Style::default());
    let row3 = screen.buffer().row(3).expect("row exists");
    assert_eq!(row3[3].char, 'X');
    assert_eq!(row3[4].char, 'X');
    let row4 = screen.buffer().row(4).expect("row exists");
    assert_eq!(row4[3].char, 'X');
    assert_eq!(row4[4].char, 'X');
}

#[test]
fn test_screen_invalidate_then_render() {
    let mut screen = Screen::new(10, 1);
    screen.write_str(0, 0, "Hello", &Style::default());

    let mut out1 = Vec::new();
    screen.render_to(&mut out1).unwrap();

    // Invalidate forces full redraw
    screen.invalidate();
    screen.write_str(0, 0, "Hello", &Style::default());

    let mut out2 = Vec::new();
    screen.render_to(&mut out2).unwrap();
    // Full redraw should produce output
    let out_str = String::from_utf8_lossy(&out2);
    assert!(out_str.contains("Hello"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_screen_sync_to_renderer() {
    // Test that sync_to_renderer copies buffer content properly
    let mut screen = Screen::new(5, 2);
    screen.put_char(0, 0, 'A', &Style::default());
    screen.put_char(1, 0, 'B', &Style::default());
    screen.put_char(0, 1, 'C', &Style::default());

    let mut output = Vec::new();
    screen.render_to(&mut output).unwrap();

    let out = String::from_utf8_lossy(&output);
    assert!(out.contains('A'));
    assert!(out.contains('B'));
    assert!(out.contains('C'));
}

#[test]
fn test_screen_overlay_bg_preserves_fg() {
    let mut screen = Screen::new(10, 10);
    let style = Style::new().with_fg(reovim_arch::Color::Green);
    screen.put_char(2, 2, 'X', &style);
    screen.overlay_bg(2, 2, reovim_arch::Color::Yellow);

    let cell = screen.buffer().get(2, 2).expect("cell exists");
    assert_eq!(cell.char, 'X');
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Green));
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Yellow));
}

#[test]
fn test_screen_apply_style_preserves_char() {
    let mut screen = Screen::new(10, 10);
    screen.put_char(1, 1, 'Q', &Style::default());
    let new_style = Style::new()
        .with_fg(reovim_arch::Color::Red)
        .with_bg(reovim_arch::Color::Blue);
    screen.apply_style(1, 1, &new_style);

    let cell = screen.buffer().get(1, 1).expect("cell exists");
    assert_eq!(cell.char, 'Q');
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Red));
    assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
}

#[test]
fn test_screen_apply_style_out_of_bounds() {
    let mut screen = Screen::new(5, 5);
    let style = Style::new().with_bg(reovim_arch::Color::Red);
    // Should not crash
    screen.apply_style(100, 100, &style);
}

#[test]
fn test_screen_resize_clears_content() {
    let mut screen = Screen::new(10, 10);
    screen.write_str(0, 0, "Hello", &Style::default());
    screen.resize(5, 5);

    // After resize, buffer is recreated
    let row = screen.buffer().row(0).expect("row exists");
    assert_eq!(row[0].char, ' ');
}
