use {
    super::*,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::CellCapability,
};

// Plan 23 / 17-β.2b-impl-c helpers.
fn has_content(g: &CellCapability) -> bool {
    g.iter().any(|(_, c)| c.ch != ' ')
}
fn char_at(g: &CellCapability, x: u16, y: u16) -> char {
    g.get_cell(x, y).map_or(' ', |c| c.ch)
}

// =============================================================================
// Helpers
// =============================================================================

fn active_payload(query: &str, items: &[&str]) -> String {
    let items_json: String = items
        .iter()
        .map(|d| format!(r#"{{"display":"{d}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"active":true,"query":"{query}","cursor":{cursor},"selected":0,"scrollOffset":0,"pickerTitle":"Files","prompt":"> ","totalCount":{total},"matchedCount":{matched},"items":[{items_json}]}}"#,
        cursor = query.len(),
        total = items.len(),
        matched = items.len(),
    )
}

fn render_module(module: &MicroscopeModule, w: u16, h: u16) -> CellCapability {
    let mut surface = CellCapability::new(w, h);
    let bounds = Rect::new(0, 0, w, h);
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = MicroscopeModule::new();
    assert_eq!(m.id(), "microscope");
    assert_eq!(m.kind(), "microscope");
    assert_eq!(m.name(), "Microscope");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = MicroscopeModule::default();
    assert!(!m.data.active);
}

// =============================================================================
// Chrome role tests
// =============================================================================

#[test]
fn chrome_role() {
    let m = MicroscopeModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 80);
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn lifecycle_exit_ok() {
    let mut m = MicroscopeModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("main", &["main.rs"]));
    assert!(m.data.active);
    assert_eq!(m.data.query, "main");
    assert_eq!(m.data.cursor, 4);
    assert_eq!(m.data.picker_title, "Files");
    assert_eq!(m.data.items.len(), 1);
    assert_eq!(m.data.total_count, 1);
}

#[test]
fn notification_deactivates() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("x", &[]));
    assert!(m.data.active);

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.data.active);
}

#[test]
fn notification_invalid_json() {
    let mut m = MicroscopeModule::new();
    m.on_notification("not json");
    assert!(!m.data.active);
}

#[test]
fn notification_items_parsing() {
    let mut m = MicroscopeModule::new();
    m.on_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerTitle":"F","prompt":"> ","items":[{"display":"a.rs","detail":"src/a.rs"},{"display":"b.rs"}],"totalCount":2,"matchedCount":2}"#,
    );
    assert_eq!(m.data.items.len(), 2);
    assert_eq!(m.data.items[0].detail.as_deref(), Some("src/a.rs"));
    assert!(m.data.items[1].detail.is_none());
}

#[test]
fn notification_preview_parsing() {
    let mut m = MicroscopeModule::new();
    m.on_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0,"preview":{"lines":["fn main()","{}"],"highlightLine":0}}"#,
    );
    assert!(m.data.preview.is_some());
    let preview = m.data.preview.as_ref().unwrap();
    assert_eq!(preview.lines.len(), 2);
    assert_eq!(preview.highlight_line, Some(0));
}

#[test]
fn notification_icon_parsing() {
    let mut m = MicroscopeModule::new();
    m.on_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerTitle":"F","prompt":"> ","items":[{"display":"main.rs","icon":"\ue7a8"},{"display":"readme.md"}],"totalCount":2,"matchedCount":2}"#,
    );
    assert_eq!(m.data.items.len(), 2);
    assert_eq!(m.data.items[0].icon.as_deref(), Some("\u{e7a8}"));
    assert!(m.data.items[1].icon.is_none());
}

// =============================================================================
// Cursor position tests
// =============================================================================

#[test]
fn cursor_position_inactive() {
    let m = MicroscopeModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_active() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("ab", &[]));
    let pos = m.cursor_position(80, 24);
    assert!(pos.is_some());
    let (x, _y) = pos.unwrap();
    // prompt "> " is 2 chars, cursor at position 2 => x = 0 + 2 + 2 = 4
    assert_eq!(x, 4);
}

#[test]
fn cursor_position_too_small_terminal() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("", &[]));
    // Very small screen cannot fit MIN_HEIGHT.
    let pos = m.cursor_position(10, 3);
    assert!(pos.is_none());
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = MicroscopeModule::new();
    let surface = render_module(&m, 80, 24);
    assert!(!has_content(&surface));
}

#[test]
fn render_shows_content() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("test", &["main.rs", "lib.rs"]));
    let surface = render_module(&m, 80, 24);
    assert!(has_content(&surface));
}

#[test]
fn render_selected_item_highlighted() {
    let mut m = MicroscopeModule::new();
    m.on_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerTitle":"F","prompt":"> ","items":[{"display":"first"},{"display":"second"}],"totalCount":2,"matchedCount":2}"#,
    );
    let surface = render_module(&m, 80, 24);
    // Verify content rendered (first item has '>' indicator).
    let bounds = LayoutBounds::calculate(80, 24);
    assert_eq!(char_at(&surface, 0, bounds.panel_start_y), '>');
    // Second item should have ' ' indicator.
    if bounds.panel_height > 1 {
        assert_eq!(char_at(&surface, 0, bounds.panel_start_y + 1), ' ');
    }
}

#[test]
fn render_too_small_screen() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("", &[]));
    let surface = render_module(&m, 10, 3);
    // Should not panic; small screen means nothing rendered.
    assert!(!has_content(&surface));
}
