use {
    super::*,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::{CellCapability, CellStyle},
};

// Plan 23 helpers (17-β.2b-impl-c bulk migration).

fn char_at(g: &CellCapability, x: u16, y: u16) -> char {
    g.get_cell(x, y).map_or(' ', |c| c.ch)
}

fn style_at(g: &CellCapability, x: u16, y: u16) -> CellStyle {
    g.get_cell(x, y).map(|c| c.style).unwrap_or_default()
}

// =============================================================================
// Helpers
// =============================================================================

fn active_payload(label: &str) -> String {
    format!(
        r#"{{"active":true,"label":"{label}","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
    )
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

fn render(module: &SignatureHelpModule, w: u16, h: u16) -> CellCapability {
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
fn identity() {
    let m = SignatureHelpModule::new();
    assert_eq!(m.id(), "signature-help");
    assert_eq!(m.kind(), "signature-help");
    assert_eq!(m.name(), "SignatureHelp");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = SignatureHelpModule::default();
    assert!(!m.active);
}

#[test]
fn chrome_role() {
    let m = SignatureHelpModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 44);
}

#[test]
fn lifecycle() {
    let mut m = SignatureHelpModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo(x: i32)"));
    assert!(m.active);
    assert_eq!(m.label, "fn foo(x: i32)");
    assert_eq!(m.origin_line, 5);
    assert_eq!(m.origin_col, 10);
}

#[test]
fn notification_deactivates() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    m.on_notification(&inactive());
    assert!(!m.active);
    assert!(m.label.is_empty());
}

#[test]
fn notification_invalid_json() {
    let mut m = SignatureHelpModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_empty_label_deactivates() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":"","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

#[test]
fn notification_no_origin_keeps_defaults() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(r#"{"active":true,"label":"fn foo()"}"#);
    assert!(m.active);
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_no_label_defaults_empty() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

#[test]
fn notification_overwrites_previous() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("first"));
    m.on_notification(&active_payload("second"));
    assert_eq!(m.label, "second");
}

#[test]
fn notification_deactivation_clears_origin() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    m.on_notification(&inactive());
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_null_label() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":null,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = SignatureHelpModule::new();
    let surface = render(&m, 80, 24);
    assert_eq!(char_at(&surface, 0, 0), ' ');
}

#[test]
fn render_shows_popup_above_origin() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo(x: i32)"));
    let surface = render(&m, 80, 24);
    // Origin at line 5, popup_h=3 -> py = 5 - 3 = 2, px = 10
    assert_eq!(char_at(&surface, 10, 2), '\u{256D}');
}

#[test]
fn render_border_color_yellow() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    let surface = render(&m, 80, 24);
    assert_eq!(
        style_at(&surface, 10, 2).fg,
        Some(reovim_ext_client_tui_cap_cell::CellColor::Named(11))
    );
}

#[test]
fn render_content_text() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    let surface = render(&m, 80, 24);
    // Content at (px+1, py+1) = (11, 3)
    assert_eq!(char_at(&surface, 11, 3), 'f');
    assert_eq!(
        style_at(&surface, 11, 3).fg,
        Some(reovim_ext_client_tui_cap_cell::CellColor::Named(15))
    );
}

#[test]
fn render_popup_below_when_no_room_above() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":"fn foo()","origin":{"BufferPosition":{"buffer_id":1,"line":1,"col":0}}}"#,
    );
    let surface = render(&m, 80, 24);
    // anchor_y=1, popup_h=3: 1 < 3, try below: 1+1+3=5 <= 24 -> py=2
    assert_eq!(char_at(&surface, 0, 2), '\u{256D}');
}

#[test]
fn render_popup_at_top_fallback() {
    let mut m = SignatureHelpModule::new();
    m.active = true;
    m.label = "fn foo()".to_owned();
    m.origin_line = 1;
    m.origin_col = 0;
    // Tiny terminal
    let surface = render(&m, 80, 4);
    // anchor_y=1: above needs 3, below needs 1+1+3=5 > 4. Fallback to 0.
    assert_eq!(char_at(&surface, 0, 0), '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":"fn f()","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );
    let surface = render(&m, 80, 24);
    // popup_w = 10 (min), max_x = 80 - 10 = 70. col 75 > 70.
    assert_eq!(char_at(&surface, 70, 2), '\u{256D}');
}

// =============================================================================
// Type coverage
// =============================================================================

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
    let p = SignatureHelpPayload {
        active: true,
        label: Some("test".into()),
        origin: None,
    };
    assert!(format!("{p:?}").contains("SignatureHelpPayload"));
}
