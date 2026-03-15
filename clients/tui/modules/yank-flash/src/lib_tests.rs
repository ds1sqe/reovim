use std::{sync::Arc, time::Duration};

use {reovim_arch::clock::TestClock, reovim_client_driver::ClientModule};

use super::*;

/// Create a test module with a controllable clock.
fn test_module() -> (YankFlashModule, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let module = YankFlashModule::with_clock(clock.clone());
    (module, clock)
}

/// Build a linewise yank notification JSON.
fn linewise_notification(start_line: usize, end_line: usize, sequence: u64) -> String {
    serde_json::json!({
        "bufferId": 1,
        "startLine": start_line,
        "endLine": end_line,
        "startCol": 0,
        "endCol": 0,
        "isLinewise": true,
        "sequence": sequence,
    })
    .to_string()
}

/// Build a characterwise yank notification JSON.
fn charwise_notification(
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
    sequence: u64,
) -> String {
    serde_json::json!({
        "bufferId": 1,
        "startLine": start_line,
        "endLine": end_line,
        "startCol": start_col,
        "endCol": end_col,
        "isLinewise": false,
        "sequence": sequence,
    })
    .to_string()
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn test_id() {
    let m = YankFlashModule::new();
    assert_eq!(m.id(), "yank-flash");
}

#[test]
fn test_kind() {
    let m = YankFlashModule::new();
    assert_eq!(m.kind(), "yank-flash");
}

#[test]
fn test_name() {
    let m = YankFlashModule::new();
    assert_eq!(m.name(), "Yank Flash");
}

#[test]
fn test_version() {
    let m = YankFlashModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn test_server_kinds() {
    let m = YankFlashModule::new();
    assert_eq!(m.server_kinds(), vec!["yank-flash"]);
}

#[test]
fn test_has_chrome_false() {
    let m = YankFlashModule::new();
    assert!(!m.has_chrome());
}

#[test]
fn test_default() {
    let m = YankFlashModule::default();
    assert_eq!(m.id(), "yank-flash");
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn test_init_succeeds() {
    use reovim_client_driver::testing::TestModuleContext;
    let mut m = YankFlashModule::new();
    let ctx = TestModuleContext::builder().build();
    assert!(matches!(m.init(&ctx.as_context()), ProbeResult::Success));
}

#[test]
fn test_exit_succeeds() {
    let mut m = YankFlashModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// Buffer contribution state
// =============================================================================

#[test]
fn test_has_buffer_contrib_false_initially() {
    let m = YankFlashModule::new();
    assert!(!m.has_buffer_contrib());
}

#[test]
fn test_has_buffer_contrib_true_during_flash() {
    let (mut m, _clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    assert!(m.has_buffer_contrib());
}

#[test]
fn test_has_buffer_contrib_false_after_expiry() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    clock.advance(Duration::from_millis(200));
    m.tick();
    assert!(!m.has_buffer_contrib());
}

// =============================================================================
// on_notification tests
// =============================================================================

#[test]
fn test_notification_linewise_triggers_flash() {
    let (mut m, _clock) = test_module();
    m.on_notification(&linewise_notification(5, 10, 1));
    assert!(m.active_flash.is_some());
    assert!(m.has_buffer_contrib());
}

#[test]
fn test_notification_characterwise_triggers_flash() {
    let (mut m, _clock) = test_module();
    m.on_notification(&charwise_notification(0, 5, 0, 10, 1));
    assert!(m.active_flash.is_some());
}

#[test]
fn test_notification_same_sequence_ignored() {
    let (mut m, _clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));

    // Same sequence again — should NOT restart
    let flash_start = m.active_flash.as_ref().unwrap().started_at;
    m.on_notification(&linewise_notification(10, 15, 1));

    // Flash should still be at the OLD range
    let flash = m.active_flash.as_ref().unwrap();
    assert_eq!(flash.start_line, 0);
    assert_eq!(flash.started_at, flash_start);
}

#[test]
fn test_notification_new_sequence_starts_new_flash() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    clock.advance(Duration::from_millis(50));
    m.on_notification(&linewise_notification(10, 15, 2));

    let flash = m.active_flash.as_ref().unwrap();
    assert_eq!(flash.start_line, 10);
    assert_eq!(flash.end_line, 15);
}

#[test]
fn test_notification_invalid_json_ignored() {
    let (mut m, _clock) = test_module();
    m.on_notification("not valid json");
    assert!(m.active_flash.is_none());
}

#[test]
fn test_notification_missing_sequence_ignored() {
    let (mut m, _clock) = test_module();
    m.on_notification(r#"{"startLine": 0, "endLine": 5}"#);
    assert!(m.active_flash.is_none());
}

// =============================================================================
// inline_decorations tests
// =============================================================================

#[test]
fn test_inline_decorations_empty_when_no_flash() {
    let m = YankFlashModule::new();
    assert!(m.inline_decorations(0).is_empty());
}

#[test]
fn test_inline_decorations_linewise() {
    let (mut m, _clock) = test_module();
    m.on_notification(&linewise_notification(2, 4, 1));

    // Lines in range should have decorations
    for line in 2..=4 {
        let decs = m.inline_decorations(line);
        assert_eq!(decs.len(), 1, "line {line} should have 1 decoration");
        assert_eq!(decs[0].col_start, 0);
        assert_eq!(decs[0].col_end, u16::MAX);
        assert_eq!(decs[0].style.bg, Some(FLASH_COLOR));
    }

    // Lines outside range should be empty
    assert!(m.inline_decorations(0).is_empty());
    assert!(m.inline_decorations(1).is_empty());
    assert!(m.inline_decorations(5).is_empty());
}

#[test]
fn test_inline_decorations_characterwise_single_line() {
    let (mut m, _clock) = test_module();
    m.on_notification(&charwise_notification(3, 5, 3, 10, 1));

    let decs = m.inline_decorations(3);
    assert_eq!(decs.len(), 1);
    assert_eq!(decs[0].col_start, 5);
    assert_eq!(decs[0].col_end, 10);
}

#[test]
fn test_inline_decorations_characterwise_multi_line() {
    let (mut m, _clock) = test_module();
    m.on_notification(&charwise_notification(2, 5, 4, 10, 1));

    // First line: start_col to end
    let first = m.inline_decorations(2);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].col_start, 5);
    assert_eq!(first[0].col_end, u16::MAX);

    // Middle line: full width
    let middle = m.inline_decorations(3);
    assert_eq!(middle.len(), 1);
    assert_eq!(middle[0].col_start, 0);
    assert_eq!(middle[0].col_end, u16::MAX);

    // Last line: start to end_col
    let last = m.inline_decorations(4);
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].col_start, 0);
    assert_eq!(last[0].col_end, 10);
}

#[test]
fn test_inline_decorations_empty_after_expiry() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 3, 1));
    assert!(!m.inline_decorations(0).is_empty());

    clock.advance(Duration::from_millis(200));
    m.tick();
    assert!(m.inline_decorations(0).is_empty());
}

// =============================================================================
// tick tests
// =============================================================================

#[test]
fn test_tick_returns_false_no_flash() {
    let (mut m, _clock) = test_module();
    assert!(!m.tick());
}

#[test]
fn test_tick_returns_false_before_duration() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    clock.advance(Duration::from_millis(100));
    assert!(!m.tick());
    assert!(m.active_flash.is_some());
}

#[test]
fn test_tick_returns_true_at_duration() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    clock.advance(Duration::from_millis(200));
    assert!(m.tick());
    assert!(m.active_flash.is_none());
}

#[test]
fn test_tick_returns_true_after_duration() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 5, 1));
    clock.advance(Duration::from_millis(500));
    assert!(m.tick());
}

#[test]
fn test_tick_clears_decorations() {
    let (mut m, clock) = test_module();
    m.on_notification(&linewise_notification(0, 2, 1));
    assert!(!m.decorations_by_line.is_empty());

    clock.advance(Duration::from_millis(200));
    m.tick();
    assert!(m.decorations_by_line.is_empty());
}
