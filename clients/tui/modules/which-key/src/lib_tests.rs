use {
    super::*,
    reovim_client_driver::{
        Insets, RenderingModel, reovim_arch::clock::TestClock, types::ColorDepth,
    },
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

fn test_module() -> (WhichKeyModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = WhichKeyModule::with_clock(clock.clone(), Duration::from_millis(100));
    (module, clock)
}

fn render(module: &WhichKeyModule, w: u16, h: u16) -> MockSurface {
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

fn hint_payload(prefix: &str, hints: &[(&str, &str, &str)]) -> String {
    let hints_json: Vec<String> = hints
        .iter()
        .map(|(key, cmd, cat)| format!(r#"{{"key":"{key}","command":"{cmd}","category":"{cat}"}}"#))
        .collect();
    format!(r#"{{"active":true,"prefix":"{prefix}","hints":[{}]}}"#, hints_json.join(","))
}

fn inactive() -> String {
    r#"{"active":false}"#.to_owned()
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = WhichKeyModule::new();
    assert_eq!(m.id(), "whichkey");
    assert_eq!(m.kind(), "whichkey");
    assert_eq!(m.name(), "WhichKey");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = WhichKeyModule::default();
    assert!(!m.visible);
    assert!(!m.server_active);
}

#[test]
fn chrome_role() {
    let m = WhichKeyModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 50);
}

#[test]
fn lifecycle() {
    let mut m = WhichKeyModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let (mut m, _clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));
    assert!(m.server_active);
    assert!(!m.visible); // Not visible yet - need tick
    assert_eq!(m.prefix, "g");
    assert_eq!(m.hints.len(), 1);
}

#[test]
fn notification_deactivates() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));
    clock.advance(Duration::from_millis(200));
    m.tick();
    assert!(m.visible);

    m.on_notification(&inactive());
    assert!(!m.server_active);
    assert!(!m.visible);
}

#[test]
fn notification_invalid_json() {
    let (mut m, _clock) = test_module();
    m.on_notification("not json{{{");
    assert!(!m.server_active);
}

// =============================================================================
// State machine / tick tests
// =============================================================================

#[test]
fn tick_promotes_to_visible() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));
    assert!(!m.visible);

    clock.advance(Duration::from_millis(200));
    let changed = m.tick();
    assert!(changed);
    assert!(m.visible);
}

#[test]
fn tick_no_change_before_delay() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));

    clock.advance(Duration::from_millis(50));
    let changed = m.tick();
    assert!(!changed);
    assert!(!m.visible);
}

#[test]
fn fast_completion_prevents_popup() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));

    clock.advance(Duration::from_millis(50));
    // User completed key sequence before delay
    m.on_notification(&inactive());
    assert!(!m.visible);

    clock.advance(Duration::from_millis(200));
    m.tick();
    assert!(!m.visible);
}

#[test]
fn reactivation_resets_timer() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));

    clock.advance(Duration::from_millis(50));
    m.on_notification(&inactive());
    m.on_notification(&hint_payload("z", &[("z", "center", "motion")]));

    // Timer restarted, 50ms is not enough
    clock.advance(Duration::from_millis(50));
    let changed = m.tick();
    assert!(!changed);
    assert!(!m.visible);

    // But 100ms more total should work
    clock.advance(Duration::from_millis(60));
    let changed = m.tick();
    assert!(changed);
    assert!(m.visible);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = WhichKeyModule::new();
    let surface = render(&m, 80, 24);
    assert!(!surface.has_content());
}

#[test]
fn render_shows_popup() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));
    clock.advance(Duration::from_millis(200));
    m.tick();

    let surface = render(&m, 80, 24);
    assert!(surface.has_content());
}

#[test]
fn render_no_popup_when_empty_hints() {
    let (mut m, clock) = test_module();
    m.on_notification(r#"{"active":true,"prefix":"g","hints":[]}"#);
    clock.advance(Duration::from_millis(200));
    m.tick();
    m.visible = true;

    let surface = render(&m, 80, 24);
    assert!(!surface.has_content());
}

#[test]
fn render_border_color() {
    let (mut m, clock) = test_module();
    m.on_notification(&hint_payload("g", &[("g", "goto_top", "motion")]));
    clock.advance(Duration::from_millis(200));
    m.tick();

    let surface = render(&m, 80, 24);
    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 1 + 1 + 2); // 1 statusline + 1 hint + 1 category header + 2 borders

    assert_eq!(surface.style_at(px, py).fg, Some(Color::DarkGrey));
}

// =============================================================================
// Category grouping tests
// =============================================================================

#[test]
fn grouped_hints_single_category() {
    let hints = vec![
        WhichKeyHint {
            key: "g".into(),
            command: "goto".into(),
            category: "motion".into(),
        },
        WhichKeyHint {
            key: "j".into(),
            command: "down".into(),
            category: "motion".into(),
        },
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[0].1.len(), 2);
}

#[test]
fn grouped_hints_multiple_categories() {
    let hints = vec![
        WhichKeyHint {
            key: "g".into(),
            command: "goto".into(),
            category: "motion".into(),
        },
        WhichKeyHint {
            key: "d".into(),
            command: "delete".into(),
            category: "operator".into(),
        },
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[1].0, "operator");
}

#[test]
fn grouped_hints_empty_category_becomes_other() {
    let hints = vec![WhichKeyHint {
        key: "g".into(),
        command: "goto".into(),
        category: String::new(),
    }];
    let groups = grouped_hints(&hints);
    assert_eq!(groups[0].0, "other");
}

#[test]
fn title_case_works() {
    assert_eq!(title_case("motion"), "Motion");
    assert_eq!(title_case(""), "");
    assert_eq!(title_case("a"), "A");
}

// =============================================================================
// Style config tests
// =============================================================================

#[test]
fn style_config_default() {
    let config = WhichKeyStyleConfig::default();
    assert_eq!(config.border_color, Color::DarkGrey);
    assert_eq!(config.title_color, Color::Yellow);
    assert_eq!(config.key_color, Color::Cyan);
    assert_eq!(config.desc_color, Color::White);
    assert_eq!(config.category_color, Color::DarkGrey);
}

#[test]
fn with_style_constructor() {
    let style = WhichKeyStyleConfig {
        border_color: Color::Red,
        ..WhichKeyStyleConfig::default()
    };
    let m = WhichKeyModule::with_style(style);
    assert_eq!(m.style.border_color, Color::Red);
}

#[test]
fn with_delay_constructor() {
    let m = WhichKeyModule::with_delay(Duration::from_secs(1));
    assert_eq!(m.show_delay, Duration::from_secs(1));
}

#[test]
fn style_config_debug_clone_eq() {
    let a = WhichKeyStyleConfig::default();
    #[allow(clippy::redundant_clone)]
    let b = a.clone();
    assert_eq!(a, b);
    assert!(format!("{a:?}").contains("WhichKeyStyleConfig"));
}
