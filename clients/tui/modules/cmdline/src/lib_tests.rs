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

    fn style_at(&self, x: u16, y: u16) -> &Style {
        &self.cells[y as usize][x as usize].1
    }

    fn char_at(&self, x: u16, y: u16) -> char {
        self.cells[y as usize][x as usize].0
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

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        if (x as usize) < self.width as usize && (y as usize) < self.height as usize {
            self.cells[y as usize][x as usize].1 = style;
        }
    }

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

fn render(module: &CmdlineModule, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &TestPlatformCaps);
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = CmdlineModule::new();
    assert_eq!(m.id(), "cmdline");
    assert_eq!(m.kind(), "cmdline");
    assert_eq!(m.name(), "Cmdline");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = CmdlineModule::default();
    assert!(!m.active);
}

// =============================================================================
// Chrome role tests
// =============================================================================

#[test]
fn chrome_role() {
    let m = CmdlineModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 90);
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn lifecycle() {
    let mut m = CmdlineModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
    assert!(m.active);
    assert_eq!(m.prompt, ":");
    assert_eq!(m.input, "wq");
    assert_eq!(m.cursor, 2);
}

#[test]
fn notification_deactivates() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
    assert!(m.active);

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.active);
}

#[test]
fn notification_invalid_json() {
    let mut m = CmdlineModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_with_completions() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq","wall"],"completion_index":0}"#,
    );
    assert_eq!(m.completions.len(), 3);
    assert_eq!(m.completions[0], "write");
    assert_eq!(m.completions[1], "wq");
    assert_eq!(m.completions[2], "wall");
    assert_eq!(m.completion_index, Some(0));
}

#[test]
fn notification_search_prompt() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":"/","input":"hello","cursor":5}"#);
    assert_eq!(m.prompt, "/");
    assert_eq!(m.input, "hello");
    assert_eq!(m.cursor, 5);
}

// =============================================================================
// Cursor position tests
// =============================================================================

#[test]
fn cursor_position_when_active() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let pos = m.cursor_position(80, 24);
    assert!(pos.is_some());
    let (x, y) = pos.unwrap();
    // y should be 2 (popup at y=1, content at y=2)
    assert_eq!(y, 2);
    // x = popup_x + 2 (border) + 1 (prompt ":") + 2 (cursor pos)
    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    assert_eq!(x, px + 2 + 1 + 2);
}

#[test]
fn cursor_position_when_inactive() {
    let m = CmdlineModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = CmdlineModule::new();
    let surface = render(&m, 80, 24);
    assert!(!surface.has_content());
}

#[test]
fn render_shows_popup() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let surface = render(&m, 80, 24);
    assert!(surface.has_content());

    // Check border at py=1
    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    assert_eq!(surface.char_at(px, 1), '\u{256D}'); // top-left corner
    assert_eq!(surface.char_at(px + pw - 1, 1), '\u{256E}'); // top-right corner

    // Check prompt character
    let content_x = px + 2;
    assert_eq!(surface.char_at(content_x, 2), ':');
}

#[test]
fn render_shows_completions() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":0}"#,
    );

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // Height = 3 (border+content+border) + 2 completions = 5
    // Bottom border should be at y = 1 + 4 = 5
    assert_eq!(surface.char_at(px, 5), '\u{2570}'); // bottom-left

    // Selected item should have indicator
    assert_eq!(surface.char_at(content_x, 3), '\u{25B8}'); // selected indicator

    // Second completion should have space indicator (not selected)
    assert_eq!(surface.char_at(content_x, 4), ' ');
}

#[test]
fn render_cursor_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"hello","cursor":2}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;
    // cursor at position 2 in input, prompt is 1 char
    let cursor_x = content_x + 1 + 2;

    // Cursor should have white bg and black fg (apply_style overrides)
    let style = surface.style_at(cursor_x, 2);
    assert_eq!(style.bg, Some(Color::White));
    assert_eq!(style.fg, Some(Color::Black));
}

#[test]
fn render_cursor_at_end_of_input() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"ab","cursor":2}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;
    // cursor at position 2 (past end of 2-char input), prompt is 1 char
    let cursor_x = content_x + 1 + 2;

    // When cursor is at end, a space with cursor style is written
    let style = surface.style_at(cursor_x, 2);
    assert_eq!(style.bg, Some(Color::White));
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(surface.char_at(cursor_x, 2), ' ');
}

#[test]
fn render_narrow_terminal() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"test","cursor":4}"#);

    // Narrow terminal (32 wide)
    let surface = render(&m, 32, 24);

    // Should render without panic
    let pw = reovim_client_driver::chrome_utils::popup_width(32);
    let px = reovim_client_driver::chrome_utils::popup_x(32, pw);
    assert_eq!(surface.char_at(px, 1), '\u{256D}');
}

#[test]
fn render_overflow_narrow_popup() {
    let mut m = CmdlineModule::new();
    // Long input + completions that exceed popup content width
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"this_is_a_very_long_command_that_overflows","cursor":42,"completions":["this_is_a_very_long_completion_candidate"],"completion_index":0}"#,
    );

    // Very narrow terminal -- content overflows trigger break branches
    let surface = render(&m, 16, 24);
    // Should render without panic -- overflow branches hit
    assert!(surface.has_content());
}

#[test]
fn render_prompt_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"","cursor":0}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // Prompt should have yellow foreground
    let style = surface.style_at(content_x, 2);
    assert_eq!(style.fg, Some(Color::Yellow));
}

#[test]
fn render_border_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"","cursor":0}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);

    // Border should have DarkGrey foreground
    let style = surface.style_at(px, 1);
    assert_eq!(style.fg, Some(Color::DarkGrey));
}

#[test]
fn render_selected_completion_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":1}"#,
    );

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // First completion (index 0) is not selected -- default style
    let first_style = surface.style_at(content_x, 3);
    assert_eq!(first_style.bg, None);

    // Second completion (index 1) is selected -- DarkGrey bg, White fg
    let second_style = surface.style_at(content_x, 4);
    assert_eq!(second_style.bg, Some(Color::DarkGrey));
    assert_eq!(second_style.fg, Some(Color::White));
}

#[test]
fn notification_no_completions_field() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"q","cursor":1}"#);
    assert!(m.completions.is_empty());
    assert!(m.completion_index.is_none());
}

#[test]
fn notification_defaults() {
    let mut m = CmdlineModule::new();
    // Minimal JSON -- only "active" provided
    m.on_notification(r#"{"active":true}"#);
    assert!(m.active);
    assert_eq!(m.prompt, ":");
    assert!(m.input.is_empty());
    assert_eq!(m.cursor, 0);
    assert!(m.completions.is_empty());
    assert!(m.completion_index.is_none());
}
