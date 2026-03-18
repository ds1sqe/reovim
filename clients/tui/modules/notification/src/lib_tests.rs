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

fn test_module() -> (NotificationModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = NotificationModule::with_clock(clock.clone(), Duration::from_secs(2));
    (module, clock)
}

fn render(module: &NotificationModule, w: u16, h: u16) -> RecordingSurface {
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

fn info_toast(id: u64, title: &str) -> String {
    format!(r#"{{"entries":[{{"id":{id},"level":"info","title":"{title}","body":""}}]}}"#)
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
    m.on_notification(r#"{"entries":[{"id":1,"level":"error","title":"Error"}]}"#);
    let surface = render(&m, 80, 24);
    // Toast at top-right, border should be red
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Red));
}

#[test]
fn render_level_icons() {
    // Each level has a non-empty Nerd Font icon (single Unicode codepoint)
    for level in [Level::Info, Level::Success, Level::Warning, Level::Error] {
        let icon = NotificationModule::level_icon(level);
        assert!(!icon.is_empty(), "{level:?} should have a non-empty icon");
        assert_eq!(icon.chars().count(), 1, "{level:?} icon should be a single char");
    }
    // All four levels map to distinct icons
    let icons: std::collections::HashSet<_> =
        [Level::Info, Level::Success, Level::Warning, Level::Error]
            .iter()
            .map(|l| NotificationModule::level_icon(*l))
            .collect();
    assert_eq!(icons.len(), 4, "each level should have a distinct icon");
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

// =============================================================================
// Source field and grouped rendering tests (#691)
// =============================================================================

fn sourced_toast(id: u64, title: &str, source: &str) -> String {
    format!(
        r#"{{"entries":[{{"id":{id},"level":"info","title":"{title}","body":"","source":"{source}"}}]}}"#,
    )
}

fn multi_entry_json(entries: &[(&str, u64, &str, Option<&str>)]) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|(level, id, title, source)| {
            let source_field = source
                .map(|s| format!(r#","source":"{s}""#))
                .unwrap_or_default();
            format!(r#"{{"id":{id},"level":"{level}","title":"{title}","body":""{source_field}}}"#)
        })
        .collect();
    format!(r#"{{"entries":[{}]}}"#, items.join(","))
}

#[test]
fn notification_parses_source() {
    let (mut m, _clock) = test_module();
    m.on_notification(&sourced_toast(1, "Ready", "rust-analyzer"));
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(m.toasts[0].source.as_deref(), Some("rust-analyzer"));
}

#[test]
fn notification_no_source_is_none() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello"));
    assert!(m.toasts[0].source.is_none());
}

#[test]
fn render_grouped_source() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("success", 1, "Server ready", Some("rust-analyzer")),
        ("info", 2, "Indexing", Some("rust-analyzer")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 24);
    assert!(surface.has_content());
    // Source name should appear in the rendered output
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("rust-analyzer"), "grouped box should show source name: {text}");
}

#[test]
fn render_mixed_sources_and_standalone() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("info", 1, "Indexing", Some("rust-analyzer")),
        ("success", 2, "File saved", None),
        ("info", 3, "cargo check", Some("rust-analyzer")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    // Both grouped and standalone content visible
    assert!(text.contains("rust-analyzer"));
    assert!(text.contains("File saved"));
}

#[test]
fn render_multiple_source_groups() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("info", 1, "Indexing", Some("rust-analyzer")),
        ("info", 2, "Checking", Some("gopls")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 30);
    let text: String = (0..30).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("rust-analyzer"));
    assert!(text.contains("gopls"));
}

#[test]
fn grouped_box_border_uses_highest_priority_level() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("info", 1, "Starting", Some("ra")),
        ("error", 2, "Failed", Some("ra")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 24);
    // The border should use error color (Red) since Error > Info
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    // Top border at y=1 should have the border color
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Red));
}
