use {
    super::*,
    reovim_client_driver::testing::{MockPlatformCapabilities, RecordingSurface},
};

// =============================================================================
// Helpers
// =============================================================================

fn render(module: &CompletionModule, w: u16, h: u16) -> RecordingSurface {
    let mut surface = RecordingSurface::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
}

fn single_item_payload() -> String {
    r#"{"active":true,"selected":0,"scrollOffset":0,"items":[{"label":"foo","kindAbbrev":"fn","sourceId":"lsp"}]}"#.to_owned()
}

fn multi_item_payload() -> String {
    r#"{"active":true,"selected":1,"scrollOffset":0,"items":[{"label":"foo","kindAbbrev":"fn","sourceId":"lsp"},{"label":"bar","kindAbbrev":"va","sourceId":"buffer"},{"label":"baz","kindAbbrev":"kw","sourceId":""}]}"#.to_owned()
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = CompletionModule::new();
    assert_eq!(m.id(), "completion");
    assert_eq!(m.kind(), "completion");
    assert_eq!(m.name(), "Completion");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = CompletionModule::default();
    assert!(!m.active);
}

#[test]
fn chrome_role() {
    let m = CompletionModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 70);
}

#[test]
fn lifecycle() {
    let mut m = CompletionModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let mut m = CompletionModule::new();
    m.on_notification(&single_item_payload());
    assert!(m.active);
    assert_eq!(m.items.len(), 1);
    assert_eq!(m.items[0].label, "foo");
    assert_eq!(m.items[0].kind_abbrev, "fn");
    assert_eq!(m.items[0].source_id, "lsp");
}

#[test]
fn notification_deactivates() {
    let mut m = CompletionModule::new();
    m.on_notification(&single_item_payload());
    m.on_notification(&inactive());
    assert!(!m.active);
    assert!(m.items.is_empty());
}

#[test]
fn notification_multiple_items() {
    let mut m = CompletionModule::new();
    m.on_notification(&multi_item_payload());
    assert_eq!(m.items.len(), 3);
    assert_eq!(m.selected, 1);
}

#[test]
fn notification_invalid_json() {
    let mut m = CompletionModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_item_without_label_skipped() {
    let mut m = CompletionModule::new();
    m.on_notification(
        r#"{"active":true,"selected":0,"scrollOffset":0,"items":[{"kindAbbrev":"fn"},{"label":"bar"}]}"#,
    );
    assert_eq!(m.items.len(), 1);
    assert_eq!(m.items[0].label, "bar");
}

#[test]
fn notification_defaults_kind_abbrev() {
    let mut m = CompletionModule::new();
    m.on_notification(r#"{"active":true,"selected":0,"scrollOffset":0,"items":[{"label":"foo"}]}"#);
    assert_eq!(m.items[0].kind_abbrev, "tx");
}

// =============================================================================
// calculate_popup_width tests
// =============================================================================

#[test]
fn calculate_popup_width_min() {
    let m = CompletionModule::new();
    assert_eq!(m.calculate_popup_width(80), 20);
}

#[test]
fn calculate_popup_width_adapts() {
    let mut m = CompletionModule::new();
    m.on_notification(&single_item_payload());
    // "fn" (2) + " " (1) + "foo" (3) + "  " (2) + "lsp" (3) = 11 + 4 = 15 -> clamped to 20
    assert_eq!(m.calculate_popup_width(80), 20);
}

#[test]
fn calculate_popup_width_clamped() {
    let mut m = CompletionModule::new();
    m.items = vec![CompletionRow {
        label: "a".repeat(200),
        kind_abbrev: "fn".to_owned(),
        kind_icon: String::new(),
        source_id: "lsp".to_owned(),
    }];
    assert_eq!(m.calculate_popup_width(80), 76);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = CompletionModule::new();
    let surface = render(&m, 80, 24);
    assert!(!surface.has_content());
}

#[test]
fn render_shows_popup() {
    let mut m = CompletionModule::new();
    m.on_notification(&single_item_payload());
    let surface = render(&m, 80, 24);
    assert!(surface.has_content());
}

#[test]
fn render_selected_item_highlighted() {
    let mut m = CompletionModule::new();
    m.on_notification(&multi_item_payload());
    let surface = render(&m, 80, 24);

    // popup at center-bottom
    let popup_width = m.calculate_popup_width(80);
    let px = (80 - popup_width) / 2;
    let py = 24u16.saturating_sub(3 + 2).saturating_sub(1); // 3 items + 2 border - 1

    // Selected item (index 1) should have Blue bg
    let row_y = py + 2; // py + 1 (border) + 1 (second item)
    assert_eq!(surface.style_at(px + 1, row_y).bg, Some(Color::Blue));
}

#[test]
fn render_kind_abbrev_yellow() {
    let mut m = CompletionModule::new();
    m.on_notification(&single_item_payload());
    let surface = render(&m, 80, 24);

    let popup_width = m.calculate_popup_width(80);
    let px = (80 - popup_width) / 2;
    let py = 24u16.saturating_sub(1 + 2).saturating_sub(1);

    // Kind abbrev at (px+1, py+1) should be yellow
    assert_eq!(surface.style_at(px + 1, py + 1).fg, Some(Color::Yellow));
}

// =============================================================================
// Type coverage
// =============================================================================

#[test]
fn completion_row_debug_clone() {
    let row = CompletionRow {
        label: "foo".to_owned(),
        kind_abbrev: "fn".to_owned(),
        kind_icon: "\u{f0295}".to_owned(),
        source_id: "lsp".to_owned(),
    };
    assert!(format!("{row:?}").contains("CompletionRow"));
    #[allow(clippy::redundant_clone)]
    let c = row.clone();
    assert_eq!(c.label, "foo");
}
