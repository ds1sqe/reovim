use {super::*, reovim_driver_display::FrameBuffer};

// =========================================================================
// Construction and kind
// =========================================================================

#[test]
fn test_fold_new_inactive() {
    let ext = RangeFinderFoldExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "range-finder-fold");
}

#[test]
fn test_fold_default_inactive() {
    let ext = RangeFinderFoldExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn test_fold_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(RangeFinderFoldExtension::new());
    assert_eq!(ext.kind(), "range-finder-fold");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification
// =========================================================================

#[test]
fn test_fold_apply_notification_with_folds() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo() {"}]}}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.folds.len(), 1);
    assert!(ext.folds.contains_key("1"));
    assert_eq!(ext.folds["1"][0].start_line, 5);
    assert_eq!(ext.folds["1"][0].hidden_count, 3);
    assert_eq!(ext.folds["1"][0].preview, "fn foo() {");
}

#[test]
fn test_fold_apply_notification_no_folds() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{}}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_fold_apply_notification_missing_folds_key() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"active":true}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_fold_apply_notification_invalid_json() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification("not json{{{");
    assert!(!ext.is_active());
}

#[test]
fn test_fold_apply_notification_replaces_previous() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":5,"preview":"a"}]}}"#);
    assert_eq!(ext.folds.len(), 1);

    ext.apply_notification(r#"{"folds":{"2":[{"start_line":10,"hidden_count":2,"preview":"b"}]}}"#);
    assert_eq!(ext.folds.len(), 1);
    assert!(ext.folds.contains_key("2"));
    assert!(!ext.folds.contains_key("1"));
}

#[test]
fn test_fold_apply_notification_missing_start_line() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"hidden_count":5,"preview":"a"}]}}"#);
    // Entry skipped, no folds parsed
    assert!(!ext.is_active());
}

#[test]
fn test_fold_apply_notification_missing_hidden_count() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"preview":"a"}]}}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_fold_apply_notification_missing_preview_defaults_empty() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":5}]}}"#);
    assert!(ext.is_active());
    assert_eq!(ext.folds["1"][0].preview, "");
}

#[test]
fn test_fold_apply_notification_multi_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"fn a"}],
            "2":[{"start_line":10,"hidden_count":5,"preview":"fn b"}]
        }}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.folds.len(), 2);
}

#[test]
fn test_fold_apply_notification_non_array_value() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":"not an array"}}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_fold_auto_selects_single_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"42":[{"start_line":0,"hidden_count":3,"preview":"fn a"}]}}"#,
    );
    assert_eq!(ext.active_buffer_id.as_deref(), Some("42"));
}

#[test]
fn test_fold_set_active_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.set_active_buffer("5");
    assert_eq!(ext.active_buffer_id.as_deref(), Some("5"));
}

// =========================================================================
// render / render_with_viewport
// =========================================================================

#[test]
fn test_fold_render_no_op_without_viewport() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );
    let mut fb = FrameBuffer::new(40, 10);
    ext.render(&mut fb);
    // render() is a no-op
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_fold_render_marker() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":2,"hidden_count":6,"preview":"fn foo() {"}]}}"#,
    );

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(60, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    // Marker at row 2, starting at content_x=4
    // Should contain "--- 6 lines: fn foo() { ---"
    assert_eq!(fb.get(4, 2).unwrap().char, '-');
    assert_eq!(fb.get(5, 2).unwrap().char, '-');
    assert_eq!(fb.get(6, 2).unwrap().char, '-');
    assert_eq!(fb.get(7, 2).unwrap().char, ' ');
    assert_eq!(fb.get(8, 2).unwrap().char, '6');
}

#[test]
fn test_fold_render_outside_viewport_above() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":2,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );

    let viewport = ViewportContext {
        scroll_top: 10,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    // Nothing rendered (line 2 is above viewport at scroll_top=10)
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_fold_render_outside_viewport_below() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":30,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_fold_render_inactive_no_op() {
    let ext = RangeFinderFoldExtension::new();
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_fold_render_no_active_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
        }}"#,
    );
    // Multi-buffer: auto-select doesn't apply
    ext.active_buffer_id = None;

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    // No buffer selected, nothing rendered
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_fold_render_wrong_buffer_id() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#);
    ext.active_buffer_id = Some("999".to_string());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

// =========================================================================
// Style helpers
// =========================================================================

#[test]
fn test_fold_marker_style() {
    let style = fold_marker_style();
    assert_eq!(style.fg, Some(Color::DarkGrey));
}

// =========================================================================
// Default trait methods
// =========================================================================

#[test]
fn test_fold_default_tick() {
    let mut ext = RangeFinderFoldExtension::new();
    assert!(!ext.tick());
}

#[test]
fn test_fold_default_cursor_position() {
    let ext = RangeFinderFoldExtension::new();
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn test_fold_default_content_offset_left() {
    let ext = RangeFinderFoldExtension::new();
    assert_eq!(ext.content_offset_left(), 0);
}

// =========================================================================
// fold_hidden_lines
// =========================================================================

#[test]
fn test_fold_hidden_lines_empty_when_inactive() {
    let ext = RangeFinderFoldExtension::new();
    assert!(ext.fold_hidden_lines().is_empty());
}

#[test]
fn test_fold_hidden_lines_returns_ranges() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo"}]}}"#,
    );
    let ranges = ext.fold_hidden_lines();
    assert_eq!(ranges.len(), 1);
    // Hidden lines start at start_line + 1 (fold marker line is visible)
    assert_eq!(ranges[0], (6, 3));
}

#[test]
fn test_fold_hidden_lines_multiple_folds() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[
            {"start_line":2,"hidden_count":4,"preview":"a"},
            {"start_line":10,"hidden_count":2,"preview":"b"}
        ]}}"#,
    );
    let ranges = ext.fold_hidden_lines();
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0], (3, 4));
    assert_eq!(ranges[1], (11, 2));
}

#[test]
fn test_fold_hidden_lines_zero_count_excluded() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{"1":[{"start_line":5,"hidden_count":0,"preview":"empty"}]}}"#,
    );
    let ranges = ext.fold_hidden_lines();
    assert!(ranges.is_empty());
}

#[test]
fn test_fold_hidden_lines_no_active_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
        }}"#,
    );
    // Multi-buffer: auto-select doesn't apply
    ext.active_buffer_id = None;
    ext.rebuild_hidden_ranges();
    assert!(ext.fold_hidden_lines().is_empty());
}

#[test]
fn test_fold_hidden_lines_wrong_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#);
    ext.active_buffer_id = Some("999".to_string());
    ext.rebuild_hidden_ranges();
    assert!(ext.fold_hidden_lines().is_empty());
}

#[test]
fn test_fold_hidden_lines_updated_on_set_active_buffer() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(
        r#"{"folds":{
            "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
            "2":[{"start_line":10,"hidden_count":5,"preview":"b"}]
        }}"#,
    );
    // Initially no active buffer (multi-buffer, no auto-select)
    ext.active_buffer_id = None;
    ext.rebuild_hidden_ranges();
    assert!(ext.fold_hidden_lines().is_empty());

    ext.set_active_buffer("2");
    let ranges = ext.fold_hidden_lines();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0], (11, 5));
}

#[test]
fn test_fold_hidden_lines_cleared_on_deactivation() {
    let mut ext = RangeFinderFoldExtension::new();
    ext.apply_notification(r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"a"}]}}"#);
    assert!(!ext.fold_hidden_lines().is_empty());

    // Deactivate with empty folds
    ext.apply_notification(r#"{"folds":{}}"#);
    assert!(ext.fold_hidden_lines().is_empty());
}
