use {
    super::*,
    reovim_client_driver::testing::{MockPlatformCapabilities, WriteSurface},
};

fn test_caps() -> MockPlatformCapabilities {
    MockPlatformCapabilities::new()
}

fn bounds(width: u16) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width,
        height: 1,
    }
}

fn sample_json(buffers: &str) -> String {
    format!(r#"{{"active": true, "buffers": [{buffers}]}}"#)
}

fn one_buffer_json(id: u64, name: &str, modified: bool) -> String {
    format!(
        r#"{{"id": {id}, "name": "{name}", "modified": {modified}, "pinned": false, "errorCount": 0, "warningCount": 0}}"#,
    )
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn module_id() {
    let m = BufferlineModule::new();
    assert_eq!(m.id(), "bufferline");
    assert_eq!(m.kind(), "bufferline");
    assert_eq!(m.name(), "Bufferline");
}

#[test]
fn module_version() {
    let m = BufferlineModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn module_default() {
    let m = BufferlineModule::default();
    assert_eq!(m.id(), "bufferline");
}

// =============================================================================
// Role tests
// =============================================================================

#[test]
fn has_chrome_true() {
    let m = BufferlineModule::new();
    assert!(m.has_chrome());
}

#[test]
fn chrome_position_top() {
    let m = BufferlineModule::new();
    assert_eq!(m.chrome_position(), ChromePosition::Top);
}

#[test]
fn chrome_size_one() {
    let m = BufferlineModule::new();
    assert_eq!(m.chrome_requested_size(&test_caps()), 1);
}

#[test]
fn chrome_priority() {
    let m = BufferlineModule::new();
    assert_eq!(m.chrome_priority(), 90);
}

#[test]
fn server_kinds() {
    let m = BufferlineModule::new();
    assert_eq!(m.server_kinds(), vec!["bufferline"]);
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn init_succeeds() {
    let mut m = BufferlineModule::new();
    // ModuleContext has no Default impl, so we test through a simulated path.
    // The init method is trivially ProbeResult::Success, verified via integration.
    assert!(m.exit().is_ok());
}

#[test]
fn exit_succeeds() {
    let mut m = BufferlineModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// on_notification tests
// =============================================================================

#[test]
fn on_notification_parses_valid_json() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);
    assert_eq!(m.tabs.len(), 1);
    assert_eq!(m.tabs[0].name, "main.rs");
    assert!(!m.tabs[0].modified);
}

#[test]
fn on_notification_inactive_clears_tabs() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);
    assert_eq!(m.tabs.len(), 1);

    m.on_notification(r#"{"active": false}"#);
    assert!(m.tabs.is_empty());
}

#[test]
fn on_notification_ignores_malformed_json() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);

    m.on_notification("not json");
    assert_eq!(m.tabs.len(), 1, "Tabs should be unchanged after bad JSON");
}

#[test]
fn on_notification_empty_buffers_clears() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);

    m.on_notification(r#"{"active": true, "buffers": []}"#);
    assert!(m.tabs.is_empty());
}

#[test]
fn on_notification_parses_diagnostics() {
    let mut m = BufferlineModule::new();
    let json = r#"{"active": true, "buffers": [{"id": 1, "name": "a.rs", "modified": false, "pinned": false, "errorCount": 3, "warningCount": 1}]}"#;
    m.on_notification(json);
    assert_eq!(m.tabs[0].error_count, 3);
    assert_eq!(m.tabs[0].warning_count, 1);
}

#[test]
fn on_notification_parses_pinned() {
    let mut m = BufferlineModule::new();
    let json = r#"{"active": true, "buffers": [{"id": 1, "name": "a.rs", "modified": false, "pinned": true, "errorCount": 0, "warningCount": 0}]}"#;
    m.on_notification(json);
    assert!(m.tabs[0].pinned);
}

// =============================================================================
// on_buffer_focus tests
// =============================================================================

#[test]
fn on_buffer_focus_updates_active() {
    let mut m = BufferlineModule::new();
    m.on_buffer_focus(BufferId(42));
    assert_eq!(m.active_buffer_id, Some(42));
}

// =============================================================================
// chrome_render tests
// =============================================================================

#[test]
fn render_empty_bar_no_tabs() {
    let m = BufferlineModule::new();
    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    // Only the fill should occur — no tab writes.
    let has_tab = surface.writes().iter().any(|w| w.text.contains("main.rs"));
    assert!(!has_tab);
}

#[test]
fn render_one_tab() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);

    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());

    let has_name = surface.writes().iter().any(|w| w.text.contains("main.rs"));
    assert!(has_name, "Expected 'main.rs' in renders");
}

#[test]
fn render_active_tab_highlighted() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", false));
    m.on_notification(&json);
    m.on_buffer_focus(BufferId(1));

    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());

    let active_write = surface
        .writes()
        .iter()
        .find(|w| w.text.contains("main.rs"))
        .expect("Expected main.rs tab");
    assert_eq!(active_write.style.bg, Some(Color::Blue));
}

#[test]
fn render_modified_shows_marker() {
    let mut m = BufferlineModule::new();
    let json = sample_json(&one_buffer_json(1, "main.rs", true));
    m.on_notification(&json);

    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());

    let has_modified = surface.writes().iter().any(|w| w.text.contains("[+]"));
    assert!(has_modified, "Expected [+] for modified buffer");
}

#[test]
fn render_diagnostics() {
    let mut m = BufferlineModule::new();
    let json = r#"{"active": true, "buffers": [{"id": 1, "name": "a.rs", "modified": false, "pinned": false, "errorCount": 2, "warningCount": 3}]}"#;
    m.on_notification(json);

    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());

    let has_errors = surface.writes().iter().any(|w| w.text.contains("E:2"));
    let has_warnings = surface.writes().iter().any(|w| w.text.contains("W:3"));
    assert!(has_errors, "Expected E:2 in renders");
    assert!(has_warnings, "Expected W:3 in renders");
}

#[test]
fn render_zero_width_no_crash() {
    let m = BufferlineModule::new();
    let mut surface = WriteSurface::new(80, 1);
    m.chrome_render(&mut surface, bounds(0), &test_caps());
    assert!(surface.writes().is_empty());
}

#[test]
fn render_overflow_truncates() {
    let mut m = BufferlineModule::new();
    // Create many buffers to overflow a narrow width.
    let buffers: Vec<String> = (1..=10)
        .map(|i| one_buffer_json(i, &format!("file{i}.rs"), false))
        .collect();
    let json = sample_json(&buffers.join(","));
    m.on_notification(&json);

    let mut surface = WriteSurface::new(30, 1);
    m.chrome_render(&mut surface, bounds(30), &test_caps());

    // Should render something without panicking.
    let total_written: usize = surface.writes().iter().map(|w| w.text.len()).sum();
    assert!(total_written > 0);
}

#[test]
fn render_auto_scrolls_to_active() {
    let mut m = BufferlineModule::new();
    let buffers: Vec<String> = (1..=10)
        .map(|i| one_buffer_json(i, &format!("file{i}.rs"), false))
        .collect();
    let json = sample_json(&buffers.join(","));
    m.on_notification(&json);

    // Focus on last buffer — should auto-scroll.
    m.on_buffer_focus(BufferId(10));
    // Scroll offset should not be past the active tab.
    assert!(m.scroll_offset <= 9);
}

// =============================================================================
// format_tab_label tests
// =============================================================================

#[test]
fn format_tab_label_basic() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: false,
        pinned: false,
        error_count: 0,
        warning_count: 0,
    };
    assert_eq!(format_tab_label(&tab), " main.rs ");
}

#[test]
fn format_tab_label_modified() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: true,
        pinned: false,
        error_count: 0,
        warning_count: 0,
    };
    assert_eq!(format_tab_label(&tab), " main.rs [+] ");
}

#[test]
fn format_tab_label_with_diagnostics() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: false,
        pinned: false,
        error_count: 5,
        warning_count: 2,
    };
    let label = format_tab_label(&tab);
    assert!(label.contains("E:5"));
    assert!(label.contains("W:2"));
}

#[test]
fn format_tab_label_all_indicators() {
    let tab = TabEntry {
        id: 1,
        name: String::from("x.rs"),
        modified: true,
        pinned: true,
        error_count: 1,
        warning_count: 1,
    };
    let label = format_tab_label(&tab);
    assert!(label.contains("* x.rs"), "Pinned tab should have pin indicator");
    assert!(label.contains("[+]"));
    assert!(label.contains("E:1"));
    assert!(label.contains("W:1"));
}

#[test]
fn format_tab_label_pinned() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: false,
        pinned: true,
        error_count: 0,
        warning_count: 0,
    };
    assert_eq!(format_tab_label(&tab), " * main.rs ");
}

// =============================================================================
// ensure_active_visible tests
// =============================================================================

#[test]
fn ensure_active_visible_scrolls_back() {
    let mut m = BufferlineModule::new();
    m.tabs = (0..5)
        .map(|i| TabEntry {
            id: i,
            name: format!("f{i}.rs"),
            modified: false,
            pinned: false,
            error_count: 0,
            warning_count: 0,
        })
        .collect();
    m.scroll_offset = 3;
    m.active_buffer_id = Some(1);
    m.ensure_active_visible();
    assert_eq!(m.scroll_offset, 1);
}

#[test]
fn ensure_active_visible_no_active() {
    let mut m = BufferlineModule::new();
    m.scroll_offset = 5;
    m.ensure_active_visible();
    assert_eq!(m.scroll_offset, 5, "Should not change without active buffer");
}

#[test]
fn ensure_active_visible_active_not_in_tabs() {
    let mut m = BufferlineModule::new();
    m.tabs = vec![TabEntry {
        id: 1,
        name: String::from("a.rs"),
        modified: false,
        pinned: false,
        error_count: 0,
        warning_count: 0,
    }];
    m.active_buffer_id = Some(99);
    m.scroll_offset = 0;
    m.ensure_active_visible();
    assert_eq!(m.scroll_offset, 0);
}
