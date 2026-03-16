//! Bufferline chrome module for the TUI client.
//!
//! Renders a horizontal tab bar at `ChromePosition::Top` showing open buffers
//! with active highlight, modified `[+]` markers, pinned indicators, and
//! per-buffer diagnostic counts.

use {
    reovim_client_driver::{
        BufferId, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
        PlatformCapabilities, ProbeResult, Rect, RenderSurface, Style, Version, types::Color,
    },
    serde::Deserialize,
};

/// Separator between tabs.
const TAB_SEP: &str = "\u{2502}";

/// JSON payload from the server bridge.
#[derive(Debug, Deserialize)]
struct Payload {
    active: bool,
    #[serde(default)]
    buffers: Vec<BufferPayload>,
}

/// Per-buffer data from the server.
#[derive(Debug, Deserialize)]
struct BufferPayload {
    id: u64,
    name: String,
    modified: bool,
    pinned: bool,
    #[serde(rename = "errorCount", default)]
    error_count: u32,
    #[serde(rename = "warningCount", default)]
    warning_count: u32,
}

/// Internal tab state.
#[derive(Debug, Clone)]
struct TabEntry {
    id: u64,
    name: String,
    modified: bool,
    pinned: bool,
    error_count: u32,
    warning_count: u32,
}

/// Bufferline chrome module.
///
/// Renders a 1-row tab bar at the top of the editor. Receives buffer list
/// updates from the server via `on_notification()` and tracks active buffer
/// via `on_buffer_focus()`.
pub struct BufferlineModule {
    tabs: Vec<TabEntry>,
    active_buffer_id: Option<u64>,
    scroll_offset: usize,
}

impl BufferlineModule {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_buffer_id: None,
            scroll_offset: 0,
        }
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
        Version::new(0, 1, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec!["bufferline"]
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
        ChromePosition::Top
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        1
    }

    fn chrome_priority(&self) -> u16 {
        90
    }

    fn on_notification(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<Payload>(data) else {
            return;
        };

        if !payload.active {
            self.tabs.clear();
            return;
        }

        self.tabs = payload
            .buffers
            .into_iter()
            .map(|b| TabEntry {
                id: b.id,
                name: b.name,
                modified: b.modified,
                pinned: b.pinned,
                error_count: b.error_count,
                warning_count: b.warning_count,
            })
            .collect();

        self.ensure_active_visible();
    }

    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.active_buffer_id = Some(buffer_id.0 as u64);
        self.ensure_active_visible();
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        let width = bounds.width as usize;
        if width == 0 {
            return;
        }

        // Fill background.
        let bg_style = Style::new().bg(Color::DarkGrey).fg(Color::Grey);
        surface.fill(bounds, ' ', bg_style.clone());

        if self.tabs.is_empty() {
            return;
        }

        // Build tab labels and compute widths.
        let labels: Vec<(String, bool)> = self
            .tabs
            .iter()
            .map(|tab| {
                let is_active = self.active_buffer_id == Some(tab.id);
                let label = format_tab_label(tab);
                (label, is_active)
            })
            .collect();

        // Compute tab widths (label + separator).
        let tab_widths: Vec<usize> = labels.iter().map(|(l, _)| l.len()).collect();
        let total_width: usize =
            tab_widths.iter().sum::<usize>() + tab_widths.len().saturating_sub(1); // separators

        // Determine scroll offset.
        let scroll = if total_width <= width {
            0
        } else {
            self.scroll_offset
        };

        // Render tabs with scroll.
        let mut x = bounds.x;
        let mut consumed = 0usize;

        // Show left overflow indicator.
        if scroll > 0 {
            x += surface.write_styled(x, bounds.y, "<", bg_style.clone());
            consumed += 1;
        }

        let mut skipped = 0usize;
        for (i, (label, is_active)) in labels.iter().enumerate() {
            let tab_start = if i == 0 {
                0
            } else {
                tab_widths[..i].iter().sum::<usize>() + i // include separators
            };

            if tab_start + tab_widths[i] <= scroll {
                skipped = i + 1;
                continue;
            }

            // Separator before tab (except first visible).
            if i > skipped.max(if scroll > 0 { skipped } else { 0 }) {
                if consumed + 1 > width {
                    break;
                }
                let sep_style = Style::new().fg(Color::Grey).bg(Color::DarkGrey);
                x += surface.write_styled(x, bounds.y, TAB_SEP, sep_style);
                consumed += 1;
            }

            let remaining = width.saturating_sub(consumed);
            if remaining == 0 {
                break;
            }

            let style = if *is_active {
                Style::new().bg(Color::Blue).fg(Color::White)
            } else {
                Style::new().bg(Color::DarkGrey).fg(Color::White)
            };

            let display = if label.len() > remaining {
                &label[..remaining]
            } else {
                label.as_str()
            };

            let written = surface.write_styled(x, bounds.y, display, style) as usize;
            x += written as u16;
            consumed += written;

            if consumed >= width {
                break;
            }
        }

        // Show right overflow indicator.
        if total_width > width && consumed < width && scroll + width < total_width {
            surface.write_styled(bounds.x + bounds.width - 1, bounds.y, ">", bg_style);
        }
    }
}

impl BufferlineModule {
    /// Adjust scroll to keep the active buffer tab visible.
    fn ensure_active_visible(&mut self) {
        let Some(active_id) = self.active_buffer_id else {
            return;
        };

        let Some(active_idx) = self.tabs.iter().position(|t| t.id == active_id) else {
            return;
        };

        // Simple: if active tab index is before scroll, scroll back.
        if active_idx < self.scroll_offset {
            self.scroll_offset = active_idx;
        }
        // Note: full pixel-level scroll requires knowing bounds width,
        // which we don't have here. The render loop handles the rest.
    }
}

/// Format a tab label: ` [pin] name [+] E:n W:n `
fn format_tab_label(tab: &TabEntry) -> String {
    let mut label = if tab.pinned {
        format!(" * {} ", tab.name)
    } else {
        format!(" {} ", tab.name)
    };

    if tab.modified {
        label.push_str("[+] ");
    }

    if tab.error_count > 0 {
        use std::fmt::Write;
        let _ = write!(label, "E:{} ", tab.error_count);
    }

    if tab.warning_count > 0 {
        use std::fmt::Write;
        let _ = write!(label, "W:{} ", tab.warning_count);
    }

    label
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
