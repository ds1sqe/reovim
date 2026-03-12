use super::*;

#[test]
fn test_new_extension() {
    let ext = MarkdownRenderExtension::new();
    assert_eq!(ext.kind(), "markdown");
    assert!(ext.is_active());
    assert!(ext.virtual_lines().is_empty());
}

#[test]
fn test_default_extension() {
    let ext = MarkdownRenderExtension::default();
    assert_eq!(ext.kind(), "markdown");
}

#[test]
fn test_classify_token_delegates() {
    let ext = MarkdownRenderExtension::new();
    assert!(ext.classify_token("markup.heading.1").is_some());
    assert!(ext.classify_token("keyword").is_none());
}

#[test]
fn test_on_buffer_update_detects_tables() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);
    assert!(!ext.virtual_lines().is_empty());
    assert_eq!(ext.virtual_lines().len(), 2); // top + bottom border
}

#[test]
fn test_on_buffer_update_no_tables() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec!["hello".to_string(), "world".to_string()];
    ext.on_buffer_update(1, &lines);
    assert!(ext.virtual_lines().is_empty());
}

#[test]
fn test_virtual_lines_top_bottom_borders() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    let vlines = ext.virtual_lines();
    assert_eq!(vlines.len(), 2);

    // Top border before first table row
    assert_eq!(vlines[0].buffer_line, 0);
    assert_eq!(vlines[0].position, VirtualLinePosition::Before);
    assert!(vlines[0].content.starts_with('┌'));

    // Bottom border after last table row
    assert_eq!(vlines[1].buffer_line, 2);
    assert_eq!(vlines[1].position, VirtualLinePosition::After);
    assert!(vlines[1].content.starts_with('└'));
}

#[test]
fn test_transform_line_header_centered() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    let result = ext.transform_line(1, 0, "| A | B |");
    assert!(result.is_some());
    let transformed = result.unwrap();
    assert!(transformed.text.starts_with('│'));
    assert!(transformed.text.ends_with('│'));
}

#[test]
fn test_transform_line_delimiter_mid_border() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    let result = ext.transform_line(1, 1, "|---|---|");
    assert!(result.is_some());
    let transformed = result.unwrap();
    assert!(transformed.text.starts_with('├'));
    assert!(transformed.text.contains('┼'));
    assert!(transformed.text.ends_with('┤'));
}

#[test]
fn test_transform_line_data_left_aligned() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    let result = ext.transform_line(1, 2, "| 1 | 2 |");
    assert!(result.is_some());
    let transformed = result.unwrap();
    assert!(transformed.text.starts_with('│'));
    assert!(transformed.text.contains(" 1 "));
}

#[test]
fn test_transform_line_outside_table() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "hello".to_string(),
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    assert!(ext.transform_line(1, 0, "hello").is_none());
}

#[test]
fn test_transform_line_insert_mode_bypass() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    // Enable insert mode and set cursor to line 0
    ext.on_mode_change("INSERT", true);
    ext.on_cursor_update(1, 0, 0);

    // Should return None (raw text shown in insert mode)
    assert!(ext.transform_line(1, 0, "| A | B |").is_none());
}

#[test]
fn test_transform_line_normal_mode_cursor() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    // Normal mode on cursor line — still transforms
    ext.on_mode_change("NORMAL", false);
    ext.on_cursor_update(1, 0, 0);
    assert!(ext.transform_line(1, 0, "| A | B |").is_some());
}

#[test]
fn test_transform_line_styles_border_chars() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    let result = ext.transform_line(1, 0, "| A | B |").unwrap();
    // First char is '│' — should have a border style (DarkGrey)
    assert!(result.styles[0].is_some());
    let border_style = result.styles[0].as_ref().unwrap();
    assert_eq!(border_style.fg, Some(Color::DarkGrey));
}

#[test]
fn test_map_cursor_column_outside_table() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec!["hello".to_string()];
    ext.on_buffer_update(1, &lines);
    assert!(ext.map_cursor_column(1, 0, 3).is_none());
}

#[test]
fn test_map_cursor_column_insert_mode_bypass() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);
    ext.on_mode_change("INSERT", true);
    ext.on_cursor_update(1, 0, 0);
    assert!(ext.map_cursor_column(1, 0, 3).is_none());
}

#[test]
fn test_map_cursor_column_with_stored_lines() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    // Pipe at buffer col 0 should map to visual col 0
    let result = ext.map_cursor_column(1, 0, 0);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_map_cursor_column_pipe_maps_to_border() {
    let mut ext = MarkdownRenderExtension::new();
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines);

    // Middle pipe at buffer col 4 should map to a visual border position
    let result = ext.map_cursor_column(1, 0, 4);
    assert!(result.is_some());
    // Visual pipe position depends on col_widths — just verify it maps
    assert!(result.unwrap() > 0);
}

#[test]
fn test_on_cursor_update() {
    let mut ext = MarkdownRenderExtension::new();
    ext.on_cursor_update(1, 5, 10);
    assert_eq!(ext.cursor_line, Some(5));
}

#[test]
fn test_on_mode_change() {
    let mut ext = MarkdownRenderExtension::new();
    ext.on_mode_change("INSERT", true);
    assert!(ext.is_insert);
    ext.on_mode_change("NORMAL", false);
    assert!(!ext.is_insert);
}

#[test]
fn test_apply_notification_noop() {
    let mut ext = MarkdownRenderExtension::new();
    ext.apply_notification("anything");
    // Should not change state
    assert!(ext.is_active());
}

#[test]
fn test_multiple_buffers() {
    let mut ext = MarkdownRenderExtension::new();

    let lines1 = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    ext.on_buffer_update(1, &lines1);

    let lines2 = vec!["no tables".to_string()];
    ext.on_buffer_update(2, &lines2);

    // Buffer 1 has tables
    assert!(ext.transform_line(1, 0, "| A | B |").is_some());
    // Buffer 2 has no tables
    assert!(ext.transform_line(2, 0, "no tables").is_none());
}
