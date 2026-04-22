#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, allow(unused_features))]
//! Which-key popup chrome module.
//!
//! Displays a popup showing available key continuations when a prefix
//! key is held (e.g., `g` shows `gg`, `gd`, etc.).
//!
//! The popup appears after a configurable delay (default 500ms).
//! If the user completes the key sequence before the delay expires,
//! no popup is shown at all.
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reovim_client_driver::{
    ChromePosition, ChromeSurface, ClientModule, ClientModuleError, ModuleContext,
    PlatformCapabilities, ProbeResult, Rect, Style, Version, types::Color,
};

// Re-export the Clock trait from arch for constructor usage.
pub use reovim_client_driver::reovim_arch::clock::{Clock, SystemClock};

/// Default show-delay before the which-key popup appears.
const DEFAULT_SHOW_DELAY: Duration = Duration::from_millis(500);

/// Default category display order. Unlisted categories sort after these.
const CATEGORY_ORDER: &[&str] = &["motion", "operator", "textobject", "window", "buffer"];

const KIND: &str = "whichkey";

/// A single hint entry with key, command, and category.
struct WhichKeyHint {
    key: String,
    command: String,
    category: String,
}

/// Group hints by category and return ordered groups.
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
        (idx.is_none(), idx.unwrap_or(0), *cat)
    });
    result
}

/// Title-case a category name (e.g., "motion" -> "Motion").
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().to_string() + chars.as_str())
}

/// Color configuration for the which-key popup.
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
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
fn render_hint_row(
    surface: &mut dyn ChromeSurface,
    hint: &WhichKeyHint,
    py: u16,
    row_offset: u16,
    content_x: u16,
    content_width: u16,
    px: u16,
    pw: u16,
    key_style: &Style,
    cmd_style: &Style,
) {
    let row_y = py + 1 + row_offset;
    surface.fill(
        Rect {
            x: content_x,
            y: row_y,
            width: content_width,
            height: 1,
        },
        ' ',
        Style::new(),
    );

    let key_display = reovim_client_subsys_chrome::ui::truncate_end(&hint.key, 6);
    surface.write_styled(content_x, row_y, &key_display, key_style.clone());

    let cmd_x = content_x + 7;
    if cmd_x < px + pw - 2 {
        #[allow(clippy::cast_possible_truncation)]
        let max_cmd_width = (px + pw - 2).saturating_sub(cmd_x) as usize;
        let cmd_display =
            reovim_client_subsys_chrome::ui::truncate_end(&hint.command, max_cmd_width);
        surface.write_styled(cmd_x, row_y, &cmd_display, cmd_style.clone());
    }
}

/// Which-key popup chrome module with configurable show-delay.
pub struct WhichKeyModule {
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

impl WhichKeyModule {
    /// Create a new which-key module with the default 500ms delay.
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

    /// Create a which-key module with custom color styling.
    #[must_use]
    pub fn with_style(style: WhichKeyStyleConfig) -> Self {
        Self {
            style,
            ..Self::new()
        }
    }

    /// Create a which-key module with a custom delay.
    #[must_use]
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            show_delay: delay,
            ..Self::new()
        }
    }

    /// Create a which-key module with a custom clock and delay.
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

impl Default for WhichKeyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for WhichKeyModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "WhichKey"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        50
    }

    fn on_notification(&mut self, data: &str) {
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
            } else {
                // -> IDLE: reset everything
                self.server_active = false;
                self.visible = false;
                self.activated_at = None;
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.visible || self.hints.is_empty() {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);

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

        let pw = reovim_client_subsys_chrome::chrome_utils::popup_width(width);
        let px = reovim_client_subsys_chrome::chrome_utils::popup_x(width, pw);
        // Position above the statusline (bottom-anchored)
        let py = height.saturating_sub(1 + popup_height);

        let border_style = Style::new().fg(self.style.border_color);

        // Draw border
        reovim_client_subsys_chrome::chrome_utils::render_box_border(
            surface,
            px,
            py,
            pw,
            popup_height,
            &border_style,
        );

        // Draw title in the top border: "--- g ---"
        if !self.prefix.is_empty() {
            let title = format!(" {} ", self.prefix);
            let title_x = px + 3;
            let title_style = Style::new().fg(self.style.title_color);
            surface.write_styled(title_x, py, &title, title_style);
        }

        // Draw hints
        let content_x = px + 2;
        let content_width = pw.saturating_sub(4);

        let key_style = Style::new().fg(self.style.key_color);
        let cmd_style = Style::new().fg(self.style.desc_color);
        let cat_style = Style::new().fg(self.style.category_color);

        let mut row_offset: u16 = 0;

        if has_categories {
            for (cat, group_hints) in &groups {
                // Draw category header
                let header_y = py + 1 + row_offset;
                surface.fill(
                    Rect {
                        x: content_x,
                        y: header_y,
                        width: content_width,
                        height: 1,
                    },
                    ' ',
                    Style::new(),
                );
                let header_text = title_case(cat);
                surface.write_styled(content_x, header_y, &header_text, cat_style.clone());
                row_offset += 1;

                for hint in group_hints {
                    render_hint_row(
                        surface,
                        hint,
                        py,
                        row_offset,
                        content_x,
                        content_width,
                        px,
                        pw,
                        &key_style,
                        &cmd_style,
                    );
                    row_offset += 1;
                }
            }
        } else {
            for hint in &self.hints {
                render_hint_row(
                    surface,
                    hint,
                    py,
                    row_offset,
                    content_x,
                    content_width,
                    px,
                    pw,
                    &key_style,
                    &cmd_style,
                );
                row_offset += 1;
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
