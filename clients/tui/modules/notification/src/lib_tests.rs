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

// =============================================================================
// Coverage gap: on_notification — missing "entries" key
// =============================================================================

#[test]
fn notification_missing_entries_key() {
    let (mut m, _clock) = test_module();
    m.on_notification(r#"{"other": "data"}"#);
    assert!(m.toasts.is_empty());
}

// =============================================================================
// Coverage gap: on_notification — entry missing "id" field
// =============================================================================

#[test]
fn notification_entry_missing_id() {
    let (mut m, _clock) = test_module();
    m.on_notification(r#"{"entries":[{"level":"info","title":"No ID"}]}"#);
    assert!(m.toasts.is_empty(), "entry without id should be skipped");
}

// =============================================================================
// Coverage gap: toast with body text
// =============================================================================

fn toast_with_body(id: u64, title: &str, body: &str) -> String {
    format!(
        r#"{{"entries":[{{"id":{id},"level":"info","title":"{title}","body":"{body}"}}]}}"#,
    )
}

#[test]
fn notification_with_body() {
    let (mut m, _clock) = test_module();
    m.on_notification(&toast_with_body(1, "Alert", "Something happened"));
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(m.toasts[0].body, "Something happened");
}

#[test]
fn render_toast_with_body() {
    let (mut m, _clock) = test_module();
    m.on_notification(&toast_with_body(1, "Alert", "Something happened"));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("Alert"), "should contain title: {text}");
    assert!(
        text.contains("Something happened"),
        "should contain body: {text}"
    );
}

// =============================================================================
// Coverage gap: toast with progress bar rendering
// =============================================================================

#[test]
fn render_progress_toast() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Building", 42, "compiling"));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("Building"), "should contain title: {text}");
    assert!(text.contains("42%"), "should contain percentage: {text}");
}

// =============================================================================
// Coverage gap: progress bar with empty detail
// =============================================================================

#[test]
fn render_progress_toast_no_detail() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Building", 75, ""));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("75%"), "should contain percentage: {text}");
}

// =============================================================================
// Coverage gap: progress bar with percent > 100 (clamped to 100)
// =============================================================================

#[test]
fn render_progress_toast_percent_over_100() {
    let (mut m, _clock) = test_module();
    // Manually craft JSON with percent > 100 but within u8 range
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"info","title":"Over","body":"","progress":{"percent":120,"detail":"test"}}]}"#,
    );
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(
        m.toasts[0].progress,
        Some((120, "test".to_string()))
    );
    // Rendering should still work (percent.min(100) in render_progress_bar)
    let surface = render(&m, 80, 24);
    assert!(surface.has_content());
}

// =============================================================================
// Coverage gap: progress bar with percent value > 255 (u8 overflow)
// =============================================================================

#[test]
fn notification_progress_percent_overflow_u8() {
    let (mut m, _clock) = test_module();
    // percent=300 overflows u8, u8::try_from returns Err, unwrap_or(100) kicks in
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"info","title":"Overflow","body":"","progress":{"percent":300,"detail":""}}]}"#,
    );
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(
        m.toasts[0].progress,
        Some((100, String::new())),
        "percent > 255 should fall back to 100"
    );
}

// =============================================================================
// Coverage gap: progress update with u8 overflow on existing toast
// =============================================================================

#[test]
fn notification_progress_update_overflow_u8() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "started"));
    // Update with percent=999 which overflows u8
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"info","title":"Build","body":"","progress":{"percent":999,"detail":"done"}}]}"#,
    );
    assert_eq!(
        m.toasts[0].progress,
        Some((100, "done".to_string())),
        "update with overflow should use 100"
    );
}

// =============================================================================
// Coverage gap: standalone toast with body + progress
// =============================================================================

fn toast_body_progress(id: u64, title: &str, body: &str, percent: u8, detail: &str) -> String {
    format!(
        r#"{{"entries":[{{"id":{id},"level":"warning","title":"{title}","body":"{body}","progress":{{"percent":{percent},"detail":"{detail}"}}}}]}}"#,
    )
}

#[test]
fn render_standalone_toast_with_body_and_progress() {
    let (mut m, _clock) = test_module();
    m.on_notification(&toast_body_progress(1, "Indexing", "workspace files", 33, "parsing"));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("Indexing"), "should contain title: {text}");
    assert!(
        text.contains("workspace files"),
        "should contain body: {text}"
    );
    assert!(text.contains("33%"), "should contain percentage: {text}");
}

// =============================================================================
// Coverage gap: grouped box with body + progress toasts
// =============================================================================

#[test]
fn render_grouped_box_with_body_and_progress() {
    let (mut m, _clock) = test_module();
    // Two toasts from the same source, one with body, one with progress
    m.on_notification(&{
        let entries = [
            r#"{"id":1,"level":"success","title":"Server ready","body":"rust-analyzer v0.3","source":"ra"}"#,
            r#"{"id":2,"level":"info","title":"Indexing","body":"","progress":{"percent":55,"detail":"crate X"},"source":"ra"}"#,
        ];
        format!(r#"{{"entries":[{},{}]}}"#, entries[0], entries[1])
    });

    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("ra"), "grouped box should show source name: {text}");
    assert!(text.contains("Server ready"), "should contain first title: {text}");
    assert!(
        text.contains("rust-analyzer v0.3"),
        "should contain first body: {text}"
    );
    assert!(text.contains("Indexing"), "should contain second title: {text}");
    assert!(text.contains("55%"), "should contain progress percentage: {text}");
}

// =============================================================================
// Coverage gap: MAX_VISIBLE toast limit
// =============================================================================

#[test]
fn render_max_visible_toasts() {
    let (mut m, _clock) = test_module();
    // Add 7 toasts (more than MAX_VISIBLE=5)
    let entries: Vec<String> = (1..=7)
        .map(|i| format!(r#"{{"id":{i},"level":"info","title":"Toast{i}","body":""}}"#))
        .collect();
    let data = format!(r#"{{"entries":[{}]}}"#, entries.join(","));
    m.on_notification(&data);
    assert_eq!(m.toasts.len(), 7);

    // Render — only MAX_VISIBLE=5 should be rendered (newest first)
    let surface = render(&m, 80, 40);
    let text: String = (0..40).map(|row| surface.text_at_row(row)).collect();
    // Newest (id=7) should be visible, oldest (id=1) may not be
    assert!(text.contains("Toast7"), "newest toast should be visible");
}

// =============================================================================
// Coverage gap: narrow surface rendering (toast_w clamping)
// =============================================================================

#[test]
fn render_narrow_surface() {
    let (mut m, _clock) = test_module();
    m.on_notification(&info_toast(1, "Hello World"));
    // Surface width=10 — toast_w = min(TOAST_WIDTH, 10-2) = 8
    let surface = render(&m, 10, 24);
    assert!(surface.has_content());
}

// =============================================================================
// Coverage gap: init method with ModuleContext
// =============================================================================

#[test]
fn init_returns_success() {
    use reovim_client_driver::testing::TestModuleContext;
    let mut m = NotificationModule::new();
    let test_ctx = TestModuleContext::builder().build();
    let result = m.init(&test_ctx.as_context());
    assert!(matches!(result, reovim_client_driver::ProbeResult::Success));
}

// =============================================================================
// Coverage gap: has_buffer_contrib returns false
// =============================================================================

#[test]
fn has_buffer_contrib_is_false() {
    let m = NotificationModule::new();
    assert!(!m.has_buffer_contrib());
}

// =============================================================================
// Coverage gap: progress bar at 0% and 100%
// =============================================================================

#[test]
fn render_progress_toast_zero_percent() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Starting", 0, "init"));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("0%"), "should contain 0%: {text}");
}

#[test]
fn render_progress_toast_hundred_percent() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Done", 100, "finished"));
    let surface = render(&m, 80, 24);
    let text: String = (0..24).map(|row| surface.text_at_row(row)).collect();
    assert!(text.contains("100%"), "should contain 100%: {text}");
}

// =============================================================================
// Coverage gap: level-specific rendering (all 4 levels as standalone toasts)
// =============================================================================

#[test]
fn render_success_toast() {
    let (mut m, _clock) = test_module();
    m.on_notification(r#"{"entries":[{"id":1,"level":"success","title":"OK","body":""}]}"#);
    let surface = render(&m, 80, 24);
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Green));
}

#[test]
fn render_warning_toast() {
    let (mut m, _clock) = test_module();
    m.on_notification(r#"{"entries":[{"id":1,"level":"warning","title":"Warn","body":""}]}"#);
    let surface = render(&m, 80, 24);
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Yellow));
}

// =============================================================================
// Coverage gap: on_notification — progress update without "progress" field
// (existing toast found but no progress to update)
// =============================================================================

#[test]
fn notification_update_existing_toast_no_progress_change() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "compiling"));
    // Re-send same toast id but WITHOUT progress field — no update to progress
    m.on_notification(r#"{"entries":[{"id":1,"level":"info","title":"Build","body":""}]}"#);
    // The toast is found by id, but no "progress" field in the entry, so progress stays
    assert_eq!(
        m.toasts[0].progress,
        Some((50, "compiling".to_string())),
        "progress should remain unchanged when not in update"
    );
}

// =============================================================================
// Coverage gap: on_notification — progress update with missing percent/detail
// =============================================================================

#[test]
fn notification_progress_update_missing_percent() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "compiling"));
    // Update with progress object but no percent field
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"info","title":"Build","body":"","progress":{"detail":"linking"}}]}"#,
    );
    // percent defaults to 0 via unwrap_or(0), then u8::try_from(0)=Ok(0)
    assert_eq!(
        m.toasts[0].progress,
        Some((0, "linking".to_string()))
    );
}

// =============================================================================
// Coverage gap: on_notification — new toast with progress missing detail
// =============================================================================

#[test]
fn notification_new_toast_progress_no_detail() {
    let (mut m, _clock) = test_module();
    m.on_notification(
        r#"{"entries":[{"id":1,"level":"info","title":"Build","body":"","progress":{"percent":80}}]}"#,
    );
    assert_eq!(m.toasts.len(), 1);
    assert_eq!(
        m.toasts[0].progress,
        Some((80, String::new())),
        "missing detail should default to empty string"
    );
}

// =============================================================================
// Coverage gap: grouped box highest-priority: Success level
// =============================================================================

#[test]
fn grouped_box_border_success_priority() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("info", 1, "Starting", Some("ra")),
        ("success", 2, "Ready", Some("ra")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 24);
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    // Success > Info, so border should be Green
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Green));
}

// =============================================================================
// Coverage gap: grouped box with Warning priority
// =============================================================================

#[test]
fn grouped_box_border_warning_priority() {
    let (mut m, _clock) = test_module();
    let data = multi_entry_json(&[
        ("success", 1, "Ready", Some("ra")),
        ("warning", 2, "Slow", Some("ra")),
    ]);
    m.on_notification(&data);

    let surface = render(&m, 80, 24);
    let toast_w = TOAST_WIDTH.min(78);
    let toast_x = 80 - toast_w - 1;
    // Warning > Success, so border should be Yellow
    assert_eq!(surface.style_at(toast_x, 1).fg, Some(Color::Yellow));
}

// =============================================================================
// Coverage gap: render_grouped_box with long source label truncation
// =============================================================================

#[test]
fn render_grouped_box_source_label_truncation() {
    let (mut m, _clock) = test_module();
    // Source name longer than toast width
    let long_source = "a".repeat(50);
    let data = format!(
        r#"{{"entries":[{{"id":1,"level":"info","title":"T","body":"","source":"{long_source}"}}]}}"#,
    );
    m.on_notification(&data);
    let surface = render(&m, 80, 24);
    // Should render without panic; label is truncated
    assert!(surface.has_content());
}

// =============================================================================
// Coverage gap: very small surface — progress bar with zero available width
// =============================================================================

#[test]
fn render_progress_bar_zero_available_width() {
    let (mut m, _clock) = test_module();
    m.on_notification(&progress_toast(1, "Build", 50, "test"));
    // Surface width=6 — toast_w = min(40, 6-2)=4, content_width = 4-4=0
    // In render_progress_bar: available = 0 => early return
    let surface = render(&m, 6, 24);
    // Should not panic
    assert!(surface.has_content());
}
