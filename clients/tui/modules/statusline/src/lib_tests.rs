use super::*;
use reovim_client_driver::types::Color;

// =============================================================================
// MockSurface
// =============================================================================

struct MockSurface {
    writes: Vec<(u16, u16, String, Style)>,
    width: u16,
    height: u16,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            writes: Vec::new(),
            width,
            height,
        }
    }

    fn text_at(&self, x: u16, y: u16) -> Option<&str> {
        self.writes
            .iter()
            .rev()
            .find(|(wx, wy, _, _)| *wx == x && *wy == y)
            .map(|(_, _, s, _)| s.as_str())
    }

    fn style_at(&self, x: u16, y: u16) -> Option<&Style> {
        self.writes
            .iter()
            .rev()
            .find(|(wx, wy, _, _)| *wx == x && *wy == y)
            .map(|(_, _, _, s)| s)
    }
}

impl RenderSurface for MockSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u16;
        self.writes.push((x, y, text.to_string(), style));
        len
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}
    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}
    fn clear(&mut self, _rect: Rect) {}
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

fn test_caps() -> TestCaps {
    TestCaps
}

struct TestCaps;

impl PlatformCapabilities for TestCaps {
    fn rendering_model(&self) -> reovim_client_driver::RenderingModel {
        reovim_client_driver::RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> reovim_client_driver::types::ColorDepth {
        reovim_client_driver::types::ColorDepth::TrueColor
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn reliable_unicode_width(&self) -> bool {
        true
    }
    fn dark_mode(&self) -> bool {
        true
    }
    fn smooth_scroll(&self) -> bool {
        false
    }
    fn pointer_events(&self) -> bool {
        true
    }
    fn touch_input(&self) -> bool {
        false
    }
    fn haptic(&self) -> bool {
        false
    }
    fn safe_area(&self) -> reovim_client_driver::Insets {
        reovim_client_driver::Insets::ZERO
    }
    fn has_focus(&self) -> bool {
        true
    }
    fn clipboard_available(&self) -> bool {
        true
    }
    fn screen_reader_active(&self) -> bool {
        false
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
    let mut surface = MockSurface::new(80, 1);
    let bounds = Rect { x: 0, y: 0, width: 80, height: 1 };
    m.chrome_render(&mut surface, bounds, &test_caps());
    assert_eq!(surface.text_at(0, 0), Some(" INSERT "));
}

#[test]
fn statusline_cursor_update() {
    let mut m = StatuslineModule::new();
    m.on_cursor_update(BufferId(0), 10, 5);
    let mut surface = MockSurface::new(80, 1);
    let bounds = Rect { x: 0, y: 0, width: 80, height: 1 };
    m.chrome_render(&mut surface, bounds, &test_caps());
    // Cursor position should show "11:6" (1-indexed)
    let has_cursor_pos = surface.writes.iter().any(|(_, _, text, _)| text == "11:6");
    assert!(has_cursor_pos, "Expected cursor position 11:6 in writes");
}

#[test]
fn statusline_no_cursor_shows_question_marks() {
    let m = StatuslineModule::new();
    let mut surface = MockSurface::new(80, 1);
    let bounds = Rect { x: 0, y: 0, width: 80, height: 1 };
    m.chrome_render(&mut surface, bounds, &test_caps());
    let has_question = surface.writes.iter().any(|(_, _, text, _)| text == "?:?");
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
    let mut surface = MockSurface::new(80, 1);
    let bounds = Rect { x: 0, y: 0, width: 80, height: 1 };
    m.chrome_render(&mut surface, bounds, &test_caps());
    let style = surface.style_at(0, 0).unwrap();
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::Magenta));
}

#[test]
fn statusline_renders_at_bounds_offset() {
    let m = StatuslineModule::new();
    let mut surface = MockSurface::new(80, 24);
    let bounds = Rect { x: 0, y: 23, width: 80, height: 1 };
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
