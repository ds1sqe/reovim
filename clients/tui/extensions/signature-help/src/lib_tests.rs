use reovim_driver_display::FrameBuffer;

use super::*;

// =========================================================================
// Helpers
// =========================================================================

fn active_payload(label: &str) -> String {
    format!(
        r#"{{"active":true,"label":"{label}","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
    )
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

// =========================================================================
// Basic trait tests
// =========================================================================

#[test]
fn new_is_inactive() {
    let ext = SignatureHelpExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "signature-help");
}

#[test]
fn default_is_inactive() {
    let ext = SignatureHelpExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(SignatureHelpExtension::new());
    assert_eq!(ext.kind(), "signature-help");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification tests
// =========================================================================

#[test]
fn apply_activates() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo(x: i32)"));
    assert!(ext.is_active());
    assert_eq!(ext.label, "fn foo(x: i32)");
    assert_eq!(ext.origin_line, 5);
    assert_eq!(ext.origin_col, 10);
}

#[test]
fn apply_deactivates() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo()"));
    assert!(ext.is_active());

    ext.apply_notification(&inactive());
    assert!(!ext.is_active());
    assert!(ext.label.is_empty());
}

#[test]
fn apply_invalid_json() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification("not json{{{");
    assert!(!ext.is_active());
}

#[test]
fn apply_empty_label_deactivates() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"label":"","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!ext.is_active());
}

#[test]
fn apply_no_origin_keeps_defaults() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(r#"{"active":true,"label":"fn foo()"}"#);
    assert!(ext.is_active());
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_no_label_defaults_empty() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    // Empty label → deactivated.
    assert!(!ext.is_active());
}

#[test]
fn apply_overwrites_previous() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("first"));
    ext.apply_notification(&active_payload("second"));
    assert_eq!(ext.label, "second");
}

#[test]
fn apply_deactivation_clears_origin() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo()"));
    assert_eq!(ext.origin_line, 5);
    assert_eq!(ext.origin_col, 10);

    ext.apply_notification(&inactive());
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_reactivation_without_origin_uses_defaults() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("first"));
    ext.apply_notification(&inactive());

    // Re-activate without origin — uses default (0, 0) not stale (5, 10).
    ext.apply_notification(r#"{"active":true,"label":"second"}"#);
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_null_label() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"label":null,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    // null label → empty → deactivated.
    assert!(!ext.is_active());
}

// =========================================================================
// render tests
// =========================================================================

#[test]
fn render_empty_no_op() {
    let ext = SignatureHelpExtension::new();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn render_shows_popup_above_origin() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo(x: i32)"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Origin at line 5, popup_h=3 → py = 5 - 3 = 2
    // px = 10 (origin_col)
    assert_eq!(fb.get(10, 2).unwrap().char, '\u{256D}');
}

#[test]
fn render_border_color_yellow() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo()"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let cell = fb.get(10, 2).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Yellow));
}

#[test]
fn render_content_text() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(&active_payload("fn foo()"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Content at (px+1, py+1) = (11, 3)
    let cell = fb.get(11, 3).unwrap();
    assert_eq!(cell.char, 'f');
    assert_eq!(cell.style.fg, Some(Color::White));
}

#[test]
fn render_popup_below_when_no_room_above() {
    let mut ext = SignatureHelpExtension::new();
    // Origin at line 1 → not enough room above (need 3 rows).
    ext.apply_notification(
        r#"{"active":true,"label":"fn foo()","origin":{"BufferPosition":{"buffer_id":1,"line":1,"col":0}}}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // anchor_y=1, popup_h=3: 1 < 3, try below: 1+1+3=5 <= 24 → py=2
    assert_eq!(fb.get(0, 2).unwrap().char, '\u{256D}');
}

#[test]
fn render_popup_at_top_fallback() {
    let mut ext = SignatureHelpExtension::new();
    ext.label = "fn foo()".to_owned();
    ext.active = true;
    ext.origin_line = 1;
    ext.origin_col = 0;

    // Tiny terminal — no room above or below.
    let mut fb = FrameBuffer::new(80, 4);
    ext.render(&mut fb);

    // anchor_y=1: above needs 3, below needs 1+1+3=5 > 4. Fallback to 0.
    assert_eq!(fb.get(0, 0).unwrap().char, '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut ext = SignatureHelpExtension::new();
    ext.apply_notification(
        r#"{"active":true,"label":"fn f()","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // popup_w = 10 (min), max_x = 80 - 10 = 70. col 75 > 70.
    assert_eq!(fb.get(70, 2).unwrap().char, '\u{256D}');
}

#[test]
fn render_long_label_truncated() {
    let mut ext = SignatureHelpExtension::new();
    let long_label = "A".repeat(200);
    ext.apply_notification(&active_payload(&long_label));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // popup_w clamped to 78 (width - 2). px = min(10, 80-78) = 2.
    // Border right edge at px + popup_w - 1 = 2 + 77 = 79.
    // Content should not overflow into border cell.
    let cell = fb.get(79, 3).unwrap();
    assert_ne!(cell.char, 'A');
}

// =========================================================================
// Deserialize type coverage
// =========================================================================

#[test]
fn origin_debug() {
    let o = Origin::BufferPosition {
        buffer_id: 1,
        line: 2,
        col: 3,
    };
    assert!(format!("{o:?}").contains("BufferPosition"));
}

#[test]
fn origin_clone() {
    let o = Origin::BufferPosition {
        buffer_id: 1,
        line: 2,
        col: 3,
    };
    #[allow(clippy::redundant_clone)]
    let c = o.clone();
    let Origin::BufferPosition { line, .. } = c;
    assert_eq!(line, 2);
}

#[test]
fn payload_debug() {
    let p = SignatureHelpPayload {
        active: true,
        label: Some("test".into()),
        origin: None,
    };
    assert!(format!("{p:?}").contains("SignatureHelpPayload"));
}
