use reovim_client_driver::{BufferId, BufferUpdateEvent, ClientModule, VirtualLinePosition};

use super::*;

fn make_buffer_event(buffer_id: usize, lines: Vec<String>) -> BufferUpdateEvent {
    let total = lines.len();
    BufferUpdateEvent {
        buffer_id: BufferId(buffer_id),
        revision: 1,
        changed_range: 0..total,
        new_lines: lines,
        total_lines: total,
    }
}

fn table_lines() -> Vec<String> {
    vec![
        "| Name  | Age |".to_string(),
        "| ----- | --- |".to_string(),
        "| Alice | 30  |".to_string(),
    ]
}

// =========================================================================
// Construction
// =========================================================================

#[test]
fn test_new_module() {
    let m = MarkdownModule::new();
    assert!(m.has_buffer_contrib());
    assert_eq!(m.kind(), "markdown");
    assert_eq!(m.name(), "Markdown");
}

#[test]
fn test_default_module() {
    let m = MarkdownModule::default();
    assert!(m.has_buffer_contrib());
}

// =========================================================================
// classify_token
// =========================================================================

#[test]
fn test_classify_token_delegates() {
    let m = MarkdownModule::new();
    assert!(m.classify_token("markup.heading.1").is_some());
    assert!(m.classify_token("keyword.function").is_none());
}

// =========================================================================
// on_buffer_update
// =========================================================================

#[test]
fn test_on_buffer_update_detects_tables() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    assert!(m.tables.contains_key(&1));
    assert_eq!(m.tables[&1].len(), 1);
    assert_eq!(m.active_buffer_id, Some(1));
}

#[test]
fn test_on_buffer_update_no_tables() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, vec!["# Hello".to_string(), "World".to_string()]);
    m.on_buffer_update(&event);

    assert!(m.tables.get(&1).unwrap().is_empty());
}

// =========================================================================
// virtual_lines
// =========================================================================

#[test]
fn test_virtual_lines_top_bottom_borders() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    let vlines = m.virtual_lines();
    assert_eq!(vlines.len(), 2);

    // Top border before the first row
    assert_eq!(vlines[0].buffer_line, 0);
    assert_eq!(vlines[0].position, VirtualLinePosition::Before);
    assert!(vlines[0].content.contains('\u{250C}')); // top-left corner

    // Bottom border after the last row
    assert_eq!(vlines[1].buffer_line, 2);
    assert_eq!(vlines[1].position, VirtualLinePosition::After);
    assert!(vlines[1].content.contains('\u{2514}')); // bottom-left corner
}

#[test]
fn test_virtual_lines_empty_without_tables() {
    let m = MarkdownModule::new();
    assert!(m.virtual_lines().is_empty());
}

// =========================================================================
// transform_line
// =========================================================================

#[test]
fn test_transform_line_header_centered() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    let result = m.transform_line(BufferId(1), 0, "| Name  | Age |");
    assert!(result.is_some());
    let line = result.unwrap();
    let full_text: String = line.segments.iter().map(|(t, _)| t.as_str()).collect();
    assert!(full_text.contains("Name"));
    assert!(full_text.contains('\u{2502}')); // vertical box drawing
}

#[test]
fn test_transform_line_delimiter_mid_border() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    let result = m.transform_line(BufferId(1), 1, "| ----- | --- |");
    assert!(result.is_some());
    let line = result.unwrap();
    let full_text: String = line.segments.iter().map(|(t, _)| t.as_str()).collect();
    assert!(full_text.contains('\u{251C}')); // left T
    assert!(full_text.contains('\u{253C}')); // cross
    assert!(full_text.contains('\u{2524}')); // right T
}

#[test]
fn test_transform_line_data_left_aligned() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    let result = m.transform_line(BufferId(1), 2, "| Alice | 30  |");
    assert!(result.is_some());
    let line = result.unwrap();
    let full_text: String = line.segments.iter().map(|(t, _)| t.as_str()).collect();
    assert!(full_text.contains("Alice"));
}

#[test]
fn test_transform_line_outside_table() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(
        1,
        vec!["# Hello".to_string(), String::new()]
            .into_iter()
            .chain(table_lines())
            .collect(),
    );
    m.on_buffer_update(&event);

    // Line 0 and 1 are not in the table
    assert!(m.transform_line(BufferId(1), 0, "# Hello").is_none());
    assert!(m.transform_line(BufferId(1), 1, "").is_none());
}

#[test]
fn test_transform_line_insert_mode_bypass() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    m.on_mode_change("insert");
    m.on_cursor_update(BufferId(1), 0, 0);

    // In insert mode, cursor line should show raw text
    assert!(m.transform_line(BufferId(1), 0, "| Name  | Age |").is_none());
    // Other lines still transform
    assert!(m.transform_line(BufferId(1), 2, "| Alice | 30  |").is_some());
}

#[test]
fn test_transform_line_normal_mode_cursor() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    m.on_mode_change("normal");
    m.on_cursor_update(BufferId(1), 0, 0);

    // Normal mode: cursor line still transforms
    assert!(m.transform_line(BufferId(1), 0, "| Name  | Age |").is_some());
}

#[test]
fn test_transform_line_border_chars_styled() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    let result = m.transform_line(BufferId(1), 0, "| Name  | Age |").unwrap();

    // Find segments with border style (DarkGrey)
    let has_border = result.segments.iter().any(|(_, style)| {
        style
            .as_ref()
            .is_some_and(|s| s.fg == Some(Color::DarkGrey))
    });
    assert!(has_border, "Should have border-styled segments");
}

// =========================================================================
// map_cursor_column
// =========================================================================

#[test]
fn test_map_cursor_column_in_table() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    // Map cursor at the first pipe position
    let result = m.map_cursor_column(BufferId(1), 0, 0);
    assert!(result.is_some());
}

#[test]
fn test_map_cursor_column_outside_table() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(
        1,
        vec!["# Hello".to_string()]
            .into_iter()
            .chain(table_lines())
            .collect(),
    );
    m.on_buffer_update(&event);

    assert!(m.map_cursor_column(BufferId(1), 0, 0).is_none());
}

#[test]
fn test_map_cursor_column_insert_mode_bypass() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    m.on_mode_change("insert");
    m.on_cursor_update(BufferId(1), 0, 0);

    assert!(m.map_cursor_column(BufferId(1), 0, 0).is_none());
}

#[test]
fn test_map_cursor_column_beyond_mapping() {
    let mut m = MarkdownModule::new();
    let event = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event);

    // Column beyond the line length
    assert!(m.map_cursor_column(BufferId(1), 0, 999).is_none());
}

// =========================================================================
// Multiple buffers
// =========================================================================

#[test]
fn test_multiple_buffers() {
    let mut m = MarkdownModule::new();
    let event1 = make_buffer_event(1, table_lines());
    m.on_buffer_update(&event1);

    let event2 = make_buffer_event(2, vec!["# No table".to_string()]);
    m.on_buffer_update(&event2);

    assert_eq!(m.tables[&1].len(), 1);
    assert!(m.tables[&2].is_empty());
}

// =========================================================================
// Lifecycle defaults
// =========================================================================

#[test]
fn tick_returns_false() {
    let mut m = MarkdownModule::new();
    assert!(!m.tick());
}

#[test]
fn cursor_position_none() {
    let m = MarkdownModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn has_annotations_false() {
    let m = MarkdownModule::new();
    assert!(!m.has_annotations());
}

#[test]
fn has_chrome_false() {
    let m = MarkdownModule::new();
    assert!(!m.has_chrome());
}
