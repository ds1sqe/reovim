//! Bufferline chrome module for the TUI client.
//!
//! Renders buffer tabs as a top-right overlay that floats over content
//! without stealing a full row. Only visible when 2+ buffers are open.
//!
//! Data flows from two sources:
//! - **Buffer list**: TUI broadcasts `{"type":"buffer_list","buffers":[...]}`
//!   to all modules when buffers change.
//! - **Pin state**: Server bridge sends `{"type":"pin_state","pins":[...]}`
//!   when pin list changes.

use {
    reovim_client_driver::{
        BufferId, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
        PlatformCapabilities, ProbeResult, Rect, RenderSurface, Style, ThemeProvider, Version,
        types::Color,
    },
    serde::Deserialize,
};

/// Separator between tabs.
const TAB_SEP: &str = "\u{2502}";

// Inline icon constants (no dependency on reovim-driver-display).
const PIN_ICON: &str = "\u{f0403}";
const MODIFIED_ICON: &str = "\u{25cf}";

// ============================================================================
// Notification payloads
// ============================================================================

/// Typed notification envelope.
#[derive(Debug, Deserialize)]
struct TypedPayload {
    #[serde(rename = "type")]
    payload_type: String,
}

/// Buffer list payload from `dispatch_buffer_list()`.
#[derive(Debug, Deserialize)]
struct BufferListPayload {
    #[serde(default)]
    buffers: Vec<BufferEntry>,
    #[serde(default = "one")]
    window_count: usize,
}

const fn one() -> usize {
    1
}

/// Per-buffer data from the TUI dispatch.
#[derive(Debug, Deserialize)]
struct BufferEntry {
    id: u64,
    name: String,
    modified: bool,
}

/// Pin state payload from the server bridge.
#[derive(Debug, Deserialize)]
struct PinStatePayload {
    #[serde(default)]
    pins: Vec<u64>,
}

// ============================================================================
// Internal state
// ============================================================================

/// Internal tab state.
#[derive(Debug, Clone)]
struct TabEntry {
    id: u64,
    name: String,
    modified: bool,
    pinned: bool,
}

/// Bufferline chrome module.
///
/// Renders buffer tabs as a top-right overlay. Only visible when 2+ buffers
/// are attached to the client's windows and only a single window is visible
/// (splits already identify which file is in each pane).
pub struct BufferlineModule {
    tabs: Vec<TabEntry>,
    active_buffer_id: Option<u64>,
    pinned_ids: Vec<u64>,
    window_count: usize,
    bg_style: Style,
    active_style: Style,
}

impl BufferlineModule {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_buffer_id: None,
            pinned_ids: Vec::new(),
            window_count: 1,
            bg_style: Style::new().bg(Color::DarkGrey).fg(Color::White),
            active_style: Style::new().bg(Color::Blue).fg(Color::White),
        }
    }

    fn cache_theme(&mut self, theme: &dyn ThemeProvider) {
        let background = theme.highlight("statusline_bg");
        let foreground = theme.highlight("statusline_fg");
        self.bg_style = Style::new()
            .bg(background.bg.unwrap_or(Color::DarkGrey))
            .fg(foreground.fg.unwrap_or(Color::White));

        let mode_normal = theme.highlight("mode_normal");
        self.active_style = Style::new()
            .bg(mode_normal.bg.unwrap_or(Color::Blue))
            .fg(mode_normal.fg.unwrap_or(Color::White));
    }

    /// Handle `{"type":"buffer_list","buffers":[...]}` from TUI dispatch.
    fn handle_buffer_list(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<BufferListPayload>(data) else {
            return;
        };

        self.window_count = payload.window_count;
        self.tabs = payload
            .buffers
            .into_iter()
            .map(|b| {
                let pinned = self.pinned_ids.contains(&b.id);
                TabEntry {
                    id: b.id,
                    name: b.name,
                    modified: b.modified,
                    pinned,
                }
            })
            .collect();

        self.sort_tabs();
    }

    /// Handle `{"type":"pin_state","pins":[...]}` from server bridge.
    fn handle_pin_state(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<PinStatePayload>(data) else {
            return;
        };

        self.pinned_ids = payload.pins;

        for tab in &mut self.tabs {
            tab.pinned = self.pinned_ids.contains(&tab.id);
        }

        self.sort_tabs();
    }

    /// Sort tabs: pinned first (in pin order), then unpinned (by ID).
    fn sort_tabs(&mut self) {
        self.tabs.sort_by(|a, b| match (a.pinned, b.pinned) {
            (true, true) => {
                let pos_a = self.pinned_ids.iter().position(|&id| id == a.id);
                let pos_b = self.pinned_ids.iter().position(|&id| id == b.id);
                pos_a.cmp(&pos_b)
            }
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => a.id.cmp(&b.id),
        });
    }

    /// Build the concatenated tab string for rendering.
    /// Returns `(full_string, active_ranges)` where each range marks
    /// the byte offsets of the active tab within the string.
    fn build_tab_string(&self) -> (String, Option<(usize, usize)>) {
        let mut result = String::new();
        let mut active_range = None;

        for (i, tab) in self.tabs.iter().enumerate() {
            if i > 0 {
                result.push_str(TAB_SEP);
            }

            let start = result.len();
            let label = format_tab_label(tab);
            result.push_str(&label);

            if self.active_buffer_id == Some(tab.id) {
                active_range = Some((start, result.len()));
            }
        }

        (result, active_range)
    }
}

impl Default for BufferlineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for BufferlineModule {
    fn id(&self) -> &'static str {
        "bufferline"
    }

    fn kind(&self) -> &'static str {
        "bufferline"
    }

    fn name(&self) -> &'static str {
        "Bufferline"
    }

    fn version(&self) -> Version {
        Version::new(0, 2, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec!["bufferline"]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        self.cache_theme(ctx.theme);
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

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        0 // Overlay — does not reserve space
    }

    fn chrome_priority(&self) -> u16 {
        90
    }

    fn on_notification(&mut self, data: &str) {
        let Ok(envelope) = serde_json::from_str::<TypedPayload>(data) else {
            return;
        };

        match envelope.payload_type.as_str() {
            "buffer_list" => self.handle_buffer_list(data),
            "pin_state" => self.handle_pin_state(data),
            _ => {}
        }
    }

    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.active_buffer_id = Some(buffer_id.0 as u64);
    }

    fn on_theme_changed(&mut self, theme: &dyn ThemeProvider) {
        self.cache_theme(theme);
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        // Only show when 2+ tabs in single-window mode.
        // Splits already identify which file is in each pane.
        if self.tabs.len() < 2 || bounds.width == 0 || self.window_count > 1 {
            return;
        }

        let screen_width = bounds.width as usize;
        let (tab_str, active_range) = self.build_tab_string();

        // Truncate from the left if too wide — keep right portion visible.
        let display_str = if tab_str.len() > screen_width {
            &tab_str[tab_str.len() - screen_width..]
        } else {
            &tab_str
        };
        let display_len = display_str.len();

        // Right-align: start x position.
        let start_x = bounds.x + bounds.width - display_len as u16;

        // Compute where in the original string the display starts.
        let display_offset = tab_str.len() - display_len;

        // Render the whole string in background style first, then
        // overwrite the active range with active style.
        surface.write_styled(start_x, bounds.y, display_str, self.bg_style.clone());

        if let Some((a_start, a_end)) = active_range {
            // Adjust range to display coordinates.
            let vis_start = a_start.saturating_sub(display_offset);
            let vis_end = a_end.saturating_sub(display_offset).min(display_len);
            if vis_start < vis_end {
                let active_text = &display_str[vis_start..vis_end];
                let ax = start_x + vis_start as u16;
                surface.write_styled(ax, bounds.y, active_text, self.active_style.clone());
            }
        }
    }
}

/// Format a tab label: ` [pin] name [modified] `
fn format_tab_label(tab: &TabEntry) -> String {
    let mut label = if tab.pinned {
        format!(" {PIN_ICON} {} ", tab.name)
    } else {
        format!(" {} ", tab.name)
    };

    if tab.modified {
        label.push_str(MODIFIED_ICON);
        label.push(' ');
    }

    label
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
