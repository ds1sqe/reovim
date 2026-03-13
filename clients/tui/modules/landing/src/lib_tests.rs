use super::*;
use reovim_client_driver::{Insets, RenderingModel};
use reovim_client_driver::types::ColorDepth;

// =============================================================================
// Mock infrastructure
// =============================================================================

struct MockSurface {
    cells: Vec<Vec<(char, Style)>>,
    width: u16,
    height: u16,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            cells: vec![vec![(' ', Style::new()); width as usize]; height as usize],
            width,
            height,
        }
    }

    fn char_at(&self, x: u16, y: u16) -> char {
        self.cells[y as usize][x as usize].0
    }

    fn has_content(&self) -> bool {
        self.cells.iter().any(|row| row.iter().any(|(ch, _)| *ch != ' '))
    }
}

impl RenderSurface for MockSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        for (i, ch) in text.chars().enumerate() {
            let cx = x as usize + i;
            if cx < self.width as usize && (y as usize) < self.height as usize {
                self.cells[y as usize][cx] = (ch, style.clone());
            }
        }
        text.len() as u16
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        for row in rect.y..rect.y + rect.height {
            for col in rect.x..rect.x + rect.width {
                if (col as usize) < self.width as usize && (row as usize) < self.height as usize {
                    self.cells[row as usize][col as usize] = (ch, style.clone());
                }
            }
        }
    }

    fn clear(&mut self, rect: Rect) {
        self.fill(rect, ' ', Style::new());
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

struct TestCaps;

impl PlatformCapabilities for TestCaps {
    fn rendering_model(&self) -> RenderingModel { RenderingModel::CellGrid }
    fn grid_size(&self) -> Option<(u16, u16)> { Some((80, 24)) }
    fn color_depth(&self) -> ColorDepth { ColorDepth::TrueColor }
    fn pixel_size(&self) -> Option<(u32, u32)> { None }
    fn reliable_unicode_width(&self) -> bool { true }
    fn dark_mode(&self) -> bool { true }
    fn smooth_scroll(&self) -> bool { false }
    fn pointer_events(&self) -> bool { true }
    fn touch_input(&self) -> bool { false }
    fn haptic(&self) -> bool { false }
    fn safe_area(&self) -> Insets { Insets::ZERO }
    fn has_focus(&self) -> bool { true }
    fn clipboard_available(&self) -> bool { true }
    fn screen_reader_active(&self) -> bool { false }
}

// =============================================================================
// Helpers
// =============================================================================

fn render(module: &LandingModule, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect { x: 0, y: 0, width: w, height: h };
    module.chrome_render(&mut surface, bounds, &TestCaps);
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = LandingModule::new();
    assert_eq!(m.id(), "landing");
    assert_eq!(m.kind(), "landing");
    assert_eq!(m.name(), "Landing");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_not_dismissed() {
    let m = LandingModule::default();
    assert!(!m.dismissed);
}

#[test]
fn chrome_role() {
    let m = LandingModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 30);
}

#[test]
fn server_kinds_empty() {
    let m = LandingModule::new();
    assert!(m.server_kinds().is_empty());
}

#[test]
fn lifecycle() {
    let mut m = LandingModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Dismissal behavior
// =============================================================================

#[test]
fn dismiss_on_cursor_update() {
    let mut m = LandingModule::new();
    assert!(!m.dismissed);
    m.on_cursor_update(BufferId(0), 0, 0);
    assert!(m.dismissed);
}

#[test]
fn dismiss_on_mode_change() {
    let mut m = LandingModule::new();
    m.on_mode_change("insert");
    assert!(m.dismissed);
}

#[test]
fn dismiss_on_buffer_update() {
    let mut m = LandingModule::new();
    m.on_buffer_update(&BufferUpdateEvent {
        buffer_id: BufferId(0),
        revision: 1,
        changed_range: 0..1,
        new_lines: vec!["hello".to_string()],
        total_lines: 1,
    });
    assert!(m.dismissed);
}

#[test]
fn dismiss_is_permanent() {
    let mut m = LandingModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    assert!(m.dismissed);
}

#[test]
fn tick_returns_false() {
    let mut m = LandingModule::new();
    assert!(!m.tick());
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_on_normal_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 80, 24);
    assert!(surface.has_content(), "Landing screen should render content");
}

#[test]
fn render_on_small_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 20, 5);
    assert!(!surface.has_content(), "Small terminal should not render landing");
}

#[test]
fn render_skipped_when_dismissed() {
    let mut m = LandingModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    let surface = render(&m, 80, 24);
    assert!(!surface.has_content(), "Dismissed extension should not render");
}

#[test]
fn render_content_centered() {
    let m = LandingModule::new();
    let surface = render(&m, 120, 40);
    let box_x: u16 = (120 - BOX_WIDTH) / 2;
    let box_y: u16 = (40 - 17) / 2;
    // Top-left corner should be U+256D
    assert_eq!(surface.char_at(box_x, box_y), '\u{256D}');
    // Top-right corner should be U+256E
    assert_eq!(surface.char_at(box_x + BOX_WIDTH - 1, box_y), '\u{256E}');
}

#[test]
fn render_large_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 200, 60);
    assert!(surface.has_content());
}
