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
#[path = "lib_tests.rs"]
mod tests;
