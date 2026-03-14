use {
    super::*,
    reovim_client_driver::{Insets, RenderingModel, Style, types::ColorDepth},
};

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

    fn has_content(&self) -> bool {
        self.cells
            .iter()
            .any(|row| row.iter().any(|(ch, _)| *ch != ' '))
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
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: reovim_client_driver::types::Color) {}

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

struct TestPlatformCaps;

impl PlatformCapabilities for TestPlatformCaps {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> ColorDepth {
        ColorDepth::TrueColor
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
    fn safe_area(&self) -> Insets {
        Insets::ZERO
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

fn render_module(module: &MicroscopeModule, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect::new(0, 0, w, h);
    module.chrome_render(&mut surface, bounds, &TestPlatformCaps);
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
    assert!(!surface.has_content());
}

#[test]
fn render_shows_content() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("test", &["main.rs", "lib.rs"]));
    let surface = render_module(&m, 80, 24);
    assert!(surface.has_content());
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
    let first_row = bounds.panel_start_y as usize;
    assert_eq!(surface.cells[first_row][0].0, '>');
    // Second item should have ' ' indicator.
    if bounds.panel_height > 1 {
        let second_row = (bounds.panel_start_y + 1) as usize;
        assert_eq!(surface.cells[second_row][0].0, ' ');
    }
}

#[test]
fn render_too_small_screen() {
    let mut m = MicroscopeModule::new();
    m.on_notification(&active_payload("", &[]));
    let surface = render_module(&m, 10, 3);
    // Should not panic; small screen means nothing rendered.
    assert!(!surface.has_content());
}
