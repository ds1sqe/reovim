use {
    super::*,
    reovim_client_driver::{
        reovim_arch::clock::TestClock,
        testing::{MockPlatformCapabilities, RecordingSurface},
    },
};

// =============================================================================
// Helpers
// =============================================================================

fn test_module() -> (WhichKeyModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = WhichKeyModule::with_clock(clock.clone(), Duration::from_millis(100));
    (module, clock)
}

fn render(module: &WhichKeyModule, w: u16, h: u16) -> RecordingSurface {
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
