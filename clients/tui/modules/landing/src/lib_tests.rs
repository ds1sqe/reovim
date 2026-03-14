use {
    super::*,
    reovim_client_driver::testing::{MockPlatformCapabilities, RecordingSurface},
};

// =============================================================================
// Helpers
// =============================================================================

fn render(module: &LandingModule, w: u16, h: u16) -> RecordingSurface {
    let mut surface = RecordingSurface::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
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
