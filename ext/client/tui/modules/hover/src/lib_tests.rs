use {
    super::*,
    markdown::StyledSpan,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::{CellCapability, CellStyle},
};

// Plan 23 / 17-β.2b-impl-c bulk migration helpers.

fn char_at(g: &CellCapability, x: u16, y: u16) -> char {
    g.get_cell(x, y).map_or(' ', |c| c.ch)
}

fn style_at(g: &CellCapability, x: u16, y: u16) -> CellStyle {
    g.get_cell(x, y).map(|c| c.style).unwrap_or_default()
}

/// Concatenate all span texts in a styled line into a single string.
fn line_text(spans: &[StyledSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Create a single plain-text styled line for direct assignment to `styled_lines`.
fn plain_line(text: &str) -> Vec<StyledSpan> {
    vec![StyledSpan {
        text: text.to_owned(),
        style: Style::new().fg(Color::White),
    }]
}

// =============================================================================
// Helpers
// =============================================================================

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

fn render_hover(module: &HoverModule, w: u16, h: u16) -> CellCapability {
    let mut surface = CellCapability::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn hover_identity() {
    let m = HoverModule::new();
    assert_eq!(m.id(), "hover");
    assert_eq!(m.kind(), "hover");
    assert_eq!(m.name(), "Hover");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn hover_default() {
    let m = HoverModule::default();
    assert!(!m.active);
}

#[test]
fn hover_chrome_role() {
    let m = HoverModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 45);
}

#[test]
fn hover_lifecycle() {
    let mut m = HoverModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates_plaintext() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo() -> bool"));
    assert!(m.active);
    assert_eq!(m.styled_lines.len(), 1);
    assert_eq!(line_text(&m.styled_lines[0]), "fn foo() -> bool");
    assert_eq!(m.content_type, ContentType::Plaintext);
    assert_eq!(m.origin_line, 5);
    assert_eq!(m.origin_col, 10);
}

#[test]
fn notification_activates_markdown() {
    let mut m = HoverModule::new();
    m.on_notification(&active_markdown("**bold**"));
    assert!(m.active);
    assert_eq!(m.content_type, ContentType::Markdown);
    assert_eq!(m.origin_line, 3);
}

#[test]
fn notification_deactivates() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    m.on_notification(&inactive());
    assert!(!m.active);
    assert!(m.styled_lines.is_empty());
}

#[test]
fn notification_invalid_json() {
    let mut m = HoverModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_empty_content_deactivates() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

#[test]
fn notification_multiline() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"line1\nline2\nline3","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert_eq!(m.styled_lines.len(), 3);
}

#[test]
fn notification_defaults_content_type() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"hello","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert_eq!(m.content_type, ContentType::Plaintext);
}

#[test]
fn notification_no_origin_keeps_defaults() {
    let mut m = HoverModule::new();
    m.on_notification(r#"{"active":true,"content":"hello","contentType":"plaintext"}"#);
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_overwrites() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("first"));
    m.on_notification(&active_markdown("second"));
    assert_eq!(line_text(&m.styled_lines[0]), "second");
    assert_eq!(m.content_type, ContentType::Markdown);
}

#[test]
fn notification_caps_at_max_lines() {
    let mut m = HoverModule::new();
    let many_lines: Vec<&str> = (0..30).map(|_| "line").collect();
    let content = many_lines.join("\\n");
    let data = format!(
        r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":0,"col":0}}}}}}"#
    );
    m.on_notification(&data);
    assert_eq!(m.styled_lines.len(), MAX_LINES);
}

#[test]
fn notification_deactivation_clears_origin() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    m.on_notification(&inactive());
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_null_content() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":null,"contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

// =============================================================================
// popup_width tests
// =============================================================================

#[test]
fn popup_width_min() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(80), MIN_WIDTH);
}

#[test]
fn popup_width_adapts_to_content() {
    let mut m = HoverModule::new();
    m.styled_lines = vec![plain_line(&"a".repeat(30))];
    assert_eq!(m.popup_width(80), 34);
}

#[test]
fn popup_width_clamped_to_max() {
    let mut m = HoverModule::new();
    m.styled_lines = vec![plain_line(&"a".repeat(200))];
    assert_eq!(m.popup_width(80), 48);
}

#[test]
fn popup_width_narrow_terminal() {
    let mut m = HoverModule::new();
    m.styled_lines = vec![plain_line("hello")];
    assert_eq!(m.popup_width(22), MIN_WIDTH);
}

#[test]
fn popup_width_zero() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(0), 0);
}

#[test]
fn popup_width_tiny() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(5), 5);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = HoverModule::new();
    let surface = render_hover(&m, 80, 24);
    assert_eq!(char_at(&surface, 0, 0), ' ');
}

#[test]
fn render_single_line() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo()"));
    let surface = render_hover(&m, 80, 24);
    // Origin line=5, col=10. Popup at (10, 6)
    assert_eq!(char_at(&surface, 10, 6), '\u{256D}');
}

#[test]
fn render_border_color_plaintext() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    let surface = render_hover(&m, 80, 24);
    assert_eq!(style_at(&surface, 10, 6).fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(7)));
}

#[test]
fn render_border_color_markdown() {
    let mut m = HoverModule::new();
    m.on_notification(&active_markdown("**bold**"));
    let surface = render_hover(&m, 80, 24);
    // Origin line=3, col=0. Popup at (0, 4)
    assert_eq!(style_at(&surface, 0, 4).fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(14)));
}

#[test]
fn render_content_text() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    let surface = render_hover(&m, 80, 24);
    // Content at (11, 7)
    assert_eq!(char_at(&surface, 11, 7), 'h');
    assert_eq!(style_at(&surface, 11, 7).fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(15)));
}

#[test]
fn render_multiline() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"line1\nline2","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":0}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // popup_h = 4, at y=6, bottom border at y=9
    assert_eq!(char_at(&surface, 0, 9), '\u{2570}');
}

#[test]
fn render_popup_above_when_no_room_below() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"text","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":22,"col":0}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // anchor_y=22, 22+1+3=26>24, above: 22-3=19
    assert_eq!(char_at(&surface, 0, 19), '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"hello","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // popup_w=20, max_x=60, origin_col=75>60
    assert_eq!(char_at(&surface, 60, 6), '\u{256D}');
}

// =============================================================================
// Type coverage
// =============================================================================

#[test]
fn content_type_debug_clone_eq() {
    let a = ContentType::Plaintext;
    let b = a;
    #[allow(clippy::clone_on_copy)]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(ContentType::Plaintext, ContentType::Markdown);
    assert!(format!("{a:?}").contains("Plaintext"));
}

#[test]
fn origin_debug_clone() {
    let o = Origin::BufferPosition {
        buffer_id: 1,
        line: 2,
        col: 3,
    };
    assert!(format!("{o:?}").contains("BufferPosition"));
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

// =============================================================================
// on_cursor_update tests (#662)
// =============================================================================

#[test]
fn cursor_update_dismisses_when_moved() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo()"));
    assert!(m.active);

    // Cursor moved to a different line
    m.on_cursor_update(BufferId(0), 10, 5);
    assert!(!m.active);
    assert!(m.styled_lines.is_empty());
}

#[test]
fn cursor_update_keeps_popup_at_origin() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo()"));
    assert!(m.active);

    // Cursor at same origin (line=5, col=10)
    m.on_cursor_update(BufferId(0), 5, 10);
    assert!(m.active);
    assert!(!m.styled_lines.is_empty());
}

#[test]
fn cursor_update_inactive_is_noop() {
    let mut m = HoverModule::new();
    assert!(!m.active);

    m.on_cursor_update(BufferId(0), 100, 200);
    assert!(!m.active);
}

#[test]
fn cursor_update_same_line_different_col_dismisses() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo()"));
    assert!(m.active);

    // Same line but different column
    m.on_cursor_update(BufferId(0), 5, 15);
    assert!(!m.active);
}

// =============================================================================
// Crash regression tests
// =============================================================================

/// Regression: hover markdown with horizontal rule (`---`) creates non-ASCII
/// content (box-drawing character `─`). The render truncation logic used
/// byte-offset slicing (`&span.text[..available]`), which panics when the
/// offset falls inside a multi-byte character.
#[test]
fn render_horizontal_rule_no_panic() {
    let mut m = HoverModule::new();
    // Markdown with `---` triggers horizontal rule rendering → `─` characters
    m.on_notification(&active_markdown("docs\\n---\\nsignature"));
    assert!(m.active);

    // The `─` character (U+2500) is 3 bytes in UTF-8.
    // With a narrow terminal, the truncation path triggers and would panic
    // if slicing at a non-char-boundary.
    // Width=30: popup_w ~20, content_w ~18, available ~18. Horizontal rule
    // text is 120 bytes / 40 chars. Byte offset 18 is NOT a char boundary
    // (char boundaries are at 0,3,6,9,12,15,18 — 18 IS a boundary for this
    // case, but let's use width=31 where available=19, which is NOT).
    let surface = render_hover(&m, 31, 24);
    // Verify no panic — render completed successfully
    let _ = surface;
}

/// Same bug but with explicit non-ASCII styled content.
#[test]
fn render_multibyte_truncation_no_panic() {
    let mut m = HoverModule::new();
    m.active = true;
    m.origin_line = 0;
    m.origin_col = 0;
    m.content_type = ContentType::Markdown;
    // Directly inject a styled line with multi-byte characters
    // Each `─` is 3 bytes. 20 chars = 60 bytes.
    let rule = "\u{2500}".repeat(20);
    m.styled_lines = vec![vec![StyledSpan {
        text: rule,
        style: Style::new().fg(Color::White),
    }]];

    // Render with narrow width where truncation happens.
    // content_w will be ~12. 12 is a multiple of 3 (char boundary).
    // Try width=15: popup_w=15, content_w=13. 13 is NOT a char boundary.
    let surface = render_hover(&m, 15, 10);
    let _ = surface;
}
