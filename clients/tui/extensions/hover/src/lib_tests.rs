use reovim_driver_display::FrameBuffer;

use super::*;

// =========================================================================
// Helpers
// =========================================================================

fn active_plaintext(content: &str) -> String {
    format!(
        r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
    )
}

fn active_markdown(content: &str) -> String {
    format!(
        r#"{{"active":true,"content":"{content}","contentType":"markdown","origin":{{"BufferPosition":{{"buffer_id":2,"line":3,"col":0}}}}}}"#
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
    let ext = HoverExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "hover");
}

#[test]
fn default_is_inactive() {
    let ext = HoverExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(HoverExtension::new());
    assert_eq!(ext.kind(), "hover");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification tests
// =========================================================================

#[test]
fn apply_activates_plaintext() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("fn foo() -> bool"));
    assert!(ext.is_active());
    assert_eq!(ext.lines.len(), 1);
    assert_eq!(ext.lines[0], "fn foo() -> bool");
    assert_eq!(ext.content_type, ContentType::Plaintext);
    assert_eq!(ext.origin_line, 5);
    assert_eq!(ext.origin_col, 10);
}

#[test]
fn apply_activates_markdown() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_markdown("**bold**"));
    assert!(ext.is_active());
    assert_eq!(ext.content_type, ContentType::Markdown);
    assert_eq!(ext.origin_line, 3);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_deactivates() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("hello"));
    assert!(ext.is_active());

    ext.apply_notification(&inactive());
    assert!(!ext.is_active());
    assert!(ext.lines.is_empty());
}

#[test]
fn apply_invalid_json() {
    let mut ext = HoverExtension::new();
    ext.apply_notification("not json{{{");
    assert!(!ext.is_active());
}

#[test]
fn apply_empty_content_deactivates() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(
        r#"{"active":true,"content":"","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!ext.is_active());
}

#[test]
fn apply_multiline_content() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(
        r#"{"active":true,"content":"line1\nline2\nline3","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.lines.len(), 3);
    assert_eq!(ext.lines[0], "line1");
    assert_eq!(ext.lines[2], "line3");
}

#[test]
fn apply_defaults_content_type() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(
        r#"{"active":true,"content":"hello","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.content_type, ContentType::Plaintext);
}

#[test]
fn apply_no_origin_keeps_defaults() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(r#"{"active":true,"content":"hello","contentType":"plaintext"}"#);
    assert!(ext.is_active());
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_overwrites_previous() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("first"));
    assert_eq!(ext.lines[0], "first");

    ext.apply_notification(&active_markdown("second"));
    assert_eq!(ext.lines[0], "second");
    assert_eq!(ext.content_type, ContentType::Markdown);
}

#[test]
fn apply_caps_at_max_lines() {
    let mut ext = HoverExtension::new();
    let many_lines: Vec<&str> = (0..30).map(|_| "line").collect();
    let content = many_lines.join("\\n");
    let data = format!(
        r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":0,"col":0}}}}}}"#
    );
    ext.apply_notification(&data);
    assert!(ext.is_active());
    assert_eq!(ext.lines.len(), MAX_LINES);
}

#[test]
fn apply_deactivation_clears_origin() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("hello"));
    assert_eq!(ext.origin_line, 5);
    assert_eq!(ext.origin_col, 10);

    ext.apply_notification(&inactive());
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_reactivation_without_origin_uses_defaults() {
    let mut ext = HoverExtension::new();
    // First activation sets origin to (5, 10).
    ext.apply_notification(&active_plaintext("first"));
    assert_eq!(ext.origin_line, 5);

    // Deactivate clears origin.
    ext.apply_notification(&inactive());

    // Re-activate without origin field — uses default (0, 0) not stale (5, 10).
    ext.apply_notification(r#"{"active":true,"content":"second","contentType":"plaintext"}"#);
    assert_eq!(ext.origin_line, 0);
    assert_eq!(ext.origin_col, 0);
}

#[test]
fn apply_null_content() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(
        r#"{"active":true,"content":null,"contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    // null content → empty → deactivated.
    assert!(!ext.is_active());
}

// =========================================================================
// popup_width tests
// =========================================================================

#[test]
fn popup_width_min() {
    let ext = HoverExtension::new();
    // No lines → max line length is 0, desired = 4, clamped to MIN_WIDTH.
    assert_eq!(ext.popup_width(80), MIN_WIDTH);
}

#[test]
fn popup_width_adapts_to_content() {
    let mut ext = HoverExtension::new();
    ext.lines = vec!["a".repeat(30)];
    // desired = 30 + 4 = 34
    let w = ext.popup_width(80);
    assert_eq!(w, 34);
}

#[test]
fn popup_width_clamped_to_max() {
    let mut ext = HoverExtension::new();
    ext.lines = vec!["a".repeat(200)];
    let w = ext.popup_width(80);
    // max_width = 0.6 * 80 = 48
    assert_eq!(w, 48);
}

#[test]
fn popup_width_narrow_terminal() {
    let mut ext = HoverExtension::new();
    ext.lines = vec!["hello".to_owned()];
    let w = ext.popup_width(22);
    assert_eq!(w, MIN_WIDTH);
}

#[test]
fn popup_width_zero_terminal() {
    let ext = HoverExtension::new();
    // Zero-width terminal should not panic.
    assert_eq!(ext.popup_width(0), 0);
}

#[test]
fn popup_width_tiny_terminal() {
    let ext = HoverExtension::new();
    assert_eq!(ext.popup_width(5), 5);
}

// =========================================================================
// render tests
// =========================================================================

#[test]
fn render_empty_no_op() {
    let ext = HoverExtension::new();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn render_single_line() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("fn foo()"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Origin at line 5, col 10 → popup at (10, 6) below origin.
    // Top-left corner at (10, 6)
    assert_eq!(fb.get(10, 6).unwrap().char, '\u{256D}');
}

#[test]
fn render_border_color_plaintext() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("hello"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let cell = fb.get(10, 6).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Grey));
}

#[test]
fn render_border_color_markdown() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_markdown("**bold**"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Origin at line 3, col 0 → popup at (0, 4)
    let cell = fb.get(0, 4).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Cyan));
}

#[test]
fn render_content_text() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(&active_plaintext("hello"));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Content at (11, 7) — px+1, py+1
    let cell = fb.get(11, 7).unwrap();
    assert_eq!(cell.char, 'h');
    assert_eq!(cell.style.fg, Some(Color::White));
}

#[test]
fn render_multiline() {
    let mut ext = HoverExtension::new();
    ext.apply_notification(
        r#"{"active":true,"content":"line1\nline2","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":0}}}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // popup_h = 2 + 2 = 4, positioned at y=6
    // Bottom border at y=6+3=9
    assert_eq!(fb.get(0, 9).unwrap().char, '\u{2570}');
}

#[test]
fn render_popup_above_when_no_room_below() {
    let mut ext = HoverExtension::new();
    // Origin near bottom of screen.
    ext.apply_notification(
        r#"{"active":true,"content":"text","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":22,"col":0}}}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // popup_h = 3 (1 line + 2 border). anchor_y = 22.
    // 22 + 1 + 3 = 26 > 24, so try above: 22 - 3 = 19.
    assert_eq!(fb.get(0, 19).unwrap().char, '\u{256D}');
}

#[test]
fn render_popup_at_top_when_no_room_either_way() {
    let mut ext = HoverExtension::new();
    // Small terminal, origin at line 1, long content.
    ext.lines = (0..5).map(|i| format!("line {i}")).collect();
    ext.active = true;
    ext.origin_line = 1;
    ext.origin_col = 0;
    ext.content_type = ContentType::Plaintext;

    // Terminal height 5 — popup_h = 7 (5 lines + 2 border).
    // Below: 1 + 1 + 7 = 9 > 5. Above: 1 < 7. Fallback to y=0.
    let mut fb = FrameBuffer::new(80, 5);
    ext.render(&mut fb);

    assert_eq!(fb.get(0, 0).unwrap().char, '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut ext = HoverExtension::new();
    // Origin col far right — popup should clamp x so it fits.
    ext.apply_notification(
        r#"{"active":true,"content":"hello","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // popup_w = 20 (min), so max_x = 80 - 20 = 60. origin_col = 75 > 60.
    assert_eq!(fb.get(60, 6).unwrap().char, '\u{256D}');
}

#[test]
fn render_long_line_truncated() {
    let mut ext = HoverExtension::new();
    let long = "A".repeat(100);
    ext.apply_notification(&active_plaintext(&long));

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Content should not overflow the popup border.
    let popup_w = ext.popup_width(80);
    let px: u16 = 10; // origin_col
    let border_x = px + popup_w - 1;
    assert_ne!(fb.get(border_x, 7).unwrap().char, 'A');
}

// =========================================================================
// Deserialize type coverage
// =========================================================================

#[test]
fn content_type_debug() {
    let ct = ContentType::Markdown;
    assert!(format!("{ct:?}").contains("Markdown"));
}

#[test]
fn content_type_clone_copy_eq() {
    let a = ContentType::Plaintext;
    let b = a;
    #[allow(clippy::clone_on_copy)]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(ContentType::Plaintext, ContentType::Markdown);
}

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
    let p = HoverPayload {
        active: true,
        content: Some("test".into()),
        content_type: Some(ContentType::Plaintext),
        origin: None,
    };
    assert!(format!("{p:?}").contains("HoverPayload"));
}
