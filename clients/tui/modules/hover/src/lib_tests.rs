use {
    super::*,
    reovim_client_driver::{Insets, RenderingModel, types::ColorDepth},
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

fn active_plaintext(content: &str) -> String {
    format!(
        r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
    )
}

fn active_markdown(content: &str) -> String {
    format!(
        r#"{{"active":true,"content":"{content}","contentType":"markdown","origin":{{"BufferPosition":{{"buffer_id":2,"line":3,"col":0}}}}}}"#
    )
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

fn render_hover(module: &HoverModule, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &TestCaps);
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn hover_identity() {
    let m = HoverModule::new();
    assert_eq!(m.id(), "hover");
    assert_eq!(m.kind(), "hover");
    assert_eq!(m.name(), "Hover");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn hover_default() {
    let m = HoverModule::default();
    assert!(!m.active);
}

#[test]
fn hover_chrome_role() {
    let m = HoverModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 45);
}

#[test]
fn hover_lifecycle() {
    let mut m = HoverModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates_plaintext() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo() -> bool"));
    assert!(m.active);
    assert_eq!(m.lines.len(), 1);
    assert_eq!(m.lines[0], "fn foo() -> bool");
    assert_eq!(m.content_type, ContentType::Plaintext);
    assert_eq!(m.origin_line, 5);
    assert_eq!(m.origin_col, 10);
}

#[test]
fn notification_activates_markdown() {
    let mut m = HoverModule::new();
    m.on_notification(&active_markdown("**bold**"));
    assert!(m.active);
    assert_eq!(m.content_type, ContentType::Markdown);
    assert_eq!(m.origin_line, 3);
}

#[test]
fn notification_deactivates() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    m.on_notification(&inactive());
    assert!(!m.active);
    assert!(m.lines.is_empty());
}

#[test]
fn notification_invalid_json() {
    let mut m = HoverModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_empty_content_deactivates() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

#[test]
fn notification_multiline() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"line1\nline2\nline3","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert_eq!(m.lines.len(), 3);
}

#[test]
fn notification_defaults_content_type() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"hello","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert_eq!(m.content_type, ContentType::Plaintext);
}

#[test]
fn notification_no_origin_keeps_defaults() {
    let mut m = HoverModule::new();
    m.on_notification(r#"{"active":true,"content":"hello","contentType":"plaintext"}"#);
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_overwrites() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("first"));
    m.on_notification(&active_markdown("second"));
    assert_eq!(m.lines[0], "second");
    assert_eq!(m.content_type, ContentType::Markdown);
}

#[test]
fn notification_caps_at_max_lines() {
    let mut m = HoverModule::new();
    let many_lines: Vec<&str> = (0..30).map(|_| "line").collect();
    let content = many_lines.join("\\n");
    let data = format!(
        r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":0,"col":0}}}}}}"#
    );
    m.on_notification(&data);
    assert_eq!(m.lines.len(), MAX_LINES);
}

#[test]
fn notification_deactivation_clears_origin() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    m.on_notification(&inactive());
    assert_eq!(m.origin_line, 0);
    assert_eq!(m.origin_col, 0);
}

#[test]
fn notification_null_content() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":null,"contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
    );
    assert!(!m.active);
}

// =============================================================================
// popup_width tests
// =============================================================================

#[test]
fn popup_width_min() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(80), MIN_WIDTH);
}

#[test]
fn popup_width_adapts_to_content() {
    let mut m = HoverModule::new();
    m.lines = vec!["a".repeat(30)];
    assert_eq!(m.popup_width(80), 34);
}

#[test]
fn popup_width_clamped_to_max() {
    let mut m = HoverModule::new();
    m.lines = vec!["a".repeat(200)];
    assert_eq!(m.popup_width(80), 48);
}

#[test]
fn popup_width_narrow_terminal() {
    let mut m = HoverModule::new();
    m.lines = vec!["hello".to_owned()];
    assert_eq!(m.popup_width(22), MIN_WIDTH);
}

#[test]
fn popup_width_zero() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(0), 0);
}

#[test]
fn popup_width_tiny() {
    let m = HoverModule::new();
    assert_eq!(m.popup_width(5), 5);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = HoverModule::new();
    let surface = render_hover(&m, 80, 24);
    assert_eq!(surface.char_at(0, 0), ' ');
}

#[test]
fn render_single_line() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("fn foo()"));
    let surface = render_hover(&m, 80, 24);
    // Origin line=5, col=10. Popup at (10, 6)
    assert_eq!(surface.char_at(10, 6), '\u{256D}');
}

#[test]
fn render_border_color_plaintext() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    let surface = render_hover(&m, 80, 24);
    assert_eq!(surface.style_at(10, 6).fg, Some(Color::Grey));
}

#[test]
fn render_border_color_markdown() {
    let mut m = HoverModule::new();
    m.on_notification(&active_markdown("**bold**"));
    let surface = render_hover(&m, 80, 24);
    // Origin line=3, col=0. Popup at (0, 4)
    assert_eq!(surface.style_at(0, 4).fg, Some(Color::Cyan));
}

#[test]
fn render_content_text() {
    let mut m = HoverModule::new();
    m.on_notification(&active_plaintext("hello"));
    let surface = render_hover(&m, 80, 24);
    // Content at (11, 7)
    assert_eq!(surface.char_at(11, 7), 'h');
    assert_eq!(surface.style_at(11, 7).fg, Some(Color::White));
}

#[test]
fn render_multiline() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"line1\nline2","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":0}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // popup_h = 4, at y=6, bottom border at y=9
    assert_eq!(surface.char_at(0, 9), '\u{2570}');
}

#[test]
fn render_popup_above_when_no_room_below() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"text","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":22,"col":0}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // anchor_y=22, 22+1+3=26>24, above: 22-3=19
    assert_eq!(surface.char_at(0, 19), '\u{256D}');
}

#[test]
fn render_clamps_x_to_screen() {
    let mut m = HoverModule::new();
    m.on_notification(
        r#"{"active":true,"content":"hello","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
    );
    let surface = render_hover(&m, 80, 24);
    // popup_w=20, max_x=60, origin_col=75>60
    assert_eq!(surface.char_at(60, 6), '\u{256D}');
}

// =============================================================================
// Type coverage
// =============================================================================

#[test]
fn content_type_debug_clone_eq() {
    let a = ContentType::Plaintext;
    let b = a;
    #[allow(clippy::clone_on_copy)]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(ContentType::Plaintext, ContentType::Markdown);
    assert!(format!("{a:?}").contains("Plaintext"));
}

#[test]
fn origin_debug_clone() {
    let o = Origin::BufferPosition {
        buffer_id: 1,
        line: 2,
        col: 3,
    };
    assert!(format!("{o:?}").contains("BufferPosition"));
    #[allow(clippy::redundant_clone)]
    let c = o.clone();
    let Origin::BufferPosition { line, .. } = c;
    assert_eq!(line, 2);
}

#[test]
fn payload_debug() {
    let p = HoverPayload {
        active: true,
        content: Some("test".into()),
        content_type: Some(ContentType::Plaintext),
        origin: None,
    };
    assert!(format!("{p:?}").contains("HoverPayload"));
}
