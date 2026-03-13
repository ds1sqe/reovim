use super::*;
use reovim_client_driver::{Insets, RenderingModel};
use reovim_client_driver::types::ColorDepth;
use reovim_client_driver::reovim_arch::clock::TestClock;

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

struct TestPlatformCaps;

impl PlatformCapabilities for TestPlatformCaps {
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

fn test_module() -> (NotificationModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = NotificationModule::with_clock(clock.clone(), Duration::from_secs(2));
    (module, clock)
}

fn render(module: &NotificationModule, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect { x: 0, y: 0, width: w, height: h };
    module.chrome_render(&mut surface, bounds, &TestPlatformCaps);
    surface
}

fn info_toast(id: u64, title: &str) -> String {
    format!(
        r#"{{"entries":[{{"id":{id},"level":"info","title":"{title}","body":""}}]}}"#,
    )
}

fn progress_toast(id: u64, title: &str, percent: u8, detail: &str) -> String {
    format!(
        r#"{{"entries":[{{"id":{id},"level":"info","title":"{title}","body":"","progress":{{"percent":{percent},"detail":"{detail}"}}}}]}}"#,
    )
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = NotificationModule::new();
    assert_eq!(m.id(), "notification");
    assert_eq!(m.kind(), "notification");
    assert_eq!(m.name(), "Notification");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_empty() {
    let m = NotificationModule::default();
    assert!(m.toasts.is_empty());
}

#[test]
fn chrome_role() {
    let m = NotificationModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 40);
}

#[test]
fn lifecycle() {
    let mut m = NotificationModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_adds_toast() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(m.toasts[0].title, "Hello");
}

#[test]
fn notification_deduplicates() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));
    m.on_notification(&info_toast(1, "Hello"));
    assert_eq!(m.toasts.len(), 1);
}

#[test]
fn notification_removes_missing_ids() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "First"));
    m.on_notification(&info_toast(2, "Second"));
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(m.toasts[0].id, 2);
}

#[test]
fn notification_progress_update() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "compiling"));
    assert_eq!(m.toasts[0].progress, Some((50, "compiling".to_string())));

    // Update progress
    m.on_notification(&progress_toast(1, "Build", 80, "linking"));
    assert_eq!(m.toasts[0].progress, Some((80, "linking".to_string())));
}

#[test]
fn notification_invalid_json() {
    let (mut m, _clock) = test_module();
    m.on_notification("not json{{{");
    assert!(m.toasts.is_empty());
}

#[test]
fn notification_levels() {
    let (mut m, _clock) = test_module();
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"success","title":"OK"},{"id":2,"level":"warning","title":"Warn"},{"id":3,"level":"error","title":"Err"},{"id":4,"level":"info","title":"Info"}]}"#,
    );
    assert_eq!(m.toasts.len(), 4);
    assert_eq!(m.toasts[0].level, Level::Success);
    assert_eq!(m.toasts[1].level, Level::Warning);
    assert_eq!(m.toasts[2].level, Level::Error);
    assert_eq!(m.toasts[3].level, Level::Info);
}

// =============================================================================
// Tick tests
// =============================================================================

#[test]
fn tick_dismisses_expired() {
    let (mut m, clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));
    assert_eq!(m.toasts.len(), 1);

    clock.advance(Duration::from_secs(3));
    let changed = m.tick();
    assert!(changed);
    assert!(m.toasts.is_empty());
}

#[test]
fn tick_keeps_fresh() {
    let (mut m, clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));

    clock.advance(Duration::from_secs(1));
    let changed = m.tick();
    assert!(!changed);
    assert_eq!(m.toasts.len(), 1);
}

#[test]
fn tick_keeps_progress() {
    let (mut m, clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "compiling"));

    clock.advance(Duration::from_secs(10));
    let changed = m.tick();
    assert!(!changed);
    assert_eq!(m.toasts.len(), 1);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_empty_no_op() {
    let m = NotificationModule::new();
    let surface = render(&m, 80, 24);
    assert!(!surface.has_content());
}

#[test]
fn render_shows_toast() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello World"));
    let surface = render(&m, 80, 24);
    assert!(surface.has_content());
}

#[test]
fn render_border_color_by_level() {
    let (mut m, _clock) = test_module();
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"error","title":"Error"}]}"#,
    );
    let surface = render(&m, 80, 24);
    // Toast at top-right, border should be red
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Red));
}

#[test]
fn render_level_icons() {
    // Test that level_icon returns correct chars
    assert_eq!(NotificationModule::level_icon(Level::Info), 'i');
    assert_eq!(NotificationModule::level_icon(Level::Success), '+');
    assert_eq!(NotificationModule::level_icon(Level::Warning), '!');
    assert_eq!(NotificationModule::level_icon(Level::Error), 'x');
}

#[test]
fn render_level_colors() {
    assert_eq!(NotificationModule::level_color(Level::Info), Color::Cyan);
    assert_eq!(NotificationModule::level_color(Level::Success), Color::Green);
    assert_eq!(NotificationModule::level_color(Level::Warning), Color::Yellow);
    assert_eq!(NotificationModule::level_color(Level::Error), Color::Red);
}

// =============================================================================
// Type coverage
// =============================================================================

#[test]
fn level_debug_clone_eq() {
    let a = Level::Info;
    let b = a;
    #[allow(clippy::clone_on_copy)]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_ne!(Level::Info, Level::Error);
    assert!(format!("{a:?}").contains("Info"));
}

#[test]
fn toast_debug() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));
    assert!(format!("{:?}", m.toasts[0]).contains("Toast"));
}

#[test]
fn parse_level_coverage() {
    assert_eq!(NotificationModule::parse_level("success"), Level::Success);
    assert_eq!(NotificationModule::parse_level("warning"), Level::Warning);
    assert_eq!(NotificationModule::parse_level("error"), Level::Error);
    assert_eq!(NotificationModule::parse_level("info"), Level::Info);
    assert_eq!(NotificationModule::parse_level("unknown"), Level::Info);
}
