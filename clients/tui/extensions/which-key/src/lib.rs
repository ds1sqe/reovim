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

/// Default category display order. Unlisted categories sort after these.
const CATEGORY_ORDER: &[&str] = &["motion", "operator", "textobject", "window", "buffer"];

/// A single hint entry with key, command, and category.
struct WhichKeyHint {
    key: String,
    command: String,
    category: String,
}

/// Group hints by category and return ordered groups.
///
/// Groups are ordered by [`CATEGORY_ORDER`] index, with unlisted
/// categories sorted alphabetically after known ones. Hints with
/// an empty category go into a final "other" group.
fn grouped_hints(hints: &[WhichKeyHint]) -> Vec<(&str, Vec<&WhichKeyHint>)> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<&str, Vec<&WhichKeyHint>> = BTreeMap::new();
    for hint in hints {
        let cat = if hint.category.is_empty() {
            "other"
        } else {
            hint.category.as_str()
        };
        groups.entry(cat).or_default().push(hint);
    }

    let mut result: Vec<(&str, Vec<&WhichKeyHint>)> = groups.into_iter().collect();
    result.sort_by_key(|(cat, _)| {
        let idx = CATEGORY_ORDER.iter().position(|c| c == cat);
        // Known categories get their index; unknown get a high value + alphabetical
        (idx.is_none(), idx.unwrap_or(0), *cat)
    });
    result
}

/// Title-case a category name (e.g., "motion" → "Motion").
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().to_string() + chars.as_str())
}

/// Color configuration for the which-key popup.
///
/// Controls the foreground colors used for each element of the popup.
/// Defaults match the original hardcoded values for backward compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyStyleConfig {
    /// Border and box-drawing character color.
    pub border_color: Color,
    /// Title/prefix text color (shown in top border).
    pub title_color: Color,
    /// Key binding text color (left column).
    pub key_color: Color,
    /// Command description text color (right column).
    pub desc_color: Color,
    /// Category header text color.
    pub category_color: Color,
}

impl Default for WhichKeyStyleConfig {
    fn default() -> Self {
        Self {
            border_color: Color::DarkGrey,
            title_color: Color::Yellow,
            key_color: Color::Cyan,
            desc_color: Color::White,
            category_color: Color::DarkGrey,
        }
    }
}

/// Render a single hint row. Returns the number of rows consumed (always 1).
#[allow(clippy::too_many_arguments)]
fn render_hint_row(
    backend: &mut dyn RenderBackend,
    hint: &WhichKeyHint,
    py: u16,
    row_offset: u16,
    content_x: u16,
    content_width: u16,
    px: u16,
    pw: u16,
    key_style: &Style,
    cmd_style: &Style,
    clear_style: &Style,
) -> u16 {
    let row_y = py + 1 + row_offset;
    for col in 0..content_width {
        backend.set_cell(content_x + col, row_y, ' ', clear_style);
    }

    let key_display = truncate_end(&hint.key, 6);
    backend.write_str(content_x, row_y, &key_display, key_style);

    let cmd_x = content_x + 7;
    if cmd_x < px + pw - 2 {
        let max_cmd_width = (px + pw - 2).saturating_sub(cmd_x) as usize;
        let cmd_display = truncate_end(&hint.command, max_cmd_width);
        backend.write_str(cmd_x, row_y, &cmd_display, cmd_style);
    }
    1
}

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
    /// Available continuations with category metadata.
    hints: Vec<WhichKeyHint>,
    /// Color styling configuration for popup elements.
    style: WhichKeyStyleConfig,
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
            style: WhichKeyStyleConfig::default(),
        }
    }

    /// Create a which-key extension with custom color styling.
    #[must_use]
    pub fn with_style(style: WhichKeyStyleConfig) -> Self {
        Self {
            style,
            ..Self::new()
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
                            let cat = h
                                .get("category")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("");
                            Some(WhichKeyHint {
                                key: key.to_string(),
                                command: cmd.to_string(),
                                category: cat.to_string(),
                            })
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

        let groups = grouped_hints(&self.hints);
        let has_categories = groups.len() > 1 || (groups.len() == 1 && groups[0].0 != "other");

        // popup height: borders + hints + category header rows
        let header_rows = if has_categories {
            groups.len() as u16
        } else {
            0
        };
        let hint_count = self.hints.len() as u16;
        let popup_height = hint_count + header_rows + 2;

        let pw = popup_width(width);
        let px = popup_x(width, pw);
        // Position above the statusline (bottom-anchored)
        let py = height.saturating_sub(1 + popup_height);

        let border_style = Style::default().fg(self.style.border_color);

        // Draw border
        render_box_border(backend, px, py, pw, popup_height, &border_style);

        // Draw title in the top border: "--- g ---"
        if !self.prefix.is_empty() {
            let title = format!(" {} ", self.prefix);
            let title_x = px + 3; // after "---"
            let title_style = Style::default().fg(self.style.title_color);
            backend.write_str(title_x, py, &title, &title_style);
        }

        // Draw hints
        let content_x = px + 2; // border + padding
        let content_width = pw.saturating_sub(4); // 2 padding each side

        let key_style = Style::default().fg(self.style.key_color);
        let cmd_style = Style::default().fg(self.style.desc_color);
        let cat_style = Style::default().fg(self.style.category_color);
        let clear_style = Style::default();

        let mut row_offset: u16 = 0;

        if has_categories {
            for (cat, group_hints) in &groups {
                // Draw category header
                let header_y = py + 1 + row_offset;
                for col in 0..content_width {
                    backend.set_cell(content_x + col, header_y, ' ', &clear_style);
                }
                let header_text = title_case(cat);
                backend.write_str(content_x, header_y, &header_text, &cat_style);
                row_offset += 1;

                for hint in group_hints {
                    row_offset += render_hint_row(
                        backend,
                        hint,
                        py,
                        row_offset,
                        content_x,
                        content_width,
                        px,
                        pw,
                        &key_style,
                        &cmd_style,
                        &clear_style,
                    );
                }
            }
        } else {
            for hint in &self.hints {
                row_offset += render_hint_row(
                    backend,
                    hint,
                    py,
                    row_offset,
                    content_x,
                    content_width,
                    px,
                    pw,
                    &key_style,
                    &cmd_style,
                    &clear_style,
                );
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
}
