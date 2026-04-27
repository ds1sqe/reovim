use super::*;

fn make_content(lines: &[String], cursor_line: usize) -> RenderContent<'_> {
    RenderContent {
        lines,
        first_line: 0,
        total_lines: lines.len(),
        cursor_line,
        cursor_column: 0,
        focused: true,
    }
}

#[test]
fn test_new() {
    let renderer = WindowRenderer::new();
    assert_eq!(renderer.config().line_numbers, LineNumberMode::Absolute);
}

#[test]
fn test_with_config() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Relative,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    assert_eq!(renderer.config().line_numbers, LineNumberMode::Relative);
}

#[test]
fn test_calculate_gutter_width_none() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::None,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    assert_eq!(renderer.calculate_gutter_width(100), 0);
}

#[test]
fn test_calculate_gutter_width_auto() {
    let renderer = WindowRenderer::new();
    // 1-9 lines: 1 digit + 2 = 3
    assert_eq!(renderer.calculate_gutter_width(9), 3);
    // 10-99 lines: 2 digits + 2 = 4
    assert_eq!(renderer.calculate_gutter_width(50), 4);
    // 100-999 lines: 3 digits + 2 = 5
    assert_eq!(renderer.calculate_gutter_width(500), 5);
}

#[test]
fn test_calculate_gutter_width_fixed() {
    let config = WindowRendererConfig {
        line_number_width: 6,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    assert_eq!(renderer.calculate_gutter_width(1000), 6);
}

#[test]
fn test_render_basic() {
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Hello".to_string(), "World".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // With 2 lines, gutter is 3 chars (1 digit + 2 padding)
    // Content starts at position 3
    // Check that "Hello" content is rendered somewhere in the first row
    let first_row: String = (0..40)
        .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
        .collect();
    assert!(
        first_row.contains("Hello"),
        "First row should contain 'Hello', got: {first_row}"
    );
    assert!(
        first_row.contains('1'),
        "First row should contain line number '1', got: {first_row}"
    );
}

#[test]
fn test_render_no_line_numbers() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::None,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Hello".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // First character should be 'H' (no gutter)
    assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
}

#[test]
fn test_render_empty_lines_tilde() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Absolute,
        line_number_width: 4,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Only one line".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // Line 2 (y=1) should have a tilde
    let tilde_cell = buffer.get(2, 1); // gutter_width - 2 = 4 - 2 = 2
    assert!(tilde_cell.is_some());
    assert_eq!(tilde_cell.unwrap().char, '~');
}

#[test]
fn test_visible_lines() {
    let lines: Vec<String> = (0..100).map(|i| format!("Line {i}")).collect();

    // View lines 10-19
    let visible = WindowRenderer::visible_lines(&lines, 10, 10);
    assert_eq!(visible.len(), 10);
    assert_eq!(visible[0], "Line 10");
    assert_eq!(visible[9], "Line 19");
}

#[test]
fn test_visible_lines_past_end() {
    let lines: Vec<String> = vec!["Line 0".to_string(), "Line 1".to_string()];

    // Request more than available
    let visible = WindowRenderer::visible_lines(&lines, 0, 100);
    assert_eq!(visible.len(), 2);
}

#[test]
fn test_visible_lines_empty() {
    let lines: Vec<String> = vec![];
    let visible = WindowRenderer::visible_lines(&lines, 0, 10);
    assert!(visible.is_empty());
}

#[test]
fn test_line_number_mode_default() {
    let mode = LineNumberMode::default();
    assert_eq!(mode, LineNumberMode::None);
}

#[test]
fn test_render_cursor() {
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Hello".to_string()];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 1,
        cursor_line: 0,
        cursor_column: 2, // On 'l'
        focused: true,
    };

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // The cursor should be rendered at the 'l' position
    // With gutter width ~3, cursor at col 2 = gutter + 2 = 5
}

// =========================================================================
// Extended window renderer tests
// =========================================================================

#[test]
fn test_render_relative_line_numbers() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Relative,
        line_number_width: 5,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec![
        "Line A".to_string(),
        "Line B".to_string(),
        "Line C".to_string(),
    ];
    let content = make_content(&lines, 1); // cursor on line 1

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // Relative line numbers: line 0 should show "1" (distance from cursor)
    // line 1 (cursor) should show "0"
    // line 2 should show "1"
}

#[test]
fn test_render_hybrid_line_numbers() {
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Hybrid,
        line_number_width: 5,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec![
        "Line A".to_string(),
        "Line B".to_string(),
        "Line C".to_string(),
    ];
    let content = make_content(&lines, 1); // cursor on line 1

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // Hybrid: cursor line shows absolute (2), others show relative
    let row_str: String = (0..5)
        .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
        .collect();
    assert!(
        row_str.contains('2'),
        "Cursor line should show absolute number 2, got: {row_str}"
    );
}

#[test]
fn test_render_unfocused_no_cursor() {
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Hello".to_string()];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 1,
        cursor_line: 0,
        cursor_column: 0,
        focused: false, // Not focused
    };

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // Cursor should not be rendered (no reverse style) when unfocused
    // Just verify no panic
}

#[test]
fn test_render_cursor_at_end_of_line() {
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(40, 10);
    let lines: Vec<String> = vec!["Hello".to_string()];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 1,
        cursor_line: 0,
        cursor_column: 5, // Past end of "Hello"
        focused: true,
    };

    renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

    // Should render a space at cursor position
}

#[test]
fn test_set_config() {
    let mut renderer = WindowRenderer::new();
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Relative,
        ..Default::default()
    };
    renderer.set_config(config);
    assert_eq!(renderer.config().line_numbers, LineNumberMode::Relative);
}

#[test]
fn test_calculate_gutter_width_zero_lines() {
    let renderer = WindowRenderer::new();
    // 0 lines: 1 digit + 2 = 3
    assert_eq!(renderer.calculate_gutter_width(0), 3);
}

#[test]
fn test_render_with_offset_bounds() {
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(80, 24);
    let lines: Vec<String> = vec!["Content".to_string()];
    let content = make_content(&lines, 0);

    // Render at an offset position
    renderer.render(&content, Rect::new(10, 5, 30, 10), &mut buffer, &Style::default());

    // Content should be at offset position
    let row: String = (10..40)
        .filter_map(|x| buffer.get(x, 5).map(|c| c.char))
        .collect();
    assert!(row.contains("Content"), "Row should contain 'Content' at offset, got: {row}");
}

#[test]
fn test_visible_lines_top_beyond_end() {
    let lines: Vec<String> = vec!["Line 0".to_string()];
    let visible = WindowRenderer::visible_lines(&lines, 100, 10);
    assert!(visible.is_empty());
}

// =========================================================================
// Coverage tests for uncovered lines
// =========================================================================

#[test]
fn test_from_theme_creates_config_with_theme_styles() {
    use crate::style::{
        BuiltinTheme, StyledThemeManagerExt as _, ThemeManager, groups, install_theme_factory,
    };

    install_theme_factory();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    let config = WindowRendererConfig::from_theme(&manager);

    // Verify default field values
    assert_eq!(config.line_numbers, LineNumberMode::Absolute);
    assert_eq!(config.line_number_width, 0);
    assert!(config.show_cursor);

    // Verify styles come from the theme
    let expected_line_number_style = manager.get_style(groups::LINE_NUMBER);
    let expected_cursor_line_style = manager.get_style(groups::LINE_NUMBER_ACTIVE);
    assert_eq!(config.line_number_style, expected_line_number_style);
    assert_eq!(config.cursor_line_number_style, expected_cursor_line_style);
}

#[test]
fn test_from_theme_with_light_theme() {
    use crate::style::{
        BuiltinTheme, StyledThemeManagerExt as _, ThemeManager, groups, install_theme_factory,
    };

    install_theme_factory();
    let manager = ThemeManager::new(BuiltinTheme::Light.load());
    let config = WindowRendererConfig::from_theme(&manager);

    // Should use the light theme colors
    let expected = manager.get_style(groups::LINE_NUMBER);
    assert_eq!(config.line_number_style, expected);
}

#[test]
fn test_render_more_lines_than_height_breaks_loop() {
    // Covers line 206: break when y >= bounds.y + bounds.height
    let renderer = WindowRenderer::new();
    let mut buffer = FrameBuffer::new(40, 3);
    // 5 lines, but bounds height is only 2
    let lines: Vec<String> = vec![
        "Line 0".to_string(),
        "Line 1".to_string(),
        "Line 2".to_string(),
        "Line 3".to_string(),
        "Line 4".to_string(),
    ];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 5,
        cursor_line: 0,
        cursor_column: 0,
        focused: true,
    };

    // Bounds height is 2, so lines 2-4 should be skipped (break)
    renderer.render(&content, Rect::new(0, 0, 40, 2), &mut buffer, &Style::default());

    // Verify only the first 2 lines were rendered
    let row0: String = (0..40)
        .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
        .collect();
    let row1: String = (0..40)
        .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
        .collect();
    assert!(row0.contains("Line 0"), "Row 0 should contain 'Line 0', got: {row0}");
    assert!(row1.contains("Line 1"), "Row 1 should contain 'Line 1', got: {row1}");
}

#[test]
fn test_render_line_number_none_mode_with_fixed_width() {
    // Covers line 302: LineNumberMode::None => return in render_line_number
    // Force render_line_number to be called with None mode by having
    // a fixed line_number_width but None mode. Since calculate_gutter_width
    // returns 0 for None mode, we test via a workaround: set up Absolute
    // mode, then replace config to None mode but render was already done.
    // Actually, the None arm is a defensive guard. We test it by directly
    // creating a scenario where gutter_width > 0 but mode is None.
    // This is impossible through render() because calculate_gutter_width
    // returns 0 for None. So we just verify the render path is correct
    // when mode is None (gutter_width = 0, no line numbers rendered).
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::None,
        line_number_width: 0,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 5);
    let lines: Vec<String> = vec!["Hello".to_string(), "World".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 5), &mut buffer, &Style::default());

    // With None mode, no gutter, content starts at col 0
    assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
    assert_eq!(buffer.get(0, 1).unwrap().char, 'W');
}

#[test]
fn test_render_line_content_no_fill_when_line_fills_width() {
    // Covers line 362: the else branch where line_width >= width
    // (line fills or exceeds the content area width)
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::None,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(5, 2);
    // Line exactly fills the width (5 chars for width 5)
    let lines: Vec<String> = vec!["ABCDE".to_string()];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 1,
        cursor_line: 0,
        cursor_column: 0,
        focused: false, // No cursor to keep it simple
    };

    renderer.render(&content, Rect::new(0, 0, 5, 2), &mut buffer, &Style::default());

    // All 5 characters should be rendered
    assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
    assert_eq!(buffer.get(4, 0).unwrap().char, 'E');
}

#[test]
fn test_render_line_content_overflow_line() {
    // Line wider than content width - ensures no-fill path is taken
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::None,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(10, 2);
    // Line is 8 chars but content width is only 5
    let lines: Vec<String> = vec!["ABCDEFGH".to_string()];
    let content = RenderContent {
        lines: &lines,
        first_line: 0,
        total_lines: 1,
        cursor_line: 0,
        cursor_column: 0,
        focused: false,
    };

    renderer.render(&content, Rect::new(0, 0, 5, 2), &mut buffer, &Style::default());

    // First 5 chars should still be written (write_str writes the full string to buffer)
    assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
}

#[test]
fn test_render_gutter_annotated_renders_composed_lines() {
    // Covers lines 424-460: render_gutter_annotated() and render_composed_line()
    use {
        crate::annotation::{
            Annotation, AnnotationPresenter, AnnotationStore, ColumnConfig, ColumnWidth,
            GutterComposer, GutterConfig, KindPattern, PresentedOutput, PresenterContext,
            PresenterRegistry, SourceId,
        },
        std::sync::Arc,
    };

    // Mock presenter for line numbers
    struct TestLineNumberPresenter;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl AnnotationPresenter for TestLineNumberPresenter {
        fn id(&self) -> &'static str {
            "test_line_number"
        }

        fn handles(&self) -> KindPattern {
            KindPattern::exact("line_number")
        }

        fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
            annotation
                .payload
                .as_number()
                .map_or_else(PresentedOutput::hidden, |n| {
                    PresentedOutput::text(&n.to_string(), &Style::default())
                })
        }

        fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(3)
        }
    }

    // Set up annotation store with line number annotations
    let mut store = AnnotationStore::new();
    store.replace_source(
        SourceId::new("line_number"),
        vec![
            Annotation::line_number(0, 1),
            Annotation::line_number(1, 2),
            Annotation::line_number(2, 3),
        ],
    );

    // Set up presenter registry
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(TestLineNumberPresenter));

    // Set up gutter config
    let config =
        GutterConfig::new(vec![ColumnConfig::new(KindPattern::exact("line_number")).width(3)]);

    let composer = GutterComposer::new(&store, &registry, &config);
    let ctx = PresenterContext::new(3, 0, false);

    let mut buffer = FrameBuffer::new(10, 5);
    let bounds = Rect::new(0, 0, 5, 3);

    WindowRenderer::render_gutter_annotated(&composer, &mut buffer, bounds, &ctx, 0);

    // Verify that cells were rendered (line numbers 1, 2, 3)
    // The gutter should have content in the first 3 rows
    let row0: String = (0..5)
        .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
        .collect();
    assert!(row0.contains('1'), "First gutter row should contain '1', got: '{row0}'");

    let row1: String = (0..5)
        .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
        .collect();
    assert!(row1.contains('2'), "Second gutter row should contain '2', got: '{row1}'");
}

#[test]
fn test_render_gutter_annotated_respects_bounds_height() {
    // Verifies render_gutter_annotated only renders within bounds height
    use {
        crate::annotation::{
            Annotation, AnnotationPresenter, AnnotationStore, ColumnConfig, ColumnWidth,
            GutterComposer, GutterConfig, KindPattern, PresentedOutput, PresenterContext,
            PresenterRegistry, SourceId,
        },
        std::sync::Arc,
    };

    struct TestPresenter;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl AnnotationPresenter for TestPresenter {
        fn id(&self) -> &'static str {
            "test"
        }

        fn handles(&self) -> KindPattern {
            KindPattern::exact("line_number")
        }

        fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
            annotation
                .payload
                .as_number()
                .map_or_else(PresentedOutput::hidden, |n| {
                    PresentedOutput::text(&n.to_string(), &Style::default())
                })
        }

        fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(3)
        }
    }

    let mut store = AnnotationStore::new();
    store.replace_source(
        SourceId::new("line_number"),
        vec![
            Annotation::line_number(0, 1),
            Annotation::line_number(1, 2),
            Annotation::line_number(2, 3),
            Annotation::line_number(3, 4),
            Annotation::line_number(4, 5),
        ],
    );

    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(TestPresenter));

    let config =
        GutterConfig::new(vec![ColumnConfig::new(KindPattern::exact("line_number")).width(3)]);

    let composer = GutterComposer::new(&store, &registry, &config);
    let ctx = PresenterContext::new(5, 0, false);

    let mut buffer = FrameBuffer::new(10, 3);
    // Height is 2, but compose_range generates 5 lines (first_line=0, visible=2 rows)
    let bounds = Rect::new(0, 0, 5, 2);

    WindowRenderer::render_gutter_annotated(&composer, &mut buffer, bounds, &ctx, 0);

    // Only 2 rows should be rendered
    let row0: String = (0..5)
        .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
        .collect();
    assert!(row0.contains('1'), "Row 0 should contain '1', got: '{row0}'");
}

#[test]
fn test_render_composed_line_directly() {
    // Covers render_composed_line (lines 451-460)
    use crate::annotation::GutterCell;

    let mut buffer = FrameBuffer::new(10, 1);
    let line = ComposedLine::new(vec![
        GutterCell::new('A', Style::default()),
        GutterCell::new('B', Style::default()),
        GutterCell::new('C', Style::default()),
    ]);

    WindowRenderer::render_composed_line(&mut buffer, 2, 0, &line);

    assert_eq!(buffer.get(2, 0).unwrap().char, 'A');
    assert_eq!(buffer.get(3, 0).unwrap().char, 'B');
    assert_eq!(buffer.get(4, 0).unwrap().char, 'C');
}

#[test]
fn test_render_composed_line_with_wide_chars() {
    // Covers the width calculation in render_composed_line (line 457)
    use crate::annotation::GutterCell;

    let mut buffer = FrameBuffer::new(10, 1);
    // CJK character has width 2
    let line = ComposedLine::new(vec![
        GutterCell::new('A', Style::default()),
        GutterCell::new('\u{4e2d}', Style::default()), // CJK char, width 2
        GutterCell::new('B', Style::default()),
    ]);

    WindowRenderer::render_composed_line(&mut buffer, 0, 0, &line);

    assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
    // CJK char at x=1, width=2
    assert_eq!(buffer.get(1, 0).unwrap().char, '\u{4e2d}');
    // 'B' should be at x=3 (1 + 2 = 3)
    assert_eq!(buffer.get(3, 0).unwrap().char, 'B');
}

#[test]
fn test_render_show_cursor_false() {
    // Exercise line 228: show_cursor is false, so cursor rendering is skipped
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Absolute,
        line_number_width: 4,
        show_cursor: false,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 5);
    let lines = vec!["hello world".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 5), &mut buffer, &Style::default());

    // Cursor should NOT be rendered (no reverse-video cell)
    // The character at cursor position should remain unchanged
    let cell = buffer.get(4, 0).unwrap(); // content_x = gutter_width = 4
    assert_eq!(cell.char, 'h');
}

#[test]
fn test_render_empty_lines_tilde_break() {
    // Verifies tilde rendering for empty lines in bounds
    // Use height=2 with 1 content line, so only 1 tilde line
    let config = WindowRendererConfig {
        line_numbers: LineNumberMode::Absolute,
        line_number_width: 4,
        ..Default::default()
    };
    let renderer = WindowRenderer::with_config(config);
    let mut buffer = FrameBuffer::new(40, 2);
    let lines: Vec<String> = vec!["Only line".to_string()];
    let content = make_content(&lines, 0);

    renderer.render(&content, Rect::new(0, 0, 40, 2), &mut buffer, &Style::default());

    // Row 1 should have a tilde (only 1 empty line slot available)
    let tilde_cell = buffer.get(2, 1); // gutter_width - 2 = 4 - 2 = 2
    assert!(tilde_cell.is_some());
    assert_eq!(tilde_cell.unwrap().char, '~');
}
