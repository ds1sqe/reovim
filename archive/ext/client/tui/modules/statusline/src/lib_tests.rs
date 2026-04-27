use {
    super::*,
    reovim_client_driver::{
        testing::{MockPlatformCapabilities, MockThemeProvider, TestModuleContext},
        types::Color,
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
    let (w, _h) = (g.width(), g.height());
    (0..w)
        .map(|x| g.get_cell(x, y).map_or(' ', |c| c.ch))
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn bounds(width: u16) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width,
        height: 1,
    }
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn statusline_id() {
    let m = StatuslineModule::new();
    assert_eq!(m.id(), "statusline");
    assert_eq!(m.kind(), "statusline");
    assert_eq!(m.name(), "Statusline");
}

#[test]
fn statusline_version() {
    let m = StatuslineModule::new();
    assert_eq!(m.version(), Version::new(0, 2, 0));
}

#[test]
fn statusline_server_kinds_empty() {
    let m = StatuslineModule::new();
    assert!(m.server_kinds().is_empty());
}

// =============================================================================
// Role tests
// =============================================================================

#[test]
fn statusline_has_chrome() {
    let m = StatuslineModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert!(!m.has_annotations());
}

#[test]
fn statusline_chrome_position_bottom() {
    let m = StatuslineModule::new();
    assert_eq!(m.chrome_position(), ChromePosition::Bottom);
}

#[test]
fn statusline_chrome_size_one() {
    let m = StatuslineModule::new();
    assert_eq!(m.chrome_requested_size(&test_caps()), 1);
}

#[test]
fn statusline_chrome_priority_highest() {
    let m = StatuslineModule::new();
    assert_eq!(m.chrome_priority(), 100);
}

// =============================================================================
// Event tests
// =============================================================================

#[test]
fn statusline_mode_change() {
    let mut m = StatuslineModule::new();
    m.on_mode_change("insert");
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    assert!(text_at_row(&surface, 0).contains(" INSERT "));
}

#[test]
fn statusline_cursor_update() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 10, 5);
    m.total_lines = 100;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_cursor_pos = text_at_row(&surface, 0).contains("11:6");
    assert!(has_cursor_pos, "Expected cursor position 11:6 in writes");
}

#[test]
fn statusline_no_cursor_shows_question_marks() {
    let m = StatuslineModule::new();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_question = text_at_row(&surface, 0).contains("?:?");
    assert!(has_question, "Expected ?:? when no cursor data");
}

#[test]
fn statusline_buffer_update_tracks_total_lines() {
    let mut m = StatuslineModule::new();
    m.on_buffer_update(&BufferUpdateEvent {
        buffer_id: BufferId(1),
        revision: 1,
        changed_range: 0..0,
        new_lines: vec![],
        total_lines: 500,
    });
    assert_eq!(m.total_lines, 500);
}

// =============================================================================
// Mode style tests
// =============================================================================

#[test]
fn mode_style_normal() {
    let style = mode_style("NORMAL");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Blue));
}

#[test]
fn mode_style_insert() {
    let style = mode_style("INSERT");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Green));
}

#[test]
fn mode_style_visual() {
    let style = mode_style("VISUAL");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Magenta));
}

#[test]
fn mode_style_command() {
    let style = mode_style("COMMAND");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Yellow));
}

#[test]
fn mode_style_cmdline() {
    let style = mode_style("CMDLINE");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Yellow));
}

#[test]
fn mode_style_replace() {
    let style = mode_style("REPLACE");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Red));
}

#[test]
fn mode_style_unknown_defaults_to_normal() {
    let style = mode_style("OPERATOR_PENDING");
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Blue));
}

// =============================================================================
// Progress indicator tests
// =============================================================================

#[test]
fn progress_all_for_empty_buffer() {
    assert_eq!(progress_indicator(0, 0), "All");
}

#[test]
fn progress_all_for_single_line() {
    assert_eq!(progress_indicator(0, 1), "All");
}

#[test]
fn progress_top_at_first_line() {
    assert_eq!(progress_indicator(0, 100), "Top");
}

#[test]
fn progress_bot_at_last_line() {
    assert_eq!(progress_indicator(99, 100), "Bot");
}

#[test]
fn progress_bot_beyond_last_line() {
    assert_eq!(progress_indicator(200, 100), "Bot");
}

#[test]
fn progress_50_percent_at_midpoint() {
    assert_eq!(progress_indicator(50, 101), "50%");
}

#[test]
fn progress_1_percent_near_top() {
    assert_eq!(progress_indicator(1, 100), "1%");
}

#[test]
fn progress_98_percent_near_bottom() {
    assert_eq!(progress_indicator(98, 100), "98%");
}

#[test]
fn progress_two_line_buffer() {
    assert_eq!(progress_indicator(0, 2), "Top");
    assert_eq!(progress_indicator(1, 2), "Bot");
}

// =============================================================================
// Rendering tests
// =============================================================================

#[test]
fn statusline_renders_mode_with_correct_style() {
    let mut m = StatuslineModule::new();
    m.on_mode_change("visual");
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let style = style_at(&surface, 0, 0);
    assert_eq!(style.fg, Some(CellColor::Named(0)));
    assert_eq!(style.bg, Some(CellColor::Named(13)));
}

#[test]
fn statusline_renders_at_bounds_offset() {
    let m = StatuslineModule::default();
    let mut surface = CellCapability::new(80, 24);
    let bounds = Rect {
        x: 0,
        y: 23,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    assert!(text_at_row(&surface, 23).contains(" NORMAL "));
}

#[test]
fn statusline_renders_progress_in_z_section() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 50, 10);
    m.total_lines = 200;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    // Should show progress + position
    let row = text_at_row(&surface, 0);
    let has_progress = row.contains("25%") && row.contains("51:11");
    assert!(has_progress, "Expected '25% 51:11' in row");
}

#[test]
fn statusline_renders_top_progress() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.total_lines = 100;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_top = text_at_row(&surface, 0).contains("Top");
    assert!(has_top, "Expected 'Top' in writes");
}

#[test]
fn statusline_renders_bot_progress() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 99, 0);
    m.total_lines = 100;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_bot = text_at_row(&surface, 0).contains("Bot");
    assert!(has_bot, "Expected 'Bot' in writes");
}

#[test]
fn statusline_renders_all_progress_for_small_buffer() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.total_lines = 1;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_all = text_at_row(&surface, 0).contains("All");
    assert!(has_all, "Expected 'All' in writes");
}

#[test]
fn statusline_fills_background() {
    let m = StatuslineModule::new();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    // The fill call sets bg=DarkGrey across the entire row
    let style = style_at(&surface, 40, 0);
    assert_eq!(style.bg, Some(CellColor::Named(8)));
}

#[test]
fn statusline_zero_width_does_not_render() {
    let m = StatuslineModule::new();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(0), &test_caps());
    assert!(!has_content(&surface), "Expected no writes on zero width");
}

// =============================================================================
// Section C (filename) tests — Phase 2 fields
// =============================================================================

#[test]
fn statusline_renders_filename_when_set() {
    let mut m = StatuslineModule::new();
    m.filename = Some(String::from("main.rs"));
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_filename = text_at_row(&surface, 0).contains("main.rs");
    assert!(has_filename, "Expected filename in writes");
}

#[test]
fn statusline_renders_modified_indicator() {
    let mut m = StatuslineModule::new();
    m.filename = Some(String::from("main.rs"));
    m.modified = true;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_modified = text_at_row(&surface, 0).contains(MODIFIED_ICON);
    assert!(has_modified, "Expected modified icon in row");
}

#[test]
fn statusline_renders_readonly_indicator() {
    let mut m = StatuslineModule::new();
    m.filename = Some(String::from("main.rs"));
    m.readonly = true;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_readonly = text_at_row(&surface, 0).contains(READONLY_ICON);
    assert!(has_readonly, "Expected readonly icon in row");
}

#[test]
fn statusline_no_filename_no_section_c() {
    let m = StatuslineModule::new();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let row = text_at_row(&surface, 0);
    let has_filename = row.contains("main.rs") || row.contains(MODIFIED_ICON);
    assert!(!has_filename, "Expected no filename section without data");
}

// =============================================================================
// Section Y (filetype + encoding) tests — Phase 2 fields
// =============================================================================

#[test]
fn statusline_renders_filetype_in_y_section() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.filetype = Some(String::from("rust"));
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_ft = text_at_row(&surface, 0).contains("rust");
    assert!(has_ft, "Expected filetype 'rust' in writes");
}

#[test]
fn statusline_renders_filetype_and_encoding() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.filetype = Some(String::from("rust"));
    m.encoding = Some(String::from("utf-8"));
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let row = text_at_row(&surface, 0);
    let has_both = row.contains("rust") && row.contains("utf-8");
    assert!(has_both, "Expected 'rust utf-8' in row");
}

// =============================================================================
// Section B (git branch) tests — Phase 3 fields
// =============================================================================

#[test]
fn statusline_renders_git_branch() {
    let m = StatuslineModule {
        git_branch: Some(String::from("main")),
        ..Default::default()
    };
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let row = text_at_row(&surface, 0);
    assert!(row.contains("main"), "Expected git branch 'main' in row");
    assert!(row.contains(GIT_BRANCH_ICON), "Expected git branch icon in row");
}

#[test]
fn statusline_no_branch_no_section_b() {
    let m = StatuslineModule::new();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_branch = text_at_row(&surface, 0).contains("\u{e0a0}");
    assert!(!has_branch, "Expected no branch icon without data");
}

// =============================================================================
// Build right sections tests
// =============================================================================

#[test]
fn build_right_sections_minimal() {
    let m = StatuslineModule::new();
    let sections = m.build_right_sections();
    // No cursor → just "?:?"
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].0, "?:?");
}

#[test]
fn build_right_sections_with_cursor() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 9, 4);
    m.total_lines = 50;
    let sections = m.build_right_sections();
    // Just Z section (no filetype)
    assert_eq!(sections.len(), 1);
    assert!(sections[0].0.contains("10:5"));
}

#[test]
fn build_right_sections_with_filetype() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.filetype = Some(String::from("python"));
    let sections = m.build_right_sections();
    // Y section + Z section
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "python");
}

#[test]
fn build_right_sections_with_filetype_and_encoding() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.filetype = Some(String::from("python"));
    m.encoding = Some(String::from("utf-8"));
    let sections = m.build_right_sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "python utf-8");
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn statusline_default_impl() {
    let m = StatuslineModule::default();
    assert_eq!(m.id(), "statusline");
}

#[test]
fn statusline_exit_ok() {
    let mut m = StatuslineModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// Notification tests (Phase 2: buffer metadata)
// =============================================================================

#[test]
fn on_notification_parses_buffer_metadata() {
    let mut m = StatuslineModule::new();
    m.on_notification(r#"{"filename":"main.rs","filetype":"rust","encoding":"utf-8","modified":true,"readonly":false}"#);
    assert_eq!(m.filename.as_deref(), Some("main.rs"));
    assert_eq!(m.filetype.as_deref(), Some("rust"));
    assert_eq!(m.encoding.as_deref(), Some("utf-8"));
    assert!(m.modified);
    assert!(!m.readonly);
}

#[test]
fn on_notification_partial_payload() {
    let mut m = StatuslineModule::new();
    m.on_notification(r#"{"filename":"lib.rs"}"#);
    assert_eq!(m.filename.as_deref(), Some("lib.rs"));
    // Other fields unchanged
    assert!(m.filetype.is_none());
    assert!(!m.modified);
}

#[test]
fn on_notification_empty_filename_clears() {
    let mut m = StatuslineModule::new();
    m.filename = Some(String::from("old.rs"));
    m.on_notification(r#"{"filename":""}"#);
    assert!(m.filename.is_none());
}

#[test]
fn on_notification_invalid_json_ignored() {
    let mut m = StatuslineModule::new();
    m.filename = Some(String::from("keep.rs"));
    m.on_notification("not json at all");
    assert_eq!(m.filename.as_deref(), Some("keep.rs"));
}

#[test]
fn on_notification_updates_modified_flag() {
    let mut m = StatuslineModule::new();
    assert!(!m.modified);
    m.on_notification(r#"{"modified":true}"#);
    assert!(m.modified);
    m.on_notification(r#"{"modified":false}"#);
    assert!(!m.modified);
}

#[test]
fn on_notification_updates_readonly_flag() {
    let mut m = StatuslineModule::new();
    assert!(!m.readonly);
    m.on_notification(r#"{"readonly":true}"#);
    assert!(m.readonly);
}

// =============================================================================
// PCT_STRINGS table tests
// =============================================================================

#[test]
fn pct_strings_table_has_100_entries() {
    assert_eq!(PCT_STRINGS.len(), 100);
}

#[test]
fn pct_strings_first_entry() {
    assert_eq!(PCT_STRINGS[0], "0%");
}

#[test]
fn pct_strings_last_entry() {
    assert_eq!(PCT_STRINGS[99], "99%");
}

#[test]
fn pct_strings_midpoint() {
    assert_eq!(PCT_STRINGS[50], "50%");
}

// =============================================================================
// Section X (diagnostic counts) tests — Phase 4
// =============================================================================

#[test]
fn build_diagnostic_text_all_zero() {
    let m = StatuslineModule::default();
    assert!(m.build_diagnostic_text().is_empty());
}

#[test]
fn build_diagnostic_text_errors_only() {
    let m = StatuslineModule {
        diag_error: 3,
        ..Default::default()
    };
    let text = m.build_diagnostic_text();
    assert!(text.contains(ERROR_ICON));
    assert!(text.contains('3'));
    assert!(!text.contains(WARNING_ICON));
}

#[test]
fn build_diagnostic_text_mixed() {
    let m = StatuslineModule {
        diag_error: 1,
        diag_warning: 2,
        diag_hint: 5,
        ..Default::default()
    };
    let text = m.build_diagnostic_text();
    assert!(text.contains(ERROR_ICON));
    assert!(text.contains(WARNING_ICON));
    assert!(text.contains(HINT_ICON));
    assert!(!text.contains(INFO_ICON));
}

#[test]
fn statusline_renders_diagnostics_section() {
    let mut m = StatuslineModule::default();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.diag_error = 2;
    m.diag_warning = 1;
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let has_error = text_at_row(&surface, 0).contains(ERROR_ICON);
    assert!(has_error, "Expected error icon in writes");
}

#[test]
fn statusline_no_diagnostics_when_zero() {
    let m = StatuslineModule::default();
    let mut surface = CellCapability::new(80, 1);
    m.chrome_render(&mut surface, bounds(80), &test_caps());
    let row = text_at_row(&surface, 0);
    let has_diag = row.contains(ERROR_ICON) || row.contains(WARNING_ICON);
    assert!(!has_diag, "Expected no diagnostic icons when counts are zero");
}

#[test]
fn on_notification_parses_diagnostic_counts() {
    let mut m = StatuslineModule::default();
    m.on_notification(r#"{"diagnostics":{"error":3,"warning":1,"info":0,"hint":2}}"#);
    assert_eq!(m.diag_error, 3);
    assert_eq!(m.diag_warning, 1);
    assert_eq!(m.diag_info, 0);
    assert_eq!(m.diag_hint, 2);
}

#[test]
fn build_right_sections_includes_diagnostics() {
    let mut m = StatuslineModule::default();
    m.on_cursor_update(BufferId(0), 0, 0);
    m.diag_error = 1;
    let sections = m.build_right_sections();
    // X + Z = 2 sections (no filetype)
    assert_eq!(sections.len(), 2);
    assert!(sections[0].0.contains(ERROR_ICON));
}

// =============================================================================
// Theme caching tests
// =============================================================================

#[test]
fn init_caches_theme_styles() {
    let mut m = StatuslineModule::new();
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
        .build();
    m.init(&ctx_owner.as_context());

    assert_eq!(
        m.bg_style.bg,
        Some(Color::Rgb {
            r: 40,
            g: 44,
            b: 52
        })
    );
    assert_eq!(
        m.bg_style.fg,
        Some(Color::Rgb {
            r: 171,
            g: 178,
            b: 191
        })
    );
}

#[test]
fn on_theme_changed_updates_styles() {
    let mut m = StatuslineModule::default();
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
