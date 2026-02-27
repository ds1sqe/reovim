#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Which-key TUI extension.
//!
//! Displays a popup showing available key continuations when a prefix
//! key is held (e.g., `g` shows `gg`, `gd`, etc.).
//!
//! The popup appears after a configurable delay (default 500ms).
//! If the user completes the key sequence before the delay expires,
//! no popup is shown at all.
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use {
    reovim_arch::{
        Color,
        clock::{Clock, SystemClock},
    },
    reovim_driver_display::{
        Style,
        popup_utils::{popup_width, popup_x, render_box_border},
        render_backend::{RenderBackend, TuiExtension},
        ui::truncate_end,
    },
};

/// Default show-delay before the which-key popup appears.
const DEFAULT_SHOW_DELAY: Duration = Duration::from_millis(500);

/// Which-key popup extension with configurable show-delay.
///
/// # State Machine
///
/// ```text
/// IDLE: server_active=false, visible=false
///   |
///   | apply_notification(active=true)
///   v
/// WAITING: server_active=true, visible=false
///   |--- tick(): elapsed >= show_delay ---> SHOWING (visible=true)
///   |--- apply_notification(active=false) -> IDLE
///   v
/// SHOWING: server_active=true, visible=true
///   |
///   | apply_notification(active=false)
///   v
/// IDLE
/// ```
pub struct WhichKeyExtension {
    /// Whether the server says a prefix is pending.
    server_active: bool,
    /// Whether the popup is visible to the user.
    visible: bool,
    /// When the server last activated (for delay calculation).
    activated_at: Option<Instant>,
    /// How long to wait before showing the popup.
    show_delay: Duration,
    /// Clock source (swappable for deterministic testing).
    clock: Arc<dyn Clock>,
    /// Pending key prefix (e.g., "g").
    prefix: String,
    /// Available continuations: `(key, command)` pairs.
    hints: Vec<(String, String)>,
}

impl WhichKeyExtension {
    /// Create a new which-key extension with the default 500ms delay.
    #[must_use]
    pub fn new() -> Self {
        Self {
            server_active: false,
            visible: false,
            activated_at: None,
            show_delay: DEFAULT_SHOW_DELAY,
            clock: Arc::new(SystemClock),
            prefix: String::new(),
            hints: Vec::new(),
        }
    }

    /// Create a which-key extension with a custom delay.
    #[must_use]
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            show_delay: delay,
            ..Self::new()
        }
    }

    /// Create a which-key extension with a custom clock and delay.
    ///
    /// Used for deterministic testing with `TestClock`.
    #[must_use]
    pub fn with_clock(clock: Arc<dyn Clock>, delay: Duration) -> Self {
        Self {
            show_delay: delay,
            clock,
            ..Self::new()
        }
    }
}

impl Default for WhichKeyExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for WhichKeyExtension {
    fn kind(&self) -> &'static str {
        "whichkey"
    }

    fn is_active(&self) -> bool {
        self.visible
    }

    fn apply_notification(&mut self, data: &str) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            let active = json
                .get("active")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            self.prefix = json
                .get("prefix")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            self.hints = json
                .get("hints")
                .and_then(serde_json::Value::as_array)
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|h| {
                            let key = h.get("key")?.as_str()?;
                            let cmd = h.get("command")?.as_str()?;
                            Some((key.to_string(), cmd.to_string()))
                        })
                        .collect()
                });

            if active {
                // IDLE -> WAITING: start the delay timer
                if !self.server_active {
                    self.activated_at = Some(self.clock.now());
                }
                self.server_active = true;
                // visible stays false until tick() promotes to SHOWING
            } else {
                // -> IDLE: reset everything
                self.server_active = false;
                self.visible = false;
                self.activated_at = None;
            }
        }
    }

    fn tick(&mut self) -> bool {
        if self.server_active
            && !self.visible
            && let Some(activated_at) = self.activated_at
            && self.clock.now().duration_since(activated_at) >= self.show_delay
        {
            self.visible = true;
            return true;
        }
        false
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.hints.is_empty() {
            return;
        }

        let (width, height) = backend.size();

        let hint_count = self.hints.len() as u16;
        // popup height: top border + hints + bottom border
        let popup_height = hint_count + 2;

        let pw = popup_width(width);
        let px = popup_x(width, pw);
        // Position above the statusline (bottom-anchored)
        let py = height.saturating_sub(1 + popup_height);

        let border_style = Style::default().fg(Color::DarkGrey);

        // Draw border
        render_box_border(backend, px, py, pw, popup_height, &border_style);

        // Draw title in the top border: "--- g ---"
        if !self.prefix.is_empty() {
            let title = format!(" {} ", self.prefix);
            let title_x = px + 3; // after "---"
            let title_style = Style::default().fg(Color::Yellow);
            backend.write_str(title_x, py, &title, &title_style);
        }

        // Draw hints
        let content_x = px + 2; // border + padding
        let content_width = pw.saturating_sub(4); // 2 padding each side

        let key_style = Style::default().fg(Color::Cyan);
        let cmd_style = Style::default().fg(Color::White);
        let clear_style = Style::default();

        for (i, (key, command)) in self.hints.iter().enumerate() {
            let row_y = py + 1 + i as u16;

            // Clear the row
            for col in 0..content_width {
                backend.set_cell(content_x + col, row_y, ' ', &clear_style);
            }

            // Key column (left-aligned, max 6 chars)
            let key_display = truncate_end(key, 6);
            backend.write_str(content_x, row_y, &key_display, &key_style);

            // Command column (after key + gap)
            let cmd_x = content_x + 7; // 6 chars for key + 1 space gap
            if cmd_x < px + pw - 2 {
                let max_cmd_width = (px + pw - 2).saturating_sub(cmd_x) as usize;
                let cmd_display = truncate_end(command, max_cmd_width);
                backend.write_str(cmd_x, row_y, &cmd_display, &cmd_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {reovim_arch::clock::TestClock, reovim_driver_display::FrameBuffer};

    use super::*;

    // =========================================================================
    // Helpers
    // =========================================================================

    fn activate_data() -> &'static str {
        r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"},{"key":"d","command":"goto-definition"}]}"#
    }

    fn activate_data_single() -> &'static str {
        r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"}]}"#
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
        assert_eq!(ext.hints[0].0, "g");
        assert_eq!(ext.hints[0].1, "goto-top");
        assert_eq!(ext.hints[1].0, "d");
        assert_eq!(ext.hints[1].1, "goto-definition");

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
        let py = 19;

        assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}');
        assert_eq!(fb.get(px + pw - 1, py).unwrap().char, '\u{256E}');
        assert_eq!(fb.get(px, py + 3).unwrap().char, '\u{2570}');
    }

    #[test]
    fn test_render_no_prefix() {
        let mut ext = zero_delay_ext();
        ext.server_active = true;
        ext.visible = true;
        ext.hints = vec![("a".to_string(), "cmd-a".to_string())];

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
        let py = 24 - 1 - 3;
        let content_x = px + 2;
        let cmd_x = content_x + 7;
        let hint_row_y = py + 1;

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
        assert_eq!(ext.hints[0].1, "goto-top");
    }
}
