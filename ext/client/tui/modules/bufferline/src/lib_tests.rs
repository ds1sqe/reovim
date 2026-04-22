use {
    super::*,
    reovim_client_driver::testing::{
        MockPlatformCapabilities, MockThemeProvider, TestModuleContext,
    },
    reovim_ext_client_tui_cap_cell::{CellCapability, CellColor, CellStyle},
};

fn test_caps() -> MockPlatformCapabilities {
    MockPlatformCapabilities::new()
}

fn has_content(g: &CellCapability) -> bool {
    g.iter().any(|(_, c)| c.ch != ' ')
}

fn style_at(g: &CellCapability, x: u16, y: u16) -> CellStyle {
    g.get_cell(x, y).map(|c| c.style).unwrap_or_default()
}

fn text_at_row(g: &CellCapability, y: u16) -> String {
    let w = g.width();
    (0..w)
        .map(|x| g.get_cell(x, y).map_or(' ', |c| c.ch))
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Find the x-column where `needle` starts on row `y`, or `None` if absent.
fn find_text_x(g: &CellCapability, y: u16, needle: &str) -> Option<u16> {
    let row = (0..g.width())
        .map(|x| g.get_cell(x, y).map_or(' ', |c| c.ch))
        .collect::<String>();
    row.find(needle).and_then(|byte_idx| {
        // chars up to byte_idx == column offset (all one-byte chars in test fixtures
        // are ASCII; the test rendering uses only ASCII + single-char BMP icons).
        u16::try_from(row[..byte_idx].chars().count()).ok()
    })
}

/// Highest column index on row `y` that holds a non-space cell, or `None` if row empty.
fn last_content_x(g: &CellCapability, y: u16) -> Option<u16> {
    (0..g.width())
        .rev()
        .find(|&x| g.get_cell(x, y).is_some_and(|c| c.ch != ' '))
}

fn full_bounds() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    }
}

fn buffer_list_json(buffers: &str) -> String {
    format!(r#"{{"type":"buffer_list","buffers":[{buffers}]}}"#)
}

fn one_buf(id: u64, name: &str, modified: bool) -> String {
    format!(r#"{{"id":{id},"name":"{name}","modified":{modified}}}"#)
}

fn pin_state_json(pins: &[u64]) -> String {
    let pins_str: Vec<String> = pins.iter().map(ToString::to_string).collect();
    format!(r#"{{"type":"pin_state","pins":[{}]}}"#, pins_str.join(","))
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
    assert_eq!(m.version(), Version::new(0, 2, 0));
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
fn chrome_position_overlay() {
    let m = BufferlineModule::new();
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
}

#[test]
fn chrome_size_zero_overlay() {
    let m = BufferlineModule::new();
    assert_eq!(m.chrome_requested_size(&test_caps()), 0);
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
fn init_exit_succeeds() {
    let mut m = BufferlineModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// on_notification — buffer_list
// =============================================================================

#[test]
fn buffer_list_populates_tabs() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "main.rs", false)));
    assert_eq!(m.tabs.len(), 1);
    assert_eq!(m.tabs[0].name, "main.rs");
    assert!(!m.tabs[0].modified);
}

#[test]
fn buffer_list_replaces_tabs() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "main.rs", false)));

    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", false),
        one_buf(2, "lib.rs", true)
    )));
    assert_eq!(m.tabs.len(), 2);
}

#[test]
fn buffer_list_empty_clears() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "main.rs", false)));
    m.on_notification(&buffer_list_json(""));
    assert!(m.tabs.is_empty());
}

#[test]
fn buffer_list_merges_with_cached_pins() {
    let mut m = BufferlineModule::new();
    m.on_notification(&pin_state_json(&[2]));
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "a.rs", false),
        one_buf(2, "b.rs", false)
    )));

    assert_eq!(m.tabs[0].id, 2);
    assert!(m.tabs[0].pinned);
    assert_eq!(m.tabs[1].id, 1);
    assert!(!m.tabs[1].pinned);
}

// =============================================================================
// on_notification — pin_state
// =============================================================================

#[test]
fn pin_state_updates_cache() {
    let mut m = BufferlineModule::new();
    m.on_notification(&pin_state_json(&[1, 3]));
    assert_eq!(m.pinned_ids, vec![1, 3]);
}

#[test]
fn pin_state_reapplies_to_existing_tabs() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "a.rs", false),
        one_buf(2, "b.rs", false)
    )));

    m.on_notification(&pin_state_json(&[1]));
    let tab1 = m.tabs.iter().find(|t| t.id == 1).unwrap();
    assert!(tab1.pinned);
}

#[test]
fn pin_state_empty_unpins_all() {
    let mut m = BufferlineModule::new();
    m.on_notification(&pin_state_json(&[1]));
    m.on_notification(&buffer_list_json(&one_buf(1, "a.rs", false)));
    assert!(m.tabs[0].pinned);

    m.on_notification(&pin_state_json(&[]));
    assert!(!m.tabs[0].pinned);
}

// =============================================================================
// on_notification — malformed / unknown
// =============================================================================

#[test]
fn malformed_json_ignored() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "a.rs", false)));
    m.on_notification("not json");
    assert_eq!(m.tabs.len(), 1);
}

#[test]
fn unknown_type_ignored() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "a.rs", false)));
    m.on_notification(r#"{"type":"unknown","data":42}"#);
    assert_eq!(m.tabs.len(), 1);
}

// =============================================================================
// on_buffer_focus
// =============================================================================

#[test]
fn on_buffer_focus_updates_active() {
    let mut m = BufferlineModule::new();
    m.on_buffer_focus(BufferId(42));
    assert_eq!(m.active_buffer_id, Some(42));
}

// =============================================================================
// chrome_render — overlay at top-right
// =============================================================================

#[test]
fn render_hidden_with_one_tab() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&one_buf(1, "main.rs", false)));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    // Single tab — should not render.
    let has_tab = text_at_row(&surface, 0).contains("main.rs");
    assert!(!has_tab, "Should not render with only 1 tab");
}

#[test]
fn render_visible_with_two_tabs() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", false),
        one_buf(2, "lib.rs", false)
    )));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    let has_main = text_at_row(&surface, 0).contains("main.rs");
    let has_lib = text_at_row(&surface, 0).contains("lib.rs");
    assert!(has_main, "Expected main.rs");
    assert!(has_lib, "Expected lib.rs");
}

#[test]
fn render_at_top_right() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "a.rs", false),
        one_buf(2, "b.rs", false)
    )));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    // All writes should be on row 0 (top) and right-aligned.
    for y in 1..surface.height() {
        assert!(
            text_at_row(&surface, y).is_empty(),
            "Should render on top row only; row {y} has content"
        );
    }

    // The tab display is right-aligned: `start_x = width - tab_str.byte_len`.
    // For " a.rs │ b.rs " (13 chars / 15 bytes) with a 3-byte separator,
    // start_x = 80 - 15 = 65; text occupies x=65..=77; trailing space at 77;
    // last non-space char ('s' of b.rs) at x=76. This asserts the display's
    // final non-space column lies in the expected right-edge band — a visual-
    // column analogue of the old byte-length-based "end_x == 80" assertion.
    let last_x = last_content_x(&surface, 0).expect("Expected content on row 0");
    assert!(
        (74..=79).contains(&last_x),
        "Tabs should right-align near screen edge; last_x={last_x}"
    );
}

#[test]
fn render_active_tab_highlighted() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", false),
        one_buf(2, "lib.rs", false)
    )));
    m.on_buffer_focus(BufferId(1));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    // The active tab is overwritten on top of the bg — final-grid style at the
    // first "main.rs" character carries the active-style background.
    let x = find_text_x(&surface, 0, "main.rs").expect("Expected main.rs tab on row 0");
    let style = style_at(&surface, x, 0);
    // Color::Blue → CellColor::Named(12)
    assert_eq!(style.bg, Some(CellColor::Named(12)));
}

#[test]
fn render_modified_shows_marker() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", true),
        one_buf(2, "lib.rs", false)
    )));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    let has_modified = text_at_row(&surface, 0).contains(MODIFIED_ICON);
    assert!(has_modified, "Expected modified icon for modified buffer");
}

#[test]
fn render_pinned_shows_star() {
    let mut m = BufferlineModule::new();
    m.on_notification(&pin_state_json(&[1]));
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", false),
        one_buf(2, "lib.rs", false)
    )));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    let has_pin = text_at_row(&surface, 0).contains(PIN_ICON);
    assert!(has_pin, "Expected pin icon for pinned buffer");
}

#[test]
fn render_zero_width_no_crash() {
    let mut m = BufferlineModule::new();
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "a.rs", false),
        one_buf(2, "b.rs", false)
    )));

    let mut surface = CellCapability::new(80, 24);
    let zero_bounds = Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 24,
    };
    m.chrome_render(&mut surface, zero_bounds, &test_caps());
    assert!(!has_content(&surface));
}

#[test]
fn render_empty_tabs_no_output() {
    let m = BufferlineModule::new();
    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());
    assert!(!has_content(&surface));
}

// =============================================================================
// format_tab_label
// =============================================================================

#[test]
fn format_tab_label_basic() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: false,
        pinned: false,
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
    };
    let label = format_tab_label(&tab);
    assert_eq!(label, format!(" main.rs {MODIFIED_ICON} "));
}

#[test]
fn format_tab_label_pinned() {
    let tab = TabEntry {
        id: 1,
        name: String::from("main.rs"),
        modified: false,
        pinned: true,
    };
    assert_eq!(format_tab_label(&tab), format!(" {PIN_ICON} main.rs "));
}

#[test]
fn format_tab_label_all_indicators() {
    let tab = TabEntry {
        id: 1,
        name: String::from("x.rs"),
        modified: true,
        pinned: true,
    };
    let label = format_tab_label(&tab);
    assert!(label.contains(PIN_ICON));
    assert!(label.contains("x.rs"));
    assert!(label.contains(MODIFIED_ICON));
}

// =============================================================================
// build_tab_string
// =============================================================================

#[test]
fn build_tab_string_empty() {
    let m = BufferlineModule::new();
    let (s, range) = m.build_tab_string();
    assert!(s.is_empty());
    assert!(range.is_none());
}

#[test]
fn build_tab_string_single() {
    let mut m = BufferlineModule::new();
    m.tabs = vec![TabEntry {
        id: 1,
        name: "a.rs".into(),
        modified: false,
        pinned: false,
    }];
    let (s, _) = m.build_tab_string();
    assert_eq!(s, " a.rs ");
}

#[test]
fn build_tab_string_active_range() {
    let mut m = BufferlineModule::new();
    m.active_buffer_id = Some(2);
    m.tabs = vec![
        TabEntry {
            id: 1,
            name: "a.rs".into(),
            modified: false,
            pinned: false,
        },
        TabEntry {
            id: 2,
            name: "b.rs".into(),
            modified: false,
            pinned: false,
        },
    ];
    let (s, range) = m.build_tab_string();
    assert!(s.contains("a.rs"));
    assert!(s.contains("b.rs"));
    let (start, end) = range.unwrap();
    assert_eq!(&s[start..end], " b.rs ");
}

// =============================================================================
// sort_tabs
// =============================================================================

#[test]
fn sort_tabs_pinned_first() {
    let mut m = BufferlineModule::new();
    m.pinned_ids = vec![2];
    m.tabs = vec![
        TabEntry {
            id: 1,
            name: "a.rs".into(),
            modified: false,
            pinned: false,
        },
        TabEntry {
            id: 2,
            name: "b.rs".into(),
            modified: false,
            pinned: true,
        },
    ];
    m.sort_tabs();
    assert_eq!(m.tabs[0].id, 2);
    assert_eq!(m.tabs[1].id, 1);
}

#[test]
fn sort_tabs_pinned_order_preserved() {
    let mut m = BufferlineModule::new();
    m.pinned_ids = vec![3, 1];
    m.tabs = vec![
        TabEntry {
            id: 1,
            name: "a.rs".into(),
            modified: false,
            pinned: true,
        },
        TabEntry {
            id: 2,
            name: "b.rs".into(),
            modified: false,
            pinned: false,
        },
        TabEntry {
            id: 3,
            name: "c.rs".into(),
            modified: false,
            pinned: true,
        },
    ];
    m.sort_tabs();
    assert_eq!(m.tabs[0].id, 3);
    assert_eq!(m.tabs[1].id, 1);
    assert_eq!(m.tabs[2].id, 2);
}

// =============================================================================
// Theme caching
// =============================================================================

#[test]
fn init_caches_theme_styles() {
    let mut m = BufferlineModule::new();
    let ctx_owner = TestModuleContext::builder()
        .highlight(
            "statusline_bg",
            Style::new().bg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            }),
        )
        .highlight(
            "statusline_fg",
            Style::new().fg(Color::Rgb {
                r: 171,
                g: 178,
                b: 191,
            }),
        )
        .highlight(
            "mode_normal",
            Style::new()
                .bg(Color::Rgb {
                    r: 97,
                    g: 175,
                    b: 239,
                })
                .fg(Color::White),
        )
        .build();
    m.init(&ctx_owner.as_context());

    assert_eq!(
        m.bg_style.fg,
        Some(Color::Rgb {
            r: 171,
            g: 178,
            b: 191
        })
    );
    assert_eq!(
        m.active_style.bg,
        Some(Color::Rgb {
            r: 97,
            g: 175,
            b: 239
        })
    );
}

#[test]
fn on_theme_changed_updates_styles() {
    let mut m = BufferlineModule::new();
    let original_bg = m.bg_style.bg;

    let theme = MockThemeProvider::new().with_highlight(
        "statusline_bg",
        Style::new().bg(Color::Rgb {
            r: 30,
            g: 30,
            b: 30,
        }),
    );
    m.on_theme_changed(&theme);

    assert_ne!(m.bg_style.bg, original_bg);
}

#[test]
fn render_uses_theme_styles() {
    let mut m = BufferlineModule::new();
    let custom_mode = Style::new()
        .bg(Color::Rgb {
            r: 100,
            g: 100,
            b: 200,
        })
        .fg(Color::White);
    let ctx_owner = TestModuleContext::builder()
        .highlight(
            "statusline_bg",
            Style::new().bg(Color::Rgb {
                r: 50,
                g: 50,
                b: 50,
            }),
        )
        .highlight("mode_normal", custom_mode)
        .build();
    m.init(&ctx_owner.as_context());
    m.on_notification(&buffer_list_json(&format!(
        "{},{}",
        one_buf(1, "main.rs", false),
        one_buf(2, "lib.rs", false)
    )));
    m.on_buffer_focus(BufferId(1));

    let mut surface = CellCapability::new(80, 24);
    m.chrome_render(&mut surface, full_bounds(), &test_caps());

    // Active tab should use mode_normal bg, not hardcoded Blue.
    let x = find_text_x(&surface, 0, "main.rs").expect("Expected main.rs tab on row 0");
    let style = style_at(&surface, x, 0);
    // custom_mode.bg is Color::Rgb { r: 100, g: 100, b: 200 }
    assert_eq!(style.bg, Some(CellColor::Rgb(100, 100, 200)));
}
