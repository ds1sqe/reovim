use std::{sync::Arc, time::Duration};

use reovim_arch::clock::TestClock;

use {
    super::*,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::CellCapability,
};

// =============================================================================
// Helpers
// =============================================================================

// Plan 22 / 17-β.2b-impl-b pilot migration: tests now construct a
// CellCapability (which implements RenderSurface since Plan 21)
// directly, instead of using the driver's RecordingSurface test
// fixture. The two small helpers below replace RecordingSurface's
// inspection API (`has_content` + `char_at`) using CellCapability's
// `iter` + `get_cell` methods — identical semantics, no behavioural
// change.

fn has_content(g: &CellCapability) -> bool {
    g.iter().any(|(_, c)| c.ch != ' ')
}

fn char_at(g: &CellCapability, x: u16, y: u16) -> char {
    g.get_cell(x, y).map_or(' ', |c| c.ch)
}

fn render(module: &LandingModule, w: u16, h: u16) -> CellCapability {
    let mut surface = CellCapability::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
}

fn test_module() -> (LandingModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = LandingModule::with_clock(clock.clone());
    (module, clock)
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

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_on_normal_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 80, 24);
    assert!(has_content(&surface), "Landing screen should render content");
}

#[test]
fn render_on_small_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 20, 5);
    assert!(!has_content(&surface), "Small terminal should not render landing");
}

#[test]
fn render_skipped_when_dismissed() {
    let mut m = LandingModule::new();
    m.on_cursor_update(BufferId(0), 0, 0);
    let surface = render(&m, 80, 24);
    assert!(!has_content(&surface), "Dismissed extension should not render");
}

#[test]
fn render_content_centered() {
    let m = LandingModule::new();
    let surface = render(&m, 120, 40);
    let box_x: u16 = (120 - BOX_WIDTH) / 2;
    let box_y: u16 = (40 - 17) / 2;
    // Top-left corner should be U+256D
    assert_eq!(char_at(&surface, box_x, box_y), '\u{256D}');
    // Top-right corner should be U+256E
    assert_eq!(
        char_at(&surface, box_x + BOX_WIDTH - 1, box_y),
        '\u{256E}',
    );
}

#[test]
fn render_large_terminal() {
    let m = LandingModule::new();
    let surface = render(&m, 200, 60);
    assert!(has_content(&surface));
}

// =============================================================================
// Animation tests (#657)
// =============================================================================

#[test]
fn tick_returns_false_before_first_frame() {
    let (mut m, clock) = test_module();
    clock.advance(Duration::from_millis(100));
    assert!(!m.tick());
}

#[test]
fn tick_returns_true_after_breathing_frame() {
    let (mut m, clock) = test_module();
    clock.advance(Duration::from_millis(500));
    assert!(m.tick());
}

#[test]
fn breathing_frames_cycle() {
    let (mut m, clock) = test_module();

    // Frame 0 is the initial state
    assert_eq!(m.animation_color(), BREATHING_COLORS[0]);

    // Advance through all breathing frames
    for (i, expected_color) in BREATHING_COLORS.iter().enumerate().skip(1) {
        clock.advance(Duration::from_millis(500));
        assert!(m.tick());
        assert_eq!(m.animation_color(), *expected_color, "frame {i} mismatch");
    }

    // Wraps around
    clock.advance(Duration::from_millis(500));
    assert!(m.tick());
    assert_eq!(m.animation_color(), BREATHING_COLORS[0]);
}

#[test]
fn roar_triggers_after_interval() {
    let (mut m, clock) = test_module();

    // Advance past the roar interval
    clock.advance(Duration::from_secs(8));
    // First tick advances breathing frame
    assert!(m.tick());
    // Roar should now be active
    assert!(m.roar_active);
}

#[test]
fn roar_plays_then_returns_to_breathing() {
    let (mut m, clock) = test_module();

    // Trigger roar
    clock.advance(Duration::from_secs(8));
    m.tick();
    assert!(m.roar_active);

    // Play through roar frames
    for _ in 0..ROAR_COLORS.len() {
        clock.advance(Duration::from_millis(100));
        m.tick();
    }

    // Should be back to breathing
    assert!(!m.roar_active);
}

#[test]
fn tick_returns_false_after_dismissal() {
    let (mut m, clock) = test_module();
    m.on_cursor_update(BufferId(0), 0, 0);
    clock.advance(Duration::from_millis(500));
    assert!(!m.tick());
}

#[test]
fn with_clock_constructor() {
    let clock = Arc::new(TestClock::new());
    let m = LandingModule::with_clock(clock);
    assert!(!m.dismissed);
    assert_eq!(m.current_frame, 0);
    assert!(!m.roar_active);
}

#[test]
fn animation_color_breathing() {
    let (m, _clock) = test_module();
    // Initial frame is breathing frame 0
    assert_eq!(m.animation_color(), BREATHING_COLORS[0]);
}

#[test]
fn animation_color_roar() {
    let (mut m, clock) = test_module();
    // Trigger roar
    clock.advance(Duration::from_secs(8));
    m.tick();
    assert!(m.roar_active);
    assert_eq!(m.animation_color(), ROAR_COLORS[0]);
}

#[test]
fn render_uses_animation_color() {
    let (mut m, clock) = test_module();
    // Advance to a different frame
    clock.advance(Duration::from_millis(500));
    m.tick();
    // Should render without panic using the animation color
    let surface = render(&m, 80, 24);
    assert!(has_content(&surface));
}
