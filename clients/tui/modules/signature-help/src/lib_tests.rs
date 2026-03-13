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

    fn style_at(&self, x: u16, y: u16) -> &Style {
        &self.cells[y as usize][x as usize].1
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

fn active_payload(label: &str) -> String {
    format!(
        r#"{{"active":true,"label":"{label}","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
    )
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

fn render(module: &SignatureHelpModule, w: u16, h: u16) -> MockSurface {
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
    assert_eq!(surface.char_at(0, 0), ' ');
}

#[test]
fn render_shows_popup_above_origin() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo(x: i32)"));
    let surface = render(&m, 80, 24);
    // Origin at line 5, popup_h=3 -> py = 5 - 3 = 2, px = 10
    assert_eq!(surface.char_at(10, 2), '\u{256D}');
}

#[test]
fn render_border_color_yellow() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    let surface = render(&m, 80, 24);
    assert_eq!(surface.style_at(10, 2).fg, Some(Color::Yellow));
}

#[test]
fn render_content_text() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(&active_payload("fn foo()"));
    let surface = render(&m, 80, 24);
    // Content at (px+1, py+1) = (11, 3)
    assert_eq!(surface.char_at(11, 3), 'f');
    assert_eq!(surface.style_at(11, 3).fg, Some(Color::White));
}

#[test]
fn render_popup_below_when_no_room_above() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":"fn foo()","origin":{"BufferPosition":{"buffer_id":1,"line":1,"col":0}}}"#,
    );
    let surface = render(&m, 80, 24);
    // anchor_y=1, popup_h=3: 1 < 3, try below: 1+1+3=5 <= 24 -> py=2
    assert_eq!(surface.char_at(0, 2), '\u{256D}');
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
    assert_eq!(surface.char_at(0, 0), '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut m = SignatureHelpModule::new();
    m.on_notification(
        r#"{"active":true,"label":"fn f()","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );
    let surface = render(&m, 80, 24);
    // popup_w = 10 (min), max_x = 80 - 10 = 70. col 75 > 70.
    assert_eq!(surface.char_at(70, 2), '\u{256D}');
}

// =============================================================================
// Type coverage
// =============================================================================

#[test]
fn origin_debug_clone() {
    let o = Origin::BufferPosition { buffer_id: 1, line: 2, col: 3 };
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
