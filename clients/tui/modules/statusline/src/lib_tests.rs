use {
    super::*,
    reovim_client_driver::{
        testing::{MockPlatformCapabilities, WriteSurface},
        types::Color,
    },
};

fn test_caps() -> MockPlatformCapabilities {
    MockPlatformCapabilities::new()
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
    assert_eq!(m.version(), Version::new(0, 1, 0));
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
    let mut surface = WriteSurface::new(80, 1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    assert_eq!(surface.text_at(0, 0), Some(" INSERT "));
}

#[test]
fn statusline_cursor_update() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 10, 5);
    let mut surface = WriteSurface::new(80, 1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    // Cursor position should show "11:6" (1-indexed)
    let has_cursor_pos = surface.writes().iter().any(|w| w.text == "11:6");
    assert!(has_cursor_pos, "Expected cursor position 11:6 in writes");
}

#[test]
fn statusline_no_cursor_shows_question_marks() {
    let m = StatuslineModule::new();
    let mut surface = WriteSurface::new(80, 1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    let has_question = surface.writes().iter().any(|w| w.text == "?:?");
    assert!(has_question, "Expected ?:? when no cursor data");
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

// =============================================================================
// Rendering tests
// =============================================================================

#[test]
fn statusline_renders_mode_with_correct_style() {
    let mut m = StatuslineModule::new();
    m.on_mode_change("visual");
    let mut surface = WriteSurface::new(80, 1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    let style = surface.style_at(0, 0).unwrap();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Magenta));
}

#[test]
fn statusline_renders_at_bounds_offset() {
    let m = StatuslineModule::new();
    let mut surface = WriteSurface::new(80, 24);
    let bounds = Rect {
        x: 0,
        y: 23,
        width: 80,
        height: 1,
    };
    m.chrome_render(&mut surface, bounds, &test_caps());
    // Mode should render at y=23
    assert_eq!(surface.text_at(0, 23), Some(" NORMAL "));
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
