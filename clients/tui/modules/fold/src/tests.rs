use reovim_client_driver::{BufferId, ClientModule};

use super::*;

// =========================================================================
// Construction and identity
// =========================================================================

#[test]
fn new_inactive() {
    let m = FoldModule::new();
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.id(), "range-finder-fold");
    assert_eq!(m.kind(), "range-finder-fold");
    assert_eq!(m.name(), "Range Finder Fold");
}

#[test]
fn default_inactive() {
    let m = FoldModule::default();
    assert!(!m.has_buffer_contrib());
}

#[test]
fn version() {
    let m = FoldModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

// =========================================================================
// on_notification
// =========================================================================

#[test]
fn notification_with_folds() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo() {"}]}}"#,
    );
    assert!(m.has_buffer_contrib());
    assert_eq!(m.folds.len(), 1);
    assert!(m.folds.contains_key(&1));
    assert_eq!(m.folds[&1][0].start_line, 5);
    assert_eq!(m.folds[&1][0].hidden_count, 3);
    assert_eq!(m.folds[&1][0].preview, "fn foo() {");
}

#[test]
fn notification_no_folds() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{}}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_missing_folds_key() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"active":true}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_invalid_json() {
    let mut m = FoldModule::new();
    m.on_notification("not json{{{");
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_replaces_previous() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":5,"preview":"a"}]}}"#);
    assert_eq!(m.folds.len(), 1);

    m.on_notification(r#"{"folds":{"2":[{"start_line":10,"hidden_count":2,"preview":"b"}]}}"#);
    assert_eq!(m.folds.len(), 1);
    assert!(m.folds.contains_key(&2));
    assert!(!m.folds.contains_key(&1));
}

#[test]
fn notification_missing_start_line() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"hidden_count":5,"preview":"a"}]}}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_missing_hidden_count() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":0,"preview":"a"}]}}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_missing_preview_defaults_empty() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":5}]}}"#);
    assert!(m.has_buffer_contrib());
    assert_eq!(m.folds[&1][0].preview, "");
}

#[test]
fn notification_multi_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"fn a"}],
            "2":[{"start_line":10,"hidden_count":5,"preview":"fn b"}]
        }}"#,
    );
    assert!(m.has_buffer_contrib());
    assert_eq!(m.folds.len(), 2);
}

#[test]
fn notification_non_array_value() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":"not an array"}}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn auto_selects_single_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"42":[{"start_line":0,"hidden_count":3,"preview":"fn a"}]}}"#,
    );
    assert_eq!(m.active_buffer_id, Some(42));
}

#[test]
fn notification_non_numeric_buffer_id_skipped() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"abc":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#,
    );
    assert!(!m.has_buffer_contrib());
}

// =========================================================================
// on_buffer_focus
// =========================================================================

#[test]
fn buffer_focus_sets_active() {
    let mut m = FoldModule::new();
    m.on_buffer_focus(BufferId(5));
    assert_eq!(m.active_buffer_id, Some(5));
}

#[test]
fn buffer_focus_rebuilds_ranges() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":10,"hidden_count":5,"preview":"b"}]
        }}"#,
    );
    // Multi-buffer: no auto-select
    m.active_buffer_id = None;
    m.rebuild_hidden_ranges();
    assert!(m.fold_ranges().is_empty());

    m.on_buffer_focus(BufferId(2));
    let ranges = m.fold_ranges();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0], (11, 5));
}

// =========================================================================
// fold_ranges
// =========================================================================

#[test]
fn fold_ranges_empty_when_inactive() {
    let m = FoldModule::new();
    assert!(m.fold_ranges().is_empty());
}

#[test]
fn fold_ranges_returns_hidden_ranges() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );
    let ranges = m.fold_ranges();
    assert_eq!(ranges.len(), 1);
    // Hidden lines start at start_line + 1 (fold marker line is visible)
    assert_eq!(ranges[0], (6, 3));
}

#[test]
fn fold_ranges_multiple_folds() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[
            {"start_line":2,"hidden_count":4,"preview":"a"},
            {"start_line":10,"hidden_count":2,"preview":"b"}
        ]}}"#,
    );
    let ranges = m.fold_ranges();
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0], (3, 4));
    assert_eq!(ranges[1], (11, 2));
}

#[test]
fn fold_ranges_zero_count_excluded() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":0,"preview":"empty"}]}}"#,
    );
    let ranges = m.fold_ranges();
    assert!(ranges.is_empty());
}

#[test]
fn fold_ranges_no_active_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
        }}"#,
    );
    m.active_buffer_id = None;
    m.rebuild_hidden_ranges();
    assert!(m.fold_ranges().is_empty());
}

#[test]
fn fold_ranges_wrong_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#);
    m.active_buffer_id = Some(999);
    m.rebuild_hidden_ranges();
    assert!(m.fold_ranges().is_empty());
}

#[test]
fn fold_ranges_cleared_on_deactivation() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"a"}]}}"#);
    assert!(!m.fold_ranges().is_empty());

    m.on_notification(r#"{"folds":{}}"#);
    assert!(m.fold_ranges().is_empty());
}

// =========================================================================
// transform_line (fold markers)
// =========================================================================

#[test]
fn transform_line_at_fold_start() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":6,"preview":"fn foo() {"}]}}"#,
    );

    let result = m.transform_line(BufferId(1), 5, "fn foo() {");
    assert!(result.is_some());
    let line = result.unwrap();
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].0, "--- 6 lines: fn foo() { ---");
    let style = line.segments[0].1.clone().unwrap();
    assert_eq!(style.fg, Some(Color::DarkGrey));
}

#[test]
fn transform_line_non_fold_line() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );

    let result = m.transform_line(BufferId(1), 3, "some other line");
    assert!(result.is_none());
}

#[test]
fn transform_line_inactive() {
    let m = FoldModule::new();
    let result = m.transform_line(BufferId(1), 0, "text");
    assert!(result.is_none());
}

#[test]
fn transform_line_no_active_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
        }}"#,
    );
    m.active_buffer_id = None;

    let result = m.transform_line(BufferId(1), 0, "text");
    assert!(result.is_none());
}

#[test]
fn transform_line_wrong_buffer() {
    let mut m = FoldModule::new();
    m.on_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#);
    m.active_buffer_id = Some(999);

    let result = m.transform_line(BufferId(999), 0, "text");
    assert!(result.is_none());
}

// =========================================================================
// Lifecycle defaults
// =========================================================================

#[test]
fn tick_returns_false() {
    let mut m = FoldModule::new();
    assert!(!m.tick());
}

#[test]
fn cursor_position_none() {
    let m = FoldModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn has_annotations_false() {
    let m = FoldModule::new();
    assert!(!m.has_annotations());
}

#[test]
fn has_chrome_false() {
    let m = FoldModule::new();
    assert!(!m.has_chrome());
}
