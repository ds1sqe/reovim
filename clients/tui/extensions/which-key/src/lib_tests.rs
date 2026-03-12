use {reovim_arch::clock::TestClock, reovim_driver_display::FrameBuffer};

use super::*;

// =========================================================================
// Helpers
// =========================================================================

fn activate_data() -> &'static str {
    r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top","category":"motion"},{"key":"d","command":"goto-definition","category":"motion"}]}"#
}

fn activate_data_single() -> &'static str {
    r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top","category":"motion"}]}"#
}

fn deactivate_data() -> &'static str {
    r#"{"active":false}"#
}

/// Create a which-key extension with zero delay for backward-compatible tests.
fn zero_delay_ext() -> WhichKeyExtension {
    WhichKeyExtension::with_delay(Duration::ZERO)
}

/// Create a which-key extension with a `TestClock` and given delay.
fn test_ext(delay_ms: u64) -> (WhichKeyExtension, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let ext = WhichKeyExtension::with_clock(clock.clone(), Duration::from_millis(delay_ms));
    (ext, clock)
}

// =========================================================================
// Original tests (updated to use zero-delay for backward compat)
// =========================================================================

#[test]
fn test_new_is_inactive() {
    let ext = WhichKeyExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "whichkey");
}

#[test]
fn test_default_is_inactive() {
    let ext = WhichKeyExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_activates() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(activate_data());

    // With zero delay, still not visible until tick()
    assert!(!ext.is_active());
    assert!(ext.server_active);
    assert_eq!(ext.prefix, "g");
    assert_eq!(ext.hints.len(), 2);
    assert_eq!(ext.hints[0].key, "g");
    assert_eq!(ext.hints[0].command, "goto-top");
    assert_eq!(ext.hints[1].key, "d");
    assert_eq!(ext.hints[1].command, "goto-definition");

    // tick() promotes to visible immediately with zero delay
    assert!(ext.tick());
    assert!(ext.is_active());
}

#[test]
fn test_apply_notification_deactivates() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(activate_data());
    ext.tick();
    assert!(ext.is_active());

    ext.apply_notification(deactivate_data());
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_invalid_json() {
    let mut ext = zero_delay_ext();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_missing_hints() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(r#"{"active":true,"prefix":"g"}"#);
    ext.tick();
    assert!(ext.is_active());
    assert!(ext.hints.is_empty());
}

#[test]
fn test_apply_notification_malformed_hint() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(
        r#"{"active":true,"prefix":"g","hints":[{"key":"g"},{"bad":"data"}]}"#,
    );
    ext.tick();
    assert!(ext.is_active());
    assert!(ext.hints.is_empty());
}

#[test]
fn test_render_not_shown_when_inactive() {
    let ext = zero_delay_ext();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_not_shown_with_empty_hints() {
    let mut ext = zero_delay_ext();
    ext.server_active = true;
    ext.visible = true;
    ext.prefix = "g".to_string();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_shows_popup() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(activate_data());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    // 2 hints + 1 category header ("motion") + 2 borders = 5
    let py = 24 - 1 - 5;

    assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}');
    assert_eq!(fb.get(px + pw - 1, py).unwrap().char, '\u{256E}');
    assert_eq!(fb.get(px, py + 4).unwrap().char, '\u{2570}');
}

#[test]
fn test_render_no_prefix() {
    let mut ext = zero_delay_ext();
    ext.server_active = true;
    ext.visible = true;
    ext.hints = vec![WhichKeyHint {
        key: "a".to_string(),
        command: "cmd-a".to_string(),
        category: String::new(),
    }];

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24 - 1 - 3;
    assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}');
}

#[test]
fn test_render_command_text() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    // 1 hint + 1 category header + 2 borders = 4
    let py = 24 - 1 - 4;
    let content_x = px + 2;
    let cmd_x = content_x + 7;
    // Row after border + category header
    let hint_row_y = py + 2;

    assert_eq!(fb.get(content_x, hint_row_y).unwrap().char, 'g');
    assert_eq!(fb.get(cmd_x, hint_row_y).unwrap().char, 'g');
}

#[test]
fn test_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(WhichKeyExtension::new());
    assert_eq!(ext.kind(), "whichkey");
    assert!(!ext.is_active());
}

// =========================================================================
// Delay state machine tests (TestClock-based)
// =========================================================================

#[test]
fn test_delay_not_visible_before_timeout() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    // Advance 400ms (less than 500ms delay)
    clock.advance(Duration::from_millis(400));
    assert!(!ext.tick());
    assert!(!ext.is_active());
}

#[test]
fn test_delay_visible_after_timeout() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    clock.advance(Duration::from_millis(500));
    assert!(ext.tick());
    assert!(ext.is_active());
}

#[test]
fn test_fast_completion_no_popup() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    // User completes sequence before delay
    clock.advance(Duration::from_millis(200));
    ext.apply_notification(deactivate_data());

    // tick() should not make it visible
    assert!(!ext.tick());
    assert!(!ext.is_active());
}

#[test]
fn test_reactivation_resets_timer() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    // Advance 400ms
    clock.advance(Duration::from_millis(400));
    assert!(!ext.tick());

    // Deactivate then reactivate
    ext.apply_notification(deactivate_data());
    ext.apply_notification(activate_data());

    // Only 100ms from reactivation — should NOT be visible
    clock.advance(Duration::from_millis(100));
    assert!(!ext.tick());
    assert!(!ext.is_active());

    // 400ms more — now 500ms total from reactivation
    clock.advance(Duration::from_millis(400));
    assert!(ext.tick());
    assert!(ext.is_active());
}

#[test]
fn test_tick_returns_true_on_transition() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    clock.advance(Duration::from_millis(500));
    // First tick: WAITING -> SHOWING, returns true
    assert!(ext.tick());
    // Second tick: already SHOWING, returns false
    assert!(!ext.tick());
}

#[test]
fn test_tick_returns_false_when_already_showing() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    clock.advance(Duration::from_millis(600));
    ext.tick();
    // Already visible — subsequent ticks return false
    assert!(!ext.tick());
    assert!(!ext.tick());
}

#[test]
fn test_zero_delay_shows_immediately() {
    let (mut ext, _clock) = test_ext(0);
    ext.apply_notification(activate_data());

    // Zero delay: first tick transitions immediately
    assert!(ext.tick());
    assert!(ext.is_active());
}

#[test]
fn test_tick_when_idle() {
    let (mut ext, _clock) = test_ext(500);
    // Extension is IDLE — tick should be a no-op
    assert!(!ext.tick());
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_while_showing_updates_hints() {
    let (mut ext, clock) = test_ext(500);
    ext.apply_notification(activate_data());

    clock.advance(Duration::from_millis(500));
    ext.tick();
    assert!(ext.is_active());
    assert_eq!(ext.hints.len(), 2);

    // Server sends updated hints while already showing
    ext.apply_notification(activate_data_single());
    // Still visible (server_active stays true, no deactivation)
    assert_eq!(ext.hints.len(), 1);
    assert_eq!(ext.hints[0].command, "goto-top");
}

// =========================================================================
// Category parsing tests
// =========================================================================

#[test]
fn test_apply_notification_parses_category() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(activate_data());
    assert_eq!(ext.hints[0].category, "motion");
    assert_eq!(ext.hints[1].category, "motion");
}

#[test]
fn test_apply_notification_missing_category_defaults_to_empty() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(
        r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"}]}"#,
    );
    assert_eq!(ext.hints[0].category, "");
}

// =========================================================================
// grouped_hints() tests
// =========================================================================

fn hint(key: &str, cmd: &str, cat: &str) -> WhichKeyHint {
    WhichKeyHint {
        key: key.to_string(),
        command: cmd.to_string(),
        category: cat.to_string(),
    }
}

#[test]
fn test_grouped_hints_single_category() {
    let hints = vec![
        hint("g", "goto-top", "motion"),
        hint("d", "goto-def", "motion"),
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[0].1.len(), 2);
}

#[test]
fn test_grouped_hints_multiple_categories() {
    let hints = vec![
        hint("g", "goto-top", "motion"),
        hint("d", "delete", "operator"),
        hint("w", "word-fwd", "motion"),
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 2);
    // motion comes before operator in CATEGORY_ORDER
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[0].1.len(), 2);
    assert_eq!(groups[1].0, "operator");
    assert_eq!(groups[1].1.len(), 1);
}

#[test]
fn test_grouped_hints_empty_category_becomes_other() {
    let hints = vec![
        hint("g", "goto-top", "motion"),
        hint("x", "unknown-cmd", ""),
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[1].0, "other");
}

#[test]
fn test_grouped_hints_all_empty_category() {
    let hints = vec![hint("g", "goto-top", ""), hint("d", "goto-def", "")];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "other");
}

#[test]
fn test_grouped_hints_category_ordering() {
    let hints = vec![
        hint("w", "win-cmd", "window"),
        hint("d", "delete", "operator"),
        hint("g", "goto-top", "motion"),
        hint("x", "custom", "zzz-custom"),
    ];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].0, "motion");
    assert_eq!(groups[1].0, "operator");
    assert_eq!(groups[2].0, "window");
    // Unknown category after known ones
    assert_eq!(groups[3].0, "zzz-custom");
}

#[test]
fn test_grouped_hints_unknown_categories_alphabetical() {
    let hints = vec![hint("b", "cmd-b", "zebra"), hint("a", "cmd-a", "alpha")];
    let groups = grouped_hints(&hints);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "alpha");
    assert_eq!(groups[1].0, "zebra");
}

#[test]
fn test_title_case() {
    assert_eq!(title_case("motion"), "Motion");
    assert_eq!(title_case("operator"), "Operator");
    assert_eq!(title_case(""), "");
    assert_eq!(title_case("a"), "A");
}

// =========================================================================
// Grouped render tests
// =========================================================================

#[test]
fn test_render_grouped_by_category() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(
        r#"{"active":true,"prefix":"g","hints":[
            {"key":"g","command":"goto-top","category":"motion"},
            {"key":"d","command":"delete","category":"operator"}
        ]}"#,
    );
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let content_x = px + 2;

    // popup_height = 2 (hints) + 2 (category headers) + 2 (borders) = 6
    let py = 24u16.saturating_sub(1 + 6);

    // First row should be category header "Motion"
    let header_y = py + 1;
    assert_eq!(fb.get(content_x, header_y).unwrap().char, 'M');

    // Second row should be hint "g"
    let hint_y = py + 2;
    assert_eq!(fb.get(content_x, hint_y).unwrap().char, 'g');

    // Third row should be category header "Operator"
    let header_y2 = py + 3;
    assert_eq!(fb.get(content_x, header_y2).unwrap().char, 'O');

    // Fourth row should be hint "d"
    let hint_y2 = py + 4;
    assert_eq!(fb.get(content_x, hint_y2).unwrap().char, 'd');
}

#[test]
fn test_render_no_category_flat_fallback() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(
        r#"{"active":true,"prefix":"g","hints":[
            {"key":"g","command":"goto-top"},
            {"key":"d","command":"goto-def"}
        ]}"#,
    );
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let content_x = px + 2;

    // All hints have empty category → flat layout, no headers
    // popup_height = 2 (hints) + 0 (headers) + 2 (borders) = 4
    let py = 24u16.saturating_sub(1 + 4);

    // First row should be hint "g" directly (no header)
    let hint_y = py + 1;
    assert_eq!(fb.get(content_x, hint_y).unwrap().char, 'g');
}

#[test]
fn test_render_single_named_category_shows_header() {
    let mut ext = zero_delay_ext();
    ext.apply_notification(
        r#"{"active":true,"prefix":"g","hints":[
            {"key":"g","command":"goto-top","category":"motion"},
            {"key":"d","command":"goto-def","category":"motion"}
        ]}"#,
    );
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let content_x = px + 2;

    // Single named category → has header row
    // popup_height = 2 (hints) + 1 (header) + 2 (borders) = 5
    let py = 24u16.saturating_sub(1 + 5);
    let header_y = py + 1;
    assert_eq!(fb.get(content_x, header_y).unwrap().char, 'M'); // "Motion"
}

// =========================================================================
// WhichKeyStyleConfig tests
// =========================================================================

#[test]
fn test_style_config_default() {
    let config = WhichKeyStyleConfig::default();
    assert_eq!(config.border_color, Color::DarkGrey);
    assert_eq!(config.title_color, Color::Yellow);
    assert_eq!(config.key_color, Color::Cyan);
    assert_eq!(config.desc_color, Color::White);
    assert_eq!(config.category_color, Color::DarkGrey);
}

#[test]
fn test_style_config_clone_eq() {
    let config = WhichKeyStyleConfig::default();
    let cloned = config.clone();
    assert_eq!(config, cloned);
}

#[test]
fn test_with_style_constructor() {
    let custom = WhichKeyStyleConfig {
        border_color: Color::Red,
        title_color: Color::Green,
        key_color: Color::Blue,
        desc_color: Color::Magenta,
        category_color: Color::Yellow,
    };
    let ext = WhichKeyExtension::with_style(custom.clone());
    assert_eq!(ext.style, custom);
    assert!(!ext.is_active());
}

// =========================================================================
// Render color tests
// =========================================================================

fn styled_ext(style: WhichKeyStyleConfig) -> WhichKeyExtension {
    let mut ext = zero_delay_ext();
    ext.style = style;
    ext
}

#[test]
fn test_render_uses_custom_border_color() {
    let style = WhichKeyStyleConfig {
        border_color: Color::Red,
        ..WhichKeyStyleConfig::default()
    };
    let mut ext = styled_ext(style);
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 4); // 1 hint + 1 header + 2 borders
    // Top-left corner border char
    let cell = fb.get(px, py).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Red));
}

#[test]
fn test_render_uses_custom_title_color() {
    let style = WhichKeyStyleConfig {
        title_color: Color::Magenta,
        ..WhichKeyStyleConfig::default()
    };
    let mut ext = styled_ext(style);
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 4);
    // Title is at px + 3, " g " format — the space + prefix
    let title_x = px + 3;
    let cell = fb.get(title_x, py).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Magenta));
}

#[test]
fn test_render_uses_custom_key_color() {
    let style = WhichKeyStyleConfig {
        key_color: Color::Green,
        ..WhichKeyStyleConfig::default()
    };
    let mut ext = styled_ext(style);
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 4);
    let content_x = px + 2;
    // First hint row is after the category header: py + 1 (header) + 1 (hint)
    let hint_y = py + 2;
    let cell = fb.get(content_x, hint_y).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Green));
}

#[test]
fn test_render_uses_custom_desc_color() {
    let style = WhichKeyStyleConfig {
        desc_color: Color::Blue,
        ..WhichKeyStyleConfig::default()
    };
    let mut ext = styled_ext(style);
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 4);
    // Command text at content_x + 7
    let cmd_x = px + 2 + 7;
    let hint_y = py + 2;
    let cell = fb.get(cmd_x, hint_y).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Blue));
}

#[test]
fn test_render_uses_custom_category_color() {
    let style = WhichKeyStyleConfig {
        category_color: Color::DarkMagenta,
        ..WhichKeyStyleConfig::default()
    };
    let mut ext = styled_ext(style);
    ext.apply_notification(activate_data_single());
    ext.tick();

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let py = 24u16.saturating_sub(1 + 4);
    let content_x = px + 2;
    // Category header is right after the top border
    let header_y = py + 1;
    let cell = fb.get(content_x, header_y).unwrap();
    assert_eq!(cell.style.fg, Some(Color::DarkMagenta));
}
