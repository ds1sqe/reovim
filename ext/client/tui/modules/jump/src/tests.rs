use reovim_client_driver::{BufferId, ClientModule};

use super::*;

// =========================================================================
// Construction and identity
// =========================================================================

#[test]
fn new_inactive() {
    let m = JumpModule::new();
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.id(), "range-finder-jump");
    assert_eq!(m.kind(), "range-finder-jump");
    assert_eq!(m.name(), "Range Finder Jump");
}

#[test]
fn default_inactive() {
    let m = JumpModule::default();
    assert!(!m.has_buffer_contrib());
}

#[test]
fn version() {
    let m = JumpModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

// =========================================================================
// on_notification
// =========================================================================

#[test]
fn notification_activates() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":5,"label":"s"}]}"#);
    assert!(m.has_buffer_contrib());
    assert_eq!(m.labels.len(), 1);
    assert_eq!(m.labels[0].line, 0);
    assert_eq!(m.labels[0].col, 5);
    assert_eq!(m.labels[0].label, "s");
}

#[test]
fn notification_deactivates() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
    assert!(m.has_buffer_contrib());

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.has_buffer_contrib());
    assert!(m.labels.is_empty());
}

#[test]
fn notification_invalid_json() {
    let mut m = JumpModule::new();
    m.on_notification("not valid json{{{");
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_missing_active() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"matches":[]}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_missing_matches() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true}"#);
    assert!(m.has_buffer_contrib());
    assert!(m.labels.is_empty());
}

#[test]
fn notification_empty_matches() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[]}"#);
    assert!(m.has_buffer_contrib());
    assert!(m.labels.is_empty());
}

#[test]
fn notification_multiple_matches() {
    let mut m = JumpModule::new();
    m.on_notification(
        r#"{"active":true,"matches":[
            {"line":0,"col":0,"label":"s"},
            {"line":1,"col":3,"label":"f"},
            {"line":5,"col":10,"label":"n"}
        ]}"#,
    );
    assert_eq!(m.labels.len(), 3);
    assert_eq!(m.labels[1].line, 1);
    assert_eq!(m.labels[1].col, 3);
    assert_eq!(m.labels[1].label, "f");
}

#[test]
fn notification_match_missing_line() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"col":0,"label":"s"}]}"#);
    assert!(m.labels.is_empty());
}

#[test]
fn notification_match_missing_col() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"label":"s"}]}"#);
    assert!(m.labels.is_empty());
}

#[test]
fn notification_match_missing_label() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0}]}"#);
    assert!(m.labels.is_empty());
}

#[test]
fn notification_replaces_previous() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
    assert_eq!(m.labels.len(), 1);

    m.on_notification(
        r#"{"active":true,"matches":[
            {"line":1,"col":1,"label":"f"},
            {"line":2,"col":2,"label":"n"}
        ]}"#,
    );
    assert_eq!(m.labels.len(), 2);
    assert_eq!(m.labels[0].label, "f");
}

// =========================================================================
// transform_line
// =========================================================================

#[test]
fn transform_single_char_label() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":3,"label":"s"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "fn hello()");
    assert!(result.is_some());
    let line = result.unwrap();
    // Segments: "fn " (none), "s" (bright), "ello()" (none)
    assert_eq!(line.segments.len(), 3);
    assert_eq!(line.segments[0].0, "fn ");
    assert!(line.segments[0].1.is_none());
    assert_eq!(line.segments[1].0, "s");
    assert!(line.segments[1].1.is_some());
    assert_eq!(line.segments[2].0, "ello()");
    assert!(line.segments[2].1.is_none());
}

#[test]
fn transform_two_char_label() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"sf"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "hello");
    assert!(result.is_some());
    let line = result.unwrap();
    // Segments: "s" (bright), "f" (dim), "llo" (none)
    assert_eq!(line.segments.len(), 3);
    assert_eq!(line.segments[0].0, "s");
    let bright = line.segments[0].1.clone().unwrap();
    assert_eq!(bright.fg, Some(Color::Black));
    assert_eq!(bright.bg, Some(Color::Yellow));
    assert_eq!(line.segments[1].0, "f");
    let dim = line.segments[1].1.clone().unwrap();
    assert_ne!(dim.bg, Some(Color::Yellow)); // dimmed
    assert_eq!(line.segments[2].0, "llo");
}

#[test]
fn transform_no_labels_on_line() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":5,"col":0,"label":"s"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "hello");
    assert!(result.is_none());
}

#[test]
fn transform_inactive() {
    let m = JumpModule::new();
    let result = m.transform_line(BufferId(0), 0, "hello");
    assert!(result.is_none());
}

#[test]
fn transform_label_at_start() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "hello");
    let line = result.unwrap();
    // Segments: "s" (bright), "ello" (none)
    assert_eq!(line.segments.len(), 2);
    assert_eq!(line.segments[0].0, "s");
    assert!(line.segments[0].1.is_some());
    assert_eq!(line.segments[1].0, "ello");
}

#[test]
fn transform_label_at_end() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":4,"label":"s"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "hello");
    let line = result.unwrap();
    // Segments: "hell" (none), "s" (bright)
    assert_eq!(line.segments.len(), 2);
    assert_eq!(line.segments[0].0, "hell");
    assert_eq!(line.segments[1].0, "s");
}

#[test]
fn transform_multiple_labels_same_line() {
    let mut m = JumpModule::new();
    m.on_notification(
        r#"{"active":true,"matches":[
            {"line":0,"col":0,"label":"a"},
            {"line":0,"col":5,"label":"b"}
        ]}"#,
    );

    let result = m.transform_line(BufferId(0), 0, "hello world");
    let line = result.unwrap();
    // Segments: "a" (bright), "ello" (none), "b" (bright), "world" (none)
    assert_eq!(line.segments.len(), 4);
    assert_eq!(line.segments[0].0, "a");
    assert_eq!(line.segments[1].0, "ello");
    assert_eq!(line.segments[2].0, "b");
    assert_eq!(line.segments[3].0, "world");
}

#[test]
fn transform_empty_label() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":""}]}"#);

    // Empty label produces no label segments, but labels exist so transform is attempted
    let result = m.transform_line(BufferId(0), 0, "hello");
    let line = result.unwrap();
    // Only the remaining text segment
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].0, "hello");
}

#[test]
fn transform_label_beyond_text() {
    let mut m = JumpModule::new();
    m.on_notification(r#"{"active":true,"matches":[{"line":0,"col":100,"label":"s"}]}"#);

    let result = m.transform_line(BufferId(0), 0, "hello");
    let line = result.unwrap();
    // Label at col 100 is beyond "hello" (len 5), skipped
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].0, "hello");
}

// =========================================================================
// Lifecycle defaults
// =========================================================================

#[test]
fn tick_returns_false() {
    let mut m = JumpModule::new();
    assert!(!m.tick());
}

#[test]
fn cursor_position_none() {
    let m = JumpModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn has_annotations_false() {
    let m = JumpModule::new();
    assert!(!m.has_annotations());
}

#[test]
fn has_chrome_false() {
    let m = JumpModule::new();
    assert!(!m.has_chrome());
}

// =========================================================================
// Style helpers
// =========================================================================

#[test]
fn label_style_colors() {
    let style = label_style();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Yellow));
}

#[test]
fn label_dim_style_colors() {
    let style = label_dim_style();
    assert!(style.fg.is_some());
    assert!(style.bg.is_some());
    assert_ne!(style.bg, Some(Color::Yellow));
}
