use {
    super::*,
    crate::{CursorPosition, RemoteClient},
    reovim_driver_display::{BuiltinTheme, FrameBuffer, TokenSpan},
    reovim_protocol::v2::WindowInfo,
};

/// Helper: create default token cache and theme for tests.
fn test_syntax() -> (AnnotationCacheManager, ThemeManager) {
    (AnnotationCacheManager::new(), ThemeManager::new(BuiltinTheme::Dark.load()))
}

/// Helper: create a `WindowInfo` with a buffer.
fn window(id: u64, buffer_id: u64) -> WindowInfo {
    WindowInfo {
        window_id: id,
        buffer_id: Some(buffer_id),
        rect: None,
        focused: true,
        opacity: None,
    }
}

/// Helper: create a `SelectionState`.
fn selection(
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
    mode: &str,
) -> SelectionState {
    SelectionState {
        start: CursorPosition {
            line: start_line,
            column: start_col,
        },
        end: CursorPosition {
            line: end_line,
            column: end_col,
        },
        mode: mode.to_string(),
    }
}

#[test]
fn test_client_color() {
    // Should cycle through palette
    let c0 = client_color(0);
    let c1 = client_color(1);
    let c8 = client_color(8); // Should wrap to c0

    assert_ne!(c0, c1);
    assert_eq!(c0, c8);
}

#[test]
fn test_dimmed_client_color() {
    let d0 = dimmed_client_color(0);
    let d1 = dimmed_client_color(1);
    let d8 = dimmed_client_color(8); // Should wrap to d0

    assert_ne!(d0, d1);
    assert_eq!(d0, d8);

    // Dimmed color should differ from full-intensity color
    let c0 = client_color(0);
    assert_ne!(d0, c0);
}

#[test]
fn test_render_frame_basic() {
    let mut fb = FrameBuffer::new(80, 24);
    let state = TuiCoreState::new(1);
    let config = RenderConfig::default();

    let (tc, tm) = test_syntax();

    // Statusline is now a chrome module — include it
    let statusline = reovim_tui_mod_statusline::StatuslineModule::new();
    let extensions: Vec<Box<dyn ClientModule>> = vec![Box::new(statusline)];
    render_frame(&mut fb, &state, &config, &extensions, &tc, &tm);

    // Should have rendered statusline via chrome dispatch
    let last_row = fb.row(23).unwrap();
    // At minimum, some cells should be non-empty
    assert!(last_row.iter().any(|c| c.char != ' '));
}

#[test]
fn test_mode_style_moved_to_statusline_module() {
    // mode_style is now in reovim-tui-mod-statusline.
    // Tested there: statusline module has 20 tests covering all modes.
    // This test verifies the statusline module renders via chrome dispatch.
    let mut fb = FrameBuffer::new(80, 24);
    let state = TuiCoreState::new(1);
    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();

    // StatuslineModule is a native ClientModule — need to pass it via extensions
    let mut statusline = reovim_tui_mod_statusline::StatuslineModule::new();
    statusline.on_mode_change("NORMAL");
    let extensions: Vec<Box<dyn ClientModule>> = vec![Box::new(statusline)];
    render_frame(&mut fb, &state, &config, &extensions, &tc, &tm);

    // Statusline renders at bottom via chrome dispatch
    let cell = fb.get(1, 23).unwrap();
    assert_eq!(cell.char, 'N'); // " NORMAL " - 'N' at x=1
}

#[test]
fn test_normalize_selection_already_ordered() {
    let sel = selection(1, 5, 3, 10, "char");
    let (sl, sc, el, ec) = normalize_selection(&sel);
    assert_eq!((sl, sc, el, ec), (1, 5, 3, 10));
}

#[test]
fn test_normalize_selection_reversed() {
    let sel = selection(5, 10, 2, 3, "char");
    let (sl, sc, el, ec) = normalize_selection(&sel);
    assert_eq!((sl, sc, el, ec), (2, 3, 5, 10));
}

#[test]
fn test_render_char_selection() {
    // Render a char-mode selection from (0,2) to (0,5) on a single line.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Provide buffer content so label renders after EOL, not over selection
    state
        .buffer_cache
        .insert(100, vec!["0123456789".to_string()]);

    // Add remote client in same buffer with a char selection
    let remote = RemoteClient {
        client_id: 2,
        display_name: "Remote".to_string(),
        cursor_line: 0,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "VISUAL".to_string(),
        selection: Some(selection(0, 2, 0, 5, "char")),
    };
    state.add_remote_client(remote);

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Columns 2..=4 should have dimmed selection bg.
    // Column 5 is the cursor position — cursor overwrites selection bg.
    let expected_bg = Some(dimmed_client_color(2));
    for col in 2..=4u16 {
        let cell = fb.get(col, 0).unwrap();
        assert_eq!(cell.style.bg, expected_bg, "col {col} should have selection bg");
    }

    // Column 0 should NOT have selection bg
    let before = fb.get(0, 0).unwrap();
    assert_ne!(before.style.bg, expected_bg, "col 0 should not have selection bg");
}

#[test]
fn test_render_line_selection() {
    // Render a line-mode selection spanning lines 1..=2.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    state.add_remote_client(RemoteClient {
        client_id: 3,
        display_name: "Remote".to_string(),
        cursor_line: 2,
        cursor_col: 0,
        buffer_id: Some(100),
        mode: "VISUAL LINE".to_string(),
        selection: Some(selection(1, 0, 2, 5, "line")),
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    let expected_bg = Some(dimmed_client_color(3));

    // Entire line 1 and 2 should be highlighted (col 0..40).
    // Cursor is at (2, 0), so skip that exact cell.
    // Label renders on row 2 after EOL (col 1 if no content), safely past checked col.
    for row in 1..=2u16 {
        // Use col 20 (safely past label region) to check selection bg
        let cell_mid = fb.get(20, row).unwrap();
        assert_eq!(
            cell_mid.style.bg, expected_bg,
            "row {row}, col 20 should have line selection bg"
        );
    }

    // Row 0 should NOT be highlighted
    let above = fb.get(0, 0).unwrap();
    assert_ne!(above.style.bg, expected_bg, "row 0 should not have selection bg");
}

#[test]
fn test_render_block_selection() {
    // Render a block-mode selection: columns 3..=7 on lines 0..=2.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Provide buffer content so label renders after EOL, not over selection
    state.buffer_cache.insert(
        100,
        vec![
            "0123456789".to_string(),
            "0123456789".to_string(),
            "0123456789".to_string(),
        ],
    );

    state.add_remote_client(RemoteClient {
        client_id: 4,
        display_name: "Remote".to_string(),
        cursor_line: 2,
        cursor_col: 7,
        buffer_id: Some(100),
        mode: "VISUAL BLOCK".to_string(),
        selection: Some(selection(0, 3, 2, 7, "block")),
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    let expected_bg = Some(dimmed_client_color(4));

    // Columns 3..=7 on rows 0..=2 should be highlighted.
    // Cursor is at (2, 7), so skip that cell.
    for row in 0..=2u16 {
        for col in 3..=7u16 {
            if row == 2 && col == 7 {
                continue; // cursor overwrites selection bg here
            }
            let cell = fb.get(col, row).unwrap();
            assert_eq!(
                cell.style.bg, expected_bg,
                "row {row}, col {col} should have block selection bg"
            );
        }
    }

    // Column 2 on row 0 should NOT be highlighted
    let before = fb.get(2, 0).unwrap();
    assert_ne!(before.style.bg, expected_bg, "col 2 should not have block bg");

    // Column 8 on row 0 should NOT be highlighted
    let after = fb.get(8, 0).unwrap();
    assert_ne!(after.style.bg, expected_bg, "col 8 should not have block bg");
}

#[test]
fn test_render_remote_selection_different_buffer() {
    // Remote client in a different buffer — no selection rendered.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    state.add_remote_client(RemoteClient {
        client_id: 5,
        display_name: "Remote".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(999), // Different buffer
        mode: "VISUAL".to_string(),
        selection: Some(selection(0, 0, 0, 10, "char")),
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // No selection background should appear — cells should have default bg
    let wrong_bg = Some(dimmed_client_color(5));
    for col in 0..=10u16 {
        let cell = fb.get(col, 0).unwrap();
        assert_ne!(
            cell.style.bg, wrong_bg,
            "col {col} should NOT have selection bg (different buffer)"
        );
    }
}

#[test]
fn test_render_local_selection() {
    // Local visual selection rendered with LOCAL_SELECTION_BG.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state
        .window_selections
        .insert(1, selection(0, 3, 0, 8, "char"));

    // Must enable render_self_cursor for local selection rendering
    let config = RenderConfig {
        render_self_cursor: true,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    let expected_bg = Some(LOCAL_SELECTION_BG);
    for col in 3..=8u16 {
        let cell = fb.get(col, 0).unwrap();
        assert_eq!(cell.style.bg, expected_bg, "col {col} should have local selection bg");
    }

    // Column 2 should NOT have selection bg
    let before = fb.get(2, 0).unwrap();
    assert_ne!(before.style.bg, expected_bg, "col 2 should not have local selection bg");
}

#[test]
fn test_render_multiline_char_selection() {
    // Multi-line char selection: first line partial start, middle full, last line partial end.
    // Use a wider terminal so the label fits after EOL without overlapping selections.
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Provide buffer content so label renders after EOL
    state.buffer_cache.insert(
        100,
        vec![
            "0123456789abcdef".to_string(), // 16 chars
            "0123456789abcdef".to_string(),
            "0123456789abcdef".to_string(),
        ],
    );

    state.add_remote_client(RemoteClient {
        client_id: 6,
        display_name: "Remote".to_string(),
        cursor_line: 2,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "VISUAL".to_string(),
        selection: Some(selection(0, 10, 2, 5, "char")),
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    let expected_bg = Some(dimmed_client_color(6));

    // Row 0: columns 10..end should be highlighted (first line, from start_col)
    let cell_before = fb.get(9, 0).unwrap();
    assert_ne!(cell_before.style.bg, expected_bg, "row 0 col 9 should not be highlighted");
    let cell_start = fb.get(10, 0).unwrap();
    assert_eq!(cell_start.style.bg, expected_bg, "row 0 col 10 should be highlighted");

    // Row 1: entire line should be highlighted (middle line)
    let cell_mid = fb.get(0, 1).unwrap();
    assert_eq!(cell_mid.style.bg, expected_bg, "row 1 col 0 should be highlighted");

    // Row 2: columns 0..=5 should be highlighted (last line, up to end_col).
    // Cursor at (2, 5), so check col 4 instead of col 5.
    let cell_end = fb.get(4, 2).unwrap();
    assert_eq!(cell_end.style.bg, expected_bg, "row 2 col 4 should be highlighted");
    let cell_after = fb.get(6, 2).unwrap();
    assert_ne!(cell_after.style.bg, expected_bg, "row 2 col 6 should not be highlighted");
}

// ── Cursor Label Tests ──────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_remote_cursor_label_rendered_after_eol() {
    let mut fb = FrameBuffer::new(80, 24);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Provide buffer content so EOL can be calculated
    state.buffer_cache.insert(
        100,
        vec![
            String::new(),
            String::new(),
            String::new(),
            "hello world".to_string(), // line 3: 11 chars
        ],
    );

    state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "alice".to_string(),
        cursor_line: 3,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Label on cursor row (3), after "hello world" (len 11) + 1 gap = col 12
    let expected_fg = Some(client_color(2));
    let label = label_text("alice", "NORMAL");
    let label_x = 12u16; // eol(11) + 1 gap
    #[allow(clippy::cast_possible_truncation)]
    let label_width = display_width(&label) as u16;

    for i in 0..label_width {
        let cell = fb.get(label_x + i, 3).unwrap();
        assert_eq!(cell.style.fg, expected_fg, "col {} should have label fg", label_x + i);
        assert!(
            cell.style.attributes.has_any_underline(),
            "col {} should be underlined",
            label_x + i,
        );
    }

    // Cell before label should NOT have label fg
    let before = fb.get(label_x - 1, 3).unwrap();
    assert_ne!(before.style.fg, expected_fg, "cell before label should not have label fg");
}

#[test]
fn test_remote_cursor_label_at_line_zero() {
    let mut fb = FrameBuffer::new(80, 24);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    state
        .buffer_cache
        .insert(100, vec!["fn main()".to_string()]);

    state.add_remote_client(RemoteClient {
        client_id: 3,
        display_name: "bob".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Label on row 0 after "fn main()" (len 9) + 1 gap = col 10
    let expected_fg = Some(client_color(3));
    let label = label_text("bob", "NORMAL");
    let label_x = 10u16;
    #[allow(clippy::cast_possible_truncation)]
    let label_width = display_width(&label) as u16;

    for i in 0..label_width {
        let cell = fb.get(label_x + i, 0).unwrap();
        assert_eq!(cell.style.fg, expected_fg, "col {i} on row 0 should have label fg");
        assert!(cell.style.attributes.has_any_underline(), "col {i} should be underlined");
    }
}

#[test]
fn test_remote_cursor_label_truncated() {
    let mut fb = FrameBuffer::new(80, 24);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    state.buffer_cache.insert(
        100,
        vec![String::new(); 6], // 6 empty lines
    );

    state.add_remote_client(RemoteClient {
        client_id: 4,
        display_name: "very-long-username-that-exceeds-limit".to_string(),
        cursor_line: 5,
        cursor_col: 0,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Label should be truncated to MAX_LABEL_WIDTH
    let truncated_label = label_text("very-long-username-that-exceeds-limit", "NORMAL");
    #[allow(clippy::cast_possible_truncation)]
    let label_width = display_width(&truncated_label) as u16;

    // Full name would be 39 chars; truncated should be <= MAX_LABEL_WIDTH + padding + mode
    assert!(label_width < 39, "Label should be truncated, got width {label_width}");

    // Label at col 1 (empty line EOL=0, +1 gap)
    let expected_fg = Some(client_color(4));
    for i in 0..label_width {
        let cell = fb.get(1 + i, 5).unwrap();
        assert_eq!(cell.style.fg, expected_fg, "col {i} should have label fg");
    }
}

#[test]
fn test_remote_cursor_label_skipped_when_no_room() {
    // Narrow terminal: label should be skipped if it doesn't fit after EOL
    let mut fb = FrameBuffer::new(20, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Line fills most of the 20-col screen
    state
        .buffer_cache
        .insert(100, vec!["long content here!".to_string()]);

    state.add_remote_client(RemoteClient {
        client_id: 5,
        display_name: "charlie".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // "long content here!" is 18 chars, label_x = 19 (18+1).
    // Label " charlie [N] " is 14 chars. 19 + 14 = 33 > 20.
    // Label should be skipped entirely.
    let label_fg = Some(client_color(5));
    for x in 0..20u16 {
        let cell = fb.get(x, 0).unwrap();
        assert_ne!(
            cell.style.fg, label_fg,
            "No cell should have label fg at col {x} (label should be skipped)"
        );
    }
}

#[test]
fn test_remote_cursor_label_different_buffer_not_shown() {
    let mut fb = FrameBuffer::new(80, 24);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    state.buffer_cache.insert(100, vec!["hello".to_string(); 5]);

    state.add_remote_client(RemoteClient {
        client_id: 6,
        display_name: "eve".to_string(),
        cursor_line: 3,
        cursor_col: 0,
        buffer_id: Some(999), // Different buffer
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // No label should appear anywhere (client is in a different buffer)
    let label_fg = Some(client_color(6));
    for y in 0..23u16 {
        for x in 0..80u16 {
            let cell = fb.get(x, y).unwrap();
            assert_ne!(
                cell.style.fg, label_fg,
                "No cell should have label fg at ({x}, {y}) for different-buffer client"
            );
        }
    }
}

#[test]
fn test_label_text_helper() {
    // Normal name with mode
    assert_eq!(label_text("alice", "NORMAL"), " alice [N] ");

    // Empty name
    assert_eq!(label_text("", "NORMAL"), " ? [N] ");

    // Long name triggers truncation
    let long = label_text("very-long-username-that-exceeds", "INSERT");
    assert!(long.contains("..."), "Long name should be truncated with '...'");
    assert!(long.contains("[I]"), "Label should contain insert mode indicator");
    assert!(long.starts_with(' '), "Label should have leading space");
    assert!(long.ends_with(' '), "Label should have trailing space");

    // Exact limit (16 chars)
    assert_eq!(label_text("exactly16chars!!", "NORMAL"), " exactly16chars!! [N] ");

    // Mode abbreviations
    assert!(label_text("x", "INSERT").contains("[I]"));
    assert!(label_text("x", "VISUAL").contains("[V]"));
    assert!(label_text("x", "COMMAND").contains("[C]"));
    assert!(label_text("x", "REPLACE").contains("[R]"));
    assert!(label_text("x", "NORMAL").contains("[N]"));
}

// mode_style_command and mode_style_cmdline tests moved to reovim-tui-mod-statusline

#[test]
fn test_mode_abbreviation_cmdline_without_command() {
    // Exercise line 457: mode contains "cmdline" but not "command"
    let abbrev = mode_abbreviation("CMDLINE");
    assert_eq!(abbrev, "[C]");
}

// mode_style_replace and mode_style_case_insensitive tests moved to reovim-tui-mod-statusline

#[test]
fn test_render_config_default() {
    let config = RenderConfig::default();
    assert!(!config.show_line_numbers);
    assert!(!config.render_self_cursor);
    assert_eq!(config.gutter_width, 0);
    assert_eq!(config.line_number_mode, LineNumberMode::None);
}

#[test]
fn test_render_config_debug() {
    let config = RenderConfig::default();
    let debug = format!("{config:?}");
    assert!(debug.contains("RenderConfig"));
}

#[test]
fn test_render_frame_with_buffer_content() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Add buffer content
    state
        .buffer_cache
        .insert(100, vec!["hello world".to_string(), "second line".to_string()]);

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // First line should contain 'h' at position (0, 0)
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'h');

    // Second line should contain 's' at position (0, 1)
    let cell = fb.get(0, 1).unwrap();
    assert_eq!(cell.char, 's');
}

#[test]
fn test_render_frame_tilde_for_empty_lines() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Buffer with just one line - rest should show '~'
    state.buffer_cache.insert(100, vec!["hello".to_string()]);

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Row 1 should have tilde (line beyond buffer content)
    let cell = fb.get(0, 1).unwrap();
    assert_eq!(cell.char, '~');
    assert_eq!(cell.style.fg, Some(Color::DarkGrey));
}

#[test]
fn test_render_frame_with_line_numbers_absolute() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.buffer_cache.insert(
        100,
        vec![
            "line one".to_string(),
            "line two".to_string(),
            "line three".to_string(),
        ],
    );

    let config = RenderConfig {
        show_line_numbers: true,
        line_number_mode: LineNumberMode::Absolute,
        gutter_width: 4,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Content should be offset by gutter_width
    let cell = fb.get(4, 0).unwrap();
    assert_eq!(cell.char, 'l');
}

#[test]
fn test_render_frame_with_line_numbers_relative() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.update_local_cursor(1, 1, 0); // Cursor on line 1
    state.buffer_cache.insert(
        100,
        vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ],
    );

    let config = RenderConfig {
        show_line_numbers: true,
        line_number_mode: LineNumberMode::Relative,
        gutter_width: 4,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Should render without panicking
    let cell = fb.get(4, 0).unwrap();
    assert_eq!(cell.char, 'f');
}

#[test]
fn test_render_frame_with_line_numbers_hybrid() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.update_local_cursor(1, 2, 0);
    state.buffer_cache.insert(
        100,
        vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ],
    );

    let config = RenderConfig {
        show_line_numbers: true,
        line_number_mode: LineNumberMode::Hybrid,
        gutter_width: 4,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Content at gutter offset
    let cell = fb.get(4, 0).unwrap();
    assert_eq!(cell.char, 'a');
}

#[test]
fn test_render_self_cursor_headless() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.update_local_cursor(1, 2, 5);
    state
        .buffer_cache
        .insert(100, vec!["aaaa".to_string(), "bbbb".to_string(), "cccccc".to_string()]);

    let config = RenderConfig {
        render_self_cursor: true,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Self cursor at (5, 2) should have inverse video style
    let cell = fb.get(5, 2).unwrap();
    assert_eq!(cell.style.bg, Some(Color::White));
    assert_eq!(cell.style.fg, Some(Color::Black));
}

#[test]
fn test_render_statusline_with_cursor() {
    let mut fb = FrameBuffer::new(40, 10);
    let state = TuiCoreState::new(1);
    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();

    // Create statusline module with cursor
    let mut statusline = reovim_tui_mod_statusline::StatuslineModule::new();
    statusline.on_mode_change("NORMAL");
    statusline.on_cursor_update(reovim_client_driver::BufferId(0), 3, 7);
    let extensions: Vec<Box<dyn ClientModule>> = vec![Box::new(statusline)];
    render_frame(&mut fb, &state, &config, &extensions, &tc, &tm);

    // Statusline at row 9 (height - 1)
    let cell = fb.get(1, 9).unwrap();
    assert_eq!(cell.char, 'N'); // " NORMAL " starts at x=0 with space, 'N' at x=1
}

#[test]
fn test_render_statusline_without_cursor() {
    let mut fb = FrameBuffer::new(40, 10);
    let state = TuiCoreState::new(1);
    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();

    // Create statusline module without cursor update (shows "?:?")
    let statusline = reovim_tui_mod_statusline::StatuslineModule::new();
    let extensions: Vec<Box<dyn ClientModule>> = vec![Box::new(statusline)];
    render_frame(&mut fb, &state, &config, &extensions, &tc, &tm);

    // Statusline should be rendered (row 9)
    let last_row = fb.row(9).unwrap();
    assert!(last_row.iter().any(|c| c.char != ' '));
}

#[test]
fn test_render_frame_no_windows() {
    let mut fb = FrameBuffer::new(40, 10);
    let state = TuiCoreState::new(1);
    let config = RenderConfig::default();

    // Should not panic with no windows
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);
}

#[test]
fn test_render_self_cursor_no_cursor_data() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    // No cursor data set

    let config = RenderConfig {
        render_self_cursor: true,
        ..RenderConfig::default()
    };

    // Should not panic (skips cursor rendering when no data)
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);
}

#[test]
fn test_cbf8_palette_size() {
    assert_eq!(CBF8_PALETTE.len(), 8);
    assert_eq!(CBF8_DIMMED.len(), 8);
}

#[test]
fn test_client_color_all_palette() {
    // All 8 palette entries should be distinct
    let colors: Vec<Color> = (0..8).map(client_color).collect();
    for i in 0..8 {
        for j in (i + 1)..8 {
            assert_ne!(colors[i], colors[j], "Colors at {i} and {j} should differ");
        }
    }
}

#[test]
fn test_dimmed_client_color_all_palette() {
    let colors: Vec<Color> = (0..8).map(dimmed_client_color).collect();
    for i in 0..8 {
        for j in (i + 1)..8 {
            assert_ne!(colors[i], colors[j], "Dimmed colors at {i} and {j} should differ");
        }
    }
}

#[test]
fn test_render_line_content_truncation() {
    let mut fb = FrameBuffer::new(10, 5);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;

    // Line longer than screen width
    state
        .buffer_cache
        .insert(100, vec!["abcdefghijklmnop".to_string()]);

    let config = RenderConfig::default();
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // First character should be 'a'
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'a');

    // Last visible column (9) should be 'j' (index 9 of "abcdefghij...")
    let cell = fb.get(9, 0).unwrap();
    assert_eq!(cell.char, 'j');
}

#[test]
fn test_render_config_default_opacity() {
    let config = RenderConfig::default();
    assert!((config.opacity - 1.0).abs() < f32::EPSILON);
    assert!(!config.show_line_numbers);
    assert!(!config.render_self_cursor);
    assert_eq!(config.gutter_width, 0);
}

#[test]
fn test_apply_opacity_fully_opaque() {
    let style = Style::default().fg(Color::White);
    let result = apply_opacity(&style, 1.0, Color::Black);
    assert_eq!(result.fg, style.fg);
}

#[test]
fn test_apply_opacity_half_transparent() {
    let style = Style::default().fg(Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    });
    let result = apply_opacity(&style, 0.5, Color::Black);
    // Dimmed color should be darker than original
    if let (Some(Color::Rgb { r: orig, .. }), Some(Color::Rgb { r: dimmed, .. })) =
        (style.fg, result.fg)
    {
        assert!(dimmed < orig, "Dimmed {dimmed} should be less than original {orig}");
    } else {
        panic!("Expected RGB colors");
    }
}

#[test]
fn test_apply_opacity_fully_transparent() {
    let style = Style::default().fg(Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    });
    let result = apply_opacity(&style, 0.0, Color::Black);
    // At opacity 0.0, fg should blend to default_bg (black)
    assert_eq!(result.fg, Some(Color::Black));
}

#[test]
fn test_render_with_opacity_dims_content() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.buffer_cache.insert(100, vec!["hello".to_string()]);

    // Render at full opacity
    let config_full = RenderConfig {
        opacity: 1.0,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config_full, &[], &tc, &tm);
    let full_style = fb.get(0, 0).unwrap().style.clone();

    // Render at half opacity
    let mut fb_dim = FrameBuffer::new(40, 10);
    let config_dim = RenderConfig {
        opacity: 0.5,
        ..RenderConfig::default()
    };
    render_frame(&mut fb_dim, &state, &config_dim, &[], &tc, &tm);
    let dim_style = fb_dim.get(0, 0).unwrap().style.clone();

    // Content characters should be the same
    assert_eq!(fb.get(0, 0).unwrap().char, fb_dim.get(0, 0).unwrap().char);

    // Styles may differ when opacity dims (depending on default style colors)
    // At minimum, the function should not panic
    assert_eq!(full_style.fg, None); // Default style has no explicit fg
    // With dim, fg is still None because dimming None returns None
    assert_eq!(dim_style.fg, None);
}

#[test]
fn test_render_with_opacity_dims_tilde() {
    let mut fb = FrameBuffer::new(40, 10);
    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 100));
    state.focused_window_id = 1;
    state.buffer_cache.insert(100, vec!["one".to_string()]); // 1 line, 8 rows of tildes

    // Render at half opacity
    let config = RenderConfig {
        opacity: 0.5,
        ..RenderConfig::default()
    };
    let (tc, tm) = test_syntax();
    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // Row 1 should have a dimmed tilde
    let tilde_cell = fb.get(0, 1).unwrap();
    assert_eq!(tilde_cell.char, '~');
    // The tilde's fg should be dimmed (darker than DarkGrey at 0.5 opacity)
    assert!(tilde_cell.style.fg.is_some());
}

// =========================================================================
// is_line_folded
// =========================================================================

#[test]
fn test_is_line_folded_empty_ranges() {
    assert!(!is_line_folded(5, &[]));
}

#[test]
fn test_is_line_folded_within_range() {
    let ranges = [(3, 4)]; // lines 3, 4, 5, 6 are hidden
    assert!(!is_line_folded(2, &ranges));
    assert!(is_line_folded(3, &ranges));
    assert!(is_line_folded(5, &ranges));
    assert!(is_line_folded(6, &ranges));
    assert!(!is_line_folded(7, &ranges));
}

#[test]
fn test_is_line_folded_multiple_ranges() {
    let ranges = [(3, 2), (10, 3)]; // lines 3-4 and 10-12 are hidden
    assert!(is_line_folded(3, &ranges));
    assert!(is_line_folded(4, &ranges));
    assert!(!is_line_folded(5, &ranges));
    assert!(is_line_folded(10, &ranges));
    assert!(is_line_folded(12, &ranges));
    assert!(!is_line_folded(13, &ranges));
}

#[test]
fn test_is_line_folded_boundary() {
    let ranges = [(0, 1)]; // only line 0 is hidden
    assert!(is_line_folded(0, &ranges));
    assert!(!is_line_folded(1, &ranges));
}

// =========================================================================
// Syntax highlighting integration
// =========================================================================

/// Helper: populate token cache with tokens for a buffer.
fn populate_tokens(
    tc: &mut AnnotationCacheManager,
    buffer_id: u64,
    content: &str,
    tokens: &[TokenSpan],
) {
    let cache = tc.get_or_create(buffer_id);
    cache.rebuild_line_offsets(content);
    cache.apply_update(tokens, 0, u64::MAX, true, content);
}

#[test]
fn test_render_line_content_applies_syntax_tokens() {
    let mut fb = FrameBuffer::new(20, 1);
    let mut tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    // "fn main" — "fn" is keyword (cols 0..2)
    let content = "fn main";
    populate_tokens(
        &mut tc,
        1,
        content,
        &[TokenSpan {
            start_byte: 0,
            end_byte: 2,
            category: "keyword".to_string(),
        }],
    );

    render_line_content(&mut fb, 0, 0, 20, content, 1.0, Some(1), 0, &tc, &tm, false, &[]);

    // "fn" (cols 0,1) should have keyword style from theme
    let keyword_style = tm.get_style("keyword");
    let cell_f = fb.get(0, 0).unwrap();
    let cell_n = fb.get(1, 0).unwrap();
    assert_eq!(cell_f.char, 'f');
    assert_eq!(cell_n.char, 'n');
    assert_eq!(cell_f.style.fg, keyword_style.fg, "keyword fg mismatch");
    assert_eq!(cell_n.style.fg, keyword_style.fg, "keyword fg mismatch");

    // " " (col 2) and "main" should have default style (no token)
    let cell_space = fb.get(2, 0).unwrap();
    let default_style = Style::default();
    assert_ne!(cell_f.style.fg, default_style.fg, "keyword should differ from default");
    // Space and 'm' should NOT have the keyword color
    assert_ne!(cell_space.style.fg, keyword_style.fg);
}

#[test]
fn test_render_line_content_no_tokens_uses_default_style() {
    let mut fb = FrameBuffer::new(20, 1);
    let tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    render_line_content(&mut fb, 0, 0, 20, "hello", 1.0, Some(1), 0, &tc, &tm, false, &[]);

    // All chars should be rendered with default style
    for col in 0..5u16 {
        let cell = fb.get(col, 0).unwrap();
        assert_eq!(cell.style.fg, Style::default().fg);
    }
}

#[test]
fn test_render_line_content_no_buffer_id_uses_default() {
    let mut fb = FrameBuffer::new(20, 1);
    let tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    // buffer_id=None should not query token cache
    render_line_content(&mut fb, 0, 0, 20, "hello", 1.0, None, 0, &tc, &tm, false, &[]);

    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'h');
    assert_eq!(cell.style.fg, Style::default().fg);
}

#[test]
fn test_render_line_content_token_beyond_width_stops() {
    let mut fb = FrameBuffer::new(3, 1);
    let mut tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    let content = "fn main()";
    populate_tokens(
        &mut tc,
        1,
        content,
        &[TokenSpan {
            start_byte: 0,
            end_byte: 2,
            category: "keyword".to_string(),
        }],
    );

    // Width=3, so only "fn " is rendered (cols 0,1,2)
    render_line_content(&mut fb, 0, 0, 3, content, 1.0, Some(1), 0, &tc, &tm, false, &[]);

    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'f');
    let keyword_style = tm.get_style("keyword");
    assert_eq!(cell.style.fg, keyword_style.fg);

    // Col 2 (' ') should be default style — not keyword
    let cell_space = fb.get(2, 0).unwrap();
    assert_eq!(cell_space.char, ' ');
    assert_ne!(cell_space.style.fg, keyword_style.fg);
}

#[test]
fn test_render_line_content_multiple_tokens() {
    let mut fb = FrameBuffer::new(30, 1);
    let mut tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    // "fn main" — "fn"=keyword, "main"=function
    let content = "fn main";
    populate_tokens(
        &mut tc,
        1,
        content,
        &[
            TokenSpan {
                start_byte: 0,
                end_byte: 2,
                category: "keyword".to_string(),
            },
            TokenSpan {
                start_byte: 3,
                end_byte: 7,
                category: "function".to_string(),
            },
        ],
    );

    render_line_content(&mut fb, 0, 0, 30, content, 1.0, Some(1), 0, &tc, &tm, false, &[]);

    let keyword_style = tm.get_style("keyword");
    let function_style = tm.get_style("function");

    // "fn" should have keyword style
    assert_eq!(fb.get(0, 0).unwrap().style.fg, keyword_style.fg);
    assert_eq!(fb.get(1, 0).unwrap().style.fg, keyword_style.fg);

    // "main" (cols 3-6) should have function style
    assert_eq!(fb.get(3, 0).unwrap().style.fg, function_style.fg);
    assert_eq!(fb.get(6, 0).unwrap().style.fg, function_style.fg);
}

#[test]
fn test_render_line_content_correct_line_index() {
    let mut fb = FrameBuffer::new(20, 1);
    let mut tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    // Two-line content, tokens only on line 1
    let content = "hello\nfn world";
    populate_tokens(
        &mut tc,
        1,
        content,
        &[TokenSpan {
            start_byte: 6,
            end_byte: 8,
            category: "keyword".to_string(),
        }],
    );

    // Render line 0 ("hello") — should have no keyword styling
    render_line_content(&mut fb, 0, 0, 20, "hello", 1.0, Some(1), 0, &tc, &tm, false, &[]);
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.style.fg, Style::default().fg);

    // Render line 1 ("fn world") — "fn" should have keyword styling
    let mut fb2 = FrameBuffer::new(20, 1);
    render_line_content(&mut fb2, 0, 0, 20, "fn world", 1.0, Some(1), 1, &tc, &tm, false, &[]);
    let keyword_style = tm.get_style("keyword");
    assert_eq!(fb2.get(0, 0).unwrap().style.fg, keyword_style.fg);
}

#[test]
fn test_render_frame_with_syntax_tokens() {
    let mut fb = FrameBuffer::new(40, 24);
    let mut tc = AnnotationCacheManager::new();
    let tm = ThemeManager::new(BuiltinTheme::Dark.load());

    let content = "fn main()";
    populate_tokens(
        &mut tc,
        42,
        content,
        &[TokenSpan {
            start_byte: 0,
            end_byte: 2,
            category: "keyword".to_string(),
        }],
    );

    let mut state = TuiCoreState::new(1);
    state.windows.push(window(1, 42));
    state.focused_window_id = 1;
    state.buffer_cache.insert(42, vec!["fn main()".to_string()]);
    let config = RenderConfig::default();

    render_frame(&mut fb, &state, &config, &[], &tc, &tm);

    // "fn" at content area should have keyword color
    let keyword_style = tm.get_style("keyword");
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'f');
    assert_eq!(cell.style.fg, keyword_style.fg);
}

// =========================================================================
// classify_with_extensions tests
// =========================================================================

#[test]
fn test_classify_with_extensions_no_extensions_is_highlight() {
    let result = classify_with_extensions(&[], "keyword.function");
    assert!(matches!(result, RenderBehavior::Highlight));
}

#[test]
fn test_classify_with_extensions_unknown_category_is_highlight() {
    let result = classify_with_extensions(&[], "some.random.category");
    assert!(matches!(result, RenderBehavior::Highlight));
}

// =========================================================================
// buffer_to_screen_row_vl tests
// =========================================================================

#[test]
fn test_buffer_to_screen_row_vl_no_virtual_lines() {
    assert_eq!(buffer_to_screen_row_vl(5, 2, &[]), 3);
}

#[test]
fn test_buffer_to_screen_row_vl_with_virtual_lines() {
    let vl = VirtualLine {
        buffer_line: 1,
        position: VirtualLinePosition::Before,
        content: "border".to_string(),
        style: Style::default(),
    };
    let vls: Vec<&VirtualLine> = vec![&vl];
    // Line 0: no virtual lines before it → screen 0
    assert_eq!(buffer_to_screen_row_vl(0, 0, &vls), 0);
    // Line 1: 1 virtual line at line 1 → screen 2
    assert_eq!(buffer_to_screen_row_vl(1, 0, &vls), 2);
    // Line 2: still 1 virtual line (at line 1) → screen 3
    assert_eq!(buffer_to_screen_row_vl(2, 0, &vls), 3);
}
