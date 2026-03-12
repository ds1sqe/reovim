use {reovim_arch::clock::TestClock, reovim_driver_display::FrameBuffer};

use super::*;

// =========================================================================
// Helpers
// =========================================================================

fn make_data(entries_json: &str) -> String {
    format!(r#"{{"active":true,"entries":{entries_json}}}"#)
}

fn single_info() -> String {
    make_data(r#"[{"id":0,"level":"info","title":"Hello","body":""}]"#)
}

fn single_success() -> String {
    make_data(r#"[{"id":0,"level":"success","title":"Saved","body":""}]"#)
}

fn single_with_body() -> String {
    make_data(r#"[{"id":0,"level":"warning","title":"Warn","body":"details here"}]"#)
}

fn single_with_progress() -> String {
    make_data(
        r#"[{"id":0,"level":"info","title":"Building","body":"","progress":{"percent":35,"detail":"3/10"}}]"#,
    )
}

fn two_entries() -> String {
    make_data(
        r#"[{"id":0,"level":"info","title":"First","body":""},{"id":1,"level":"error","title":"Second","body":""}]"#,
    )
}

fn empty_entries() -> String {
    make_data("[]")
}

fn test_ext(timeout_ms: u64) -> (NotificationExtension, Arc<TestClock>) {
    let clock = Arc::new(TestClock::new());
    let ext = NotificationExtension::with_clock(clock.clone(), Duration::from_millis(timeout_ms));
    (ext, clock)
}

// =========================================================================
// Basic trait tests
// =========================================================================

#[test]
fn test_new_is_inactive() {
    let ext = NotificationExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "notification");
}

#[test]
fn test_default_is_inactive() {
    let ext = NotificationExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn test_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(NotificationExtension::new());
    assert_eq!(ext.kind(), "notification");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification tests
// =========================================================================

#[test]
fn test_apply_notification_adds_toast() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());
    assert!(ext.is_active());
    assert_eq!(ext.toasts.len(), 1);
    assert_eq!(ext.toasts[0].title, "Hello");
    assert_eq!(ext.toasts[0].level, Level::Info);
    assert!(!ext.toasts[0].is_progress);
}

#[test]
fn test_apply_notification_success_level() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_success());
    assert_eq!(ext.toasts[0].level, Level::Success);
}

#[test]
fn test_apply_notification_with_body() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_body());
    assert_eq!(ext.toasts[0].body, "details here");
    assert_eq!(ext.toasts[0].level, Level::Warning);
}

#[test]
fn test_apply_notification_with_progress() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_progress());
    assert!(ext.toasts[0].is_progress);
    let (pct, detail) = ext.toasts[0].progress.as_ref().unwrap();
    assert_eq!(*pct, 35);
    assert_eq!(detail, "3/10");
}

#[test]
fn test_apply_notification_two_entries() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&two_entries());
    assert_eq!(ext.toasts.len(), 2);
    assert_eq!(ext.toasts[0].title, "First");
    assert_eq!(ext.toasts[1].title, "Second");
    assert_eq!(ext.toasts[1].level, Level::Error);
}

#[test]
fn test_apply_notification_deduplicates_by_id() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());
    ext.apply_notification(&single_info());
    assert_eq!(ext.toasts.len(), 1);
}

#[test]
fn test_apply_notification_updates_progress_on_existing() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_progress());
    assert_eq!(ext.toasts[0].progress.as_ref().unwrap().0, 35);

    // Update progress to 70%
    let updated = make_data(
        r#"[{"id":0,"level":"info","title":"Building","body":"","progress":{"percent":70,"detail":"7/10"}}]"#,
    );
    ext.apply_notification(&updated);
    assert_eq!(ext.toasts.len(), 1);
    let (pct, detail) = ext.toasts[0].progress.as_ref().unwrap();
    assert_eq!(*pct, 70);
    assert_eq!(detail, "7/10");
}

#[test]
fn test_apply_notification_removes_server_dismissed() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&two_entries());
    assert_eq!(ext.toasts.len(), 2);

    // Server only sends id=1, id=0 was dismissed
    let remaining = make_data(r#"[{"id":1,"level":"error","title":"Second","body":""}]"#);
    ext.apply_notification(&remaining);
    assert_eq!(ext.toasts.len(), 1);
    assert_eq!(ext.toasts[0].id, 1);
}

#[test]
fn test_apply_notification_empty_entries_removes_all() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());
    assert!(ext.is_active());

    ext.apply_notification(&empty_entries());
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_invalid_json() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification("not json{{{");
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_missing_entries() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(r#"{"active":true}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_entry_missing_id() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&make_data(r#"[{"level":"info","title":"no id","body":""}]"#));
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_unknown_level_defaults_info() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&make_data(r#"[{"id":0,"level":"unknown","title":"test","body":""}]"#));
    assert_eq!(ext.toasts[0].level, Level::Info);
}

// =========================================================================
// tick tests
// =========================================================================

#[test]
fn test_tick_auto_dismiss_after_timeout() {
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_info());
    assert!(ext.is_active());

    clock.advance(Duration::from_millis(3999));
    assert!(!ext.tick()); // not expired yet

    clock.advance(Duration::from_millis(1));
    assert!(ext.tick()); // expired
    assert!(!ext.is_active());
}

#[test]
fn test_tick_no_dismiss_for_progress() {
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_with_progress());
    assert!(ext.is_active());

    clock.advance(Duration::from_secs(10));
    assert!(!ext.tick()); // progress toasts don't auto-dismiss
    assert!(ext.is_active());
}

#[test]
fn test_tick_returns_false_when_empty() {
    let (mut ext, _clock) = test_ext(4000);
    assert!(!ext.tick());
}

#[test]
fn test_tick_returns_false_when_no_change() {
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_info());

    clock.advance(Duration::from_millis(100));
    assert!(!ext.tick()); // not expired, no change
}

#[test]
fn test_tick_dismisses_only_expired() {
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_info()); // id=0, not progress

    clock.advance(Duration::from_secs(2));

    // Add a second toast
    ext.apply_notification(&make_data(
        r#"[{"id":0,"level":"info","title":"Hello","body":""},{"id":1,"level":"error","title":"New","body":""}]"#,
    ));

    clock.advance(Duration::from_millis(2001)); // id=0 is now 4001ms old, id=1 is 2001ms
    assert!(ext.tick()); // id=0 dismissed
    assert_eq!(ext.toasts.len(), 1);
    assert_eq!(ext.toasts[0].id, 1);
}

// =========================================================================
// render tests
// =========================================================================

#[test]
fn test_render_empty_no_op() {
    let ext = NotificationExtension::new();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_single_toast() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Toast should be in top-right corner
    let toast_x = 80 - TOAST_WIDTH - 1;
    // Top-left corner of border at (toast_x, 1)
    assert_eq!(fb.get(toast_x, 1).unwrap().char, '\u{256D}');
    // Top-right corner
    assert_eq!(fb.get(toast_x + TOAST_WIDTH - 1, 1).unwrap().char, '\u{256E}');
    // Bottom-left corner at y = 1 + height - 1 = 1 + 2 = 3
    assert_eq!(fb.get(toast_x, 3).unwrap().char, '\u{2570}');
}

#[test]
fn test_render_icon_and_title() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_success());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let toast_x = 80 - TOAST_WIDTH - 1;
    let content_x = toast_x + 2;
    // Icon for success is '+'
    assert_eq!(fb.get(content_x, 2).unwrap().char, '+');
    // Title starts at content_x + 2
    assert_eq!(fb.get(content_x + 2, 2).unwrap().char, 'S');
}

#[test]
fn test_render_with_body() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_body());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Toast height should be 4: border-top + title + body + border-bottom
    let toast_x = 80 - TOAST_WIDTH - 1;
    // Bottom border at row 1 + 3 = 4
    assert_eq!(fb.get(toast_x, 4).unwrap().char, '\u{2570}');
}

#[test]
fn test_render_with_progress_bar() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_progress());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Toast height: border-top + title + progress + border-bottom = 4
    let toast_x = 80 - TOAST_WIDTH - 1;
    assert_eq!(fb.get(toast_x, 4).unwrap().char, '\u{2570}');

    // Progress bar row at y=3 (1 + 2)
    let content_x = toast_x + 2;
    // "35%" label — '3' at content_x+1
    assert_eq!(fb.get(content_x + 1, 3).unwrap().char, '3');
}

#[test]
fn test_render_stacking() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&two_entries());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let toast_x = 80 - TOAST_WIDTH - 1;
    // First toast (newest=id1) at y=1
    assert_eq!(fb.get(toast_x, 1).unwrap().char, '\u{256D}');
    // Second toast (id0) starts after first toast height (3) + gap (1) = y=5
    assert_eq!(fb.get(toast_x, 5).unwrap().char, '\u{256D}');
}

#[test]
fn test_render_max_visible() {
    let (mut ext, _clock) = test_ext(4000);
    // Create 7 toasts
    let mut entries = Vec::new();
    for i in 0..7 {
        entries.push(format!(r#"{{"id":{i},"level":"info","title":"Toast {i}","body":""}}"#));
    }
    let data = make_data(&format!("[{}]", entries.join(",")));
    ext.apply_notification(&data);
    assert_eq!(ext.toasts.len(), 7);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Only MAX_VISIBLE (5) should be rendered
    let toast_x = 80 - TOAST_WIDTH - 1;
    // Each toast is 3 rows + 1 gap = 4 rows. 5 toasts = 5*3 + 4*1 = 19 rows.
    // Starting at y=1, last toast bottom at y = 1 + 19 - 1 = 19.
    // The 6th toast would start at y=21, which should NOT have a border
    assert_eq!(fb.get(toast_x, 1).unwrap().char, '\u{256D}'); // first
    // There should be no border at position for 6th toast
    // y for 6th = 1 + 5*(3+1) = 21
    assert_eq!(fb.get(toast_x, 21).unwrap().char, ' '); // no 6th toast
}

#[test]
fn test_render_narrow_terminal() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());

    // Terminal narrower than TOAST_WIDTH
    let mut fb = FrameBuffer::new(30, 24);
    ext.render(&mut fb);

    // Should still render with clamped width
    // toast_w = min(40, 30-2) = 28
    // toast_x = 30 - 28 - 1 = 1
    assert_eq!(fb.get(1, 1).unwrap().char, '\u{256D}');
}

#[test]
fn test_render_level_colors() {
    // Verify level_color returns correct colors
    assert_eq!(NotificationExtension::level_color(Level::Info), Color::Cyan);
    assert_eq!(NotificationExtension::level_color(Level::Success), Color::Green);
    assert_eq!(NotificationExtension::level_color(Level::Warning), Color::Yellow);
    assert_eq!(NotificationExtension::level_color(Level::Error), Color::Red);
}

#[test]
fn test_render_level_icons() {
    assert_eq!(NotificationExtension::level_icon(Level::Info), 'i');
    assert_eq!(NotificationExtension::level_icon(Level::Success), '+');
    assert_eq!(NotificationExtension::level_icon(Level::Warning), '!');
    assert_eq!(NotificationExtension::level_icon(Level::Error), 'x');
}

#[test]
fn test_parse_level_all_variants() {
    assert_eq!(NotificationExtension::parse_level("info"), Level::Info);
    assert_eq!(NotificationExtension::parse_level("success"), Level::Success);
    assert_eq!(NotificationExtension::parse_level("warning"), Level::Warning);
    assert_eq!(NotificationExtension::parse_level("error"), Level::Error);
    assert_eq!(NotificationExtension::parse_level("unknown"), Level::Info);
}

// =========================================================================
// progress bar rendering
// =========================================================================

#[test]
fn test_progress_bar_zero_percent() {
    let mut fb = FrameBuffer::new(60, 10);
    render_progress_bar(&mut fb, 2, 3, 30, 0, "");
    // "  0% " at (2,3)
    assert_eq!(fb.get(4, 3).unwrap().char, '0');
    // Bar starts at x=7, all empty blocks
    assert_eq!(fb.get(7, 3).unwrap().char, '\u{2591}');
}

#[test]
fn test_progress_bar_100_percent() {
    let mut fb = FrameBuffer::new(60, 10);
    render_progress_bar(&mut fb, 2, 3, 30, 100, "done");
    // All filled blocks
    assert_eq!(fb.get(7, 3).unwrap().char, '\u{2588}');
}

#[test]
fn test_progress_bar_with_detail() {
    let mut fb = FrameBuffer::new(60, 10);
    // x=2, width=30, detail="half" (4 chars + 1 space = 5 cols reserved)
    // available = 25, detail_cols = min(5, 12) = 5, bar_width = 20
    // bar_x = 7, detail_x = 7 + 20 + 1 = 28
    render_progress_bar(&mut fb, 2, 3, 30, 50, "half");
    assert_eq!(fb.get(28, 3).unwrap().char, 'h');
}

#[test]
fn test_progress_bar_zero_width() {
    let mut fb = FrameBuffer::new(10, 10);
    // Width too small for even the label — should not panic
    render_progress_bar(&mut fb, 0, 0, 3, 50, "");
    // No crash is the test
}

// =========================================================================
// Toast struct coverage
// =========================================================================

#[test]
fn test_level_debug() {
    let level = Level::Warning;
    assert_eq!(format!("{level:?}"), "Warning");
}

#[test]
fn test_level_clone_copy_eq() {
    let a = Level::Error;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_toast_debug() {
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_info());
    let toast = &ext.toasts[0];
    assert!(format!("{toast:?}").contains("Hello"));
}

// =========================================================================
// P0 coverage gap tests
// =========================================================================

#[test]
fn test_apply_notification_existing_toast_no_progress_field() {
    // Covers: apply_notification line 178 FALSE branch —
    // existing toast found by ID but update payload has no "progress" key.
    let (mut ext, _clock) = test_ext(4000);
    ext.apply_notification(&single_with_progress());
    assert_eq!(ext.toasts[0].progress.as_ref().unwrap().0, 35);

    // Re-send the same ID but without "progress" field
    let update = make_data(r#"[{"id":0,"level":"info","title":"Building","body":""}]"#);
    ext.apply_notification(&update);
    // Toast should still exist with original progress unchanged
    assert_eq!(ext.toasts.len(), 1);
    assert_eq!(ext.toasts[0].progress.as_ref().unwrap().0, 35);
}

#[test]
fn test_render_toast_with_body_and_progress() {
    // Covers: render combined body + progress bar layout (content_lines = 3, height = 5).
    let (mut ext, _clock) = test_ext(4000);
    let data = make_data(
        r#"[{"id":0,"level":"info","title":"Building","body":"compiling...","progress":{"percent":50,"detail":"5/10"}}]"#,
    );
    ext.apply_notification(&data);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let toast_x = 80 - TOAST_WIDTH - 1;
    // Height: border-top + title + body + progress + border-bottom = 5
    // Bottom border at y = 1 + 4 = 5
    assert_eq!(fb.get(toast_x, 5).unwrap().char, '\u{2570}');

    // Body at row y+2 = 3
    let content_x = toast_x + 2;
    assert_eq!(fb.get(content_x, 3).unwrap().char, 'c'); // "compiling..."

    // Progress bar at row y+3 = 4
    // " 50% " label — '5' at content_x+1
    assert_eq!(fb.get(content_x + 1, 4).unwrap().char, '5');
}

#[test]
fn test_progress_bar_bar_width_zero() {
    // Covers: render_progress_bar line 367 bar_width == 0 early return.
    // available=1, detail="x" → detail_cols=min(2, 0)=0... actually
    // we need available > 0 but available.saturating_sub(detail_cols) == 0.
    // width=6 → available=1, detail="x" → needed=2, detail_cols=min(2, 0)=0
    // That gives bar_width=1. To hit zero: we need detail_cols >= available.
    // width=7 → available=2, detail="x" → needed=2, detail_cols=min(2, 1)=1
    // bar_width=1. Still not zero. For bar_width=0:
    // width=6, detail="x" → available=1, needed=2, available/2=0, detail_cols=min(2,0)=0
    // bar_width=1. Can't do it easily without larger detail.
    // Actually with detail="xx" and width=7:
    // available=2, needed=3, detail_cols=min(3,1)=1, bar_width=1. Still 1.
    // With detail="x" and width=6: available=1, needed=2, min(2,0)=0, bar_width=1.
    // The second guard (bar_width == 0) requires detail_cols == available > 0.
    // That means: needed <= available/2 AND result == available.
    // Wait - detail_cols = needed.min(available / 2). For detail_cols == available:
    // needed >= available AND available/2 >= available → only if available == 0.
    // So the bar_width == 0 guard can only fire when available==0 was already returned?
    // No: detail_cols = needed.min(available/2), bar_width = available - detail_cols.
    // If available = 2 and detail is very long: needed=100, detail_cols=min(100,1)=1,
    // bar_width = 2-1 = 1. The minimum bar_width when available > 0 is
    // available - available/2 = ceil(available/2) >= 1.
    // So bar_width == 0 when available > 0 is unreachable by integer arithmetic.
    // This is dead code — the first `available == 0` guard catches it.
    // Just verify the first guard with a very tight width.
    let mut fb = FrameBuffer::new(10, 10);
    render_progress_bar(&mut fb, 0, 0, 5, 50, "x");
    // available = 0, first guard returns immediately
    // No crash, no bar rendered
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_long_title_truncated() {
    // Covers: truncate_end on title (line 308).
    let (mut ext, _clock) = test_ext(4000);
    let long_title = "A".repeat(100);
    let data =
        make_data(&format!(r#"[{{"id":0,"level":"info","title":"{long_title}","body":""}}]"#));
    ext.apply_notification(&data);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let toast_x = 80 - TOAST_WIDTH - 1;
    let content_x = toast_x + 2;
    // Title starts at content_x+2 (after icon+space)
    // content_width = TOAST_WIDTH - 4 = 36, max_title_len = 34
    // Should be truncated, last visible char at content_x+2+33
    assert_eq!(fb.get(content_x + 2, 2).unwrap().char, 'A');
    // Past content area should be the border, not 'A'
    let border_x = toast_x + TOAST_WIDTH - 1;
    assert_ne!(fb.get(border_x, 2).unwrap().char, 'A');
}

#[test]
fn test_render_long_body_truncated() {
    // Covers: truncate_end on body (line 316).
    let (mut ext, _clock) = test_ext(4000);
    let long_body = "B".repeat(100);
    let data =
        make_data(&format!(r#"[{{"id":0,"level":"info","title":"T","body":"{long_body}"}}]"#));
    ext.apply_notification(&data);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    let toast_x = 80 - TOAST_WIDTH - 1;
    let content_x = toast_x + 2;
    // Body at row 3
    assert_eq!(fb.get(content_x, 3).unwrap().char, 'B');
    // Should not overflow into border
    let border_x = toast_x + TOAST_WIDTH - 1;
    assert_ne!(fb.get(border_x, 3).unwrap().char, 'B');
}

#[test]
fn test_progress_bar_detail_cols_one_skips_detail() {
    // Covers: render_progress_bar line 393 detail_cols <= 1 FALSE branch.
    // With available=2 and detail="longdetail": needed=11, detail_cols=min(11, 1)=1.
    // detail_cols is 1, so !detail.is_empty() && detail_cols > 1 is false → detail not rendered.
    let mut fb = FrameBuffer::new(20, 10);
    render_progress_bar(&mut fb, 0, 0, 7, 50, "longdetail");
    // available = 7-5 = 2, detail_cols=min(11,1)=1, bar_width=1
    // Bar at x=5, only 1 cell wide
    let cell = fb.get(5, 0).unwrap();
    assert!(cell.char == '\u{2588}' || cell.char == '\u{2591}');
    // No detail text after bar (detail_x would be 7, which is beyond meaningful area)
    // The detail is skipped because detail_cols <= 1
    assert_eq!(fb.get(7, 0).unwrap().char, ' ');
}

#[test]
fn test_tick_repeated_after_dismiss() {
    // Covers: tick() returns true once on dismiss, then false on subsequent calls.
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_info());

    clock.advance(Duration::from_millis(4001));
    assert!(ext.tick()); // first call: toast removed → true
    assert!(!ext.is_active());
    assert!(!ext.tick()); // second call: no change → false
}

#[test]
fn test_reactivation_after_dismiss() {
    // Covers: new toasts added after previous batch expired get fresh displayed_at.
    let (mut ext, clock) = test_ext(4000);
    ext.apply_notification(&single_info());

    clock.advance(Duration::from_millis(4001));
    assert!(ext.tick());
    assert!(!ext.is_active());

    // Add new toast after dismiss
    ext.apply_notification(&make_data(r#"[{"id":5,"level":"success","title":"New","body":""}]"#));
    assert!(ext.is_active());

    // Should not be dismissed immediately (fresh displayed_at)
    assert!(!ext.tick());
    assert!(ext.is_active());
}

#[test]
fn test_progress_bar_percent_over_100() {
    // Covers: render_progress_bar with percent > 100
    // (exercises percent.min(100) guard on line 371).
    let mut fb = FrameBuffer::new(60, 10);
    render_progress_bar(&mut fb, 2, 3, 30, 200, "");
    // Should render the same as 100%
    // All filled blocks at bar position
    assert_eq!(fb.get(7, 3).unwrap().char, '\u{2588}');
}
