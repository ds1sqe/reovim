use {super::*, reovim_driver_display::FrameBuffer};

// =========================================================================
// Construction and kind
// =========================================================================

#[test]
fn test_new_inactive() {
    let ext = RangeFinderJumpExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "range-finder-jump");
}

#[test]
fn test_default_inactive() {
    let ext = RangeFinderJumpExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn test_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(RangeFinderJumpExtension::new());
    assert_eq!(ext.kind(), "range-finder-jump");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification
// =========================================================================

#[test]
fn test_apply_notification_activates() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":5,"label":"s"}]}"#);
    assert!(ext.is_active());
    assert_eq!(ext.labels.len(), 1);
    assert_eq!(ext.labels[0].line, 0);
    assert_eq!(ext.labels[0].col, 5);
    assert_eq!(ext.labels[0].label, "s");
}

#[test]
fn test_apply_notification_deactivates() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
    assert!(ext.is_active());

    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_invalid_json() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification("not valid json{{{");
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_missing_active() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"matches":[]}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_missing_matches() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true}"#);
    assert!(ext.is_active());
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_empty_matches() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[]}"#);
    assert!(ext.is_active());
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_multiple_matches() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"matches":[
            {"line":0,"col":0,"label":"s"},
            {"line":1,"col":3,"label":"f"},
            {"line":5,"col":10,"label":"n"}
        ]}"#,
    );
    assert_eq!(ext.labels.len(), 3);
    assert_eq!(ext.labels[1].line, 1);
    assert_eq!(ext.labels[1].col, 3);
    assert_eq!(ext.labels[1].label, "f");
}

#[test]
fn test_apply_notification_match_missing_line() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"col":0,"label":"s"}]}"#);
    // Match skipped due to missing "line"
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_match_missing_col() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"label":"s"}]}"#);
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_match_missing_label() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0}]}"#);
    assert!(ext.labels.is_empty());
}

#[test]
fn test_apply_notification_replaces_previous() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
    assert_eq!(ext.labels.len(), 1);

    ext.apply_notification(
        r#"{"active":true,"matches":[
            {"line":1,"col":1,"label":"f"},
            {"line":2,"col":2,"label":"n"}
        ]}"#,
    );
    assert_eq!(ext.labels.len(), 2);
    assert_eq!(ext.labels[0].label, "f");
}

// =========================================================================
// render / render_with_viewport
// =========================================================================

#[test]
fn test_render_no_op_without_viewport() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
    let mut fb = FrameBuffer::new(20, 10);
    ext.render(&mut fb);
    // render() is a no-op for jump labels (needs viewport)
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_labels_at_positions() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"matches":[
            {"line":0,"col":3,"label":"s"},
            {"line":2,"col":0,"label":"f"}
        ]}"#,
    );

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    // Label "s" at screen (4+3=7, 0)
    assert_eq!(fb.get(7, 0).unwrap().char, 's');
    assert_eq!(fb.get(7, 0).unwrap().style.bg, Some(Color::Yellow));

    // Label "f" at screen (4+0=4, 2)
    assert_eq!(fb.get(4, 2).unwrap().char, 'f');
}

#[test]
fn test_render_labels_outside_viewport_skipped_above() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":5,"col":0,"label":"s"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 10, // line 5 is above viewport
        content_x: 4,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    // Nothing rendered
    assert_eq!(fb.get(4, 0).unwrap().char, ' ');
}

#[test]
fn test_render_labels_outside_viewport_skipped_below() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":30,"col":0,"label":"s"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 20, // only 20 rows, line 30 is below
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    assert_eq!(fb.get(4, 0).unwrap().char, ' ');
}

#[test]
fn test_render_two_char_labels() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"sf"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 2,
        content_height: 10,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);

    // First char at (2, 0) with bright style
    assert_eq!(fb.get(2, 0).unwrap().char, 's');
    assert_eq!(fb.get(2, 0).unwrap().style.bg, Some(Color::Yellow));

    // Second char at (3, 0) with dim style
    assert_eq!(fb.get(3, 0).unwrap().char, 'f');
    assert_ne!(fb.get(3, 0).unwrap().style.bg, Some(Color::Yellow));
}

#[test]
fn test_render_empty_label_no_op() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":""}]}"#);

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 10,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);

    // Empty label renders nothing
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_inactive_no_op() {
    let ext = RangeFinderJumpExtension::new();
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_active_empty_matches_no_op() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[]}"#);
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_with_scroll_offset() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":15,"col":5,"label":"s"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 10, // line 15 is at screen row 5
        content_x: 3,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(40, 20);
    ext.render_with_viewport(&mut fb, &viewport);

    // screen_y = 15 - 10 = 5, screen_x = 3 + 5 = 8
    assert_eq!(fb.get(8, 5).unwrap().char, 's');
}

#[test]
fn test_render_at_viewport_boundary() {
    let mut ext = RangeFinderJumpExtension::new();
    // Label at exactly the last visible row
    ext.apply_notification(r#"{"active":true,"matches":[{"line":9,"col":0,"label":"s"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 10, // rows 0-9 visible
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);

    // Line 9 = screen row 9, which is < content_height (10)
    assert_eq!(fb.get(0, 9).unwrap().char, 's');
}

#[test]
fn test_render_just_past_viewport_boundary() {
    let mut ext = RangeFinderJumpExtension::new();
    ext.apply_notification(r#"{"active":true,"matches":[{"line":10,"col":0,"label":"s"}]}"#);

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 10, // rows 0-9 visible, line 10 is out
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 11);
    ext.render_with_viewport(&mut fb, &viewport);

    // Line 10 = screen row 10, which is >= content_height (10)
    assert_eq!(fb.get(0, 10).unwrap().char, ' ');
}

// =========================================================================
// Style helpers
// =========================================================================

#[test]
fn test_label_style() {
    let style = label_style();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Yellow));
}

#[test]
fn test_label_dim_style() {
    let style = label_dim_style();
    assert!(style.fg.is_some());
    assert!(style.bg.is_some());
    // Dim style should differ from bright style
    assert_ne!(style.bg, Some(Color::Yellow));
}

// =========================================================================
// Default trait methods
// =========================================================================

#[test]
fn test_default_tick() {
    let mut ext = RangeFinderJumpExtension::new();
    assert!(!ext.tick());
}

#[test]
fn test_default_cursor_position() {
    let ext = RangeFinderJumpExtension::new();
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn test_default_content_offset_left() {
    let ext = RangeFinderJumpExtension::new();
    assert_eq!(ext.content_offset_left(), 0);
}
