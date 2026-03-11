//! Diagnostics inline TUI extension.
//!
//! Renders LSP diagnostic markers (underlines + virtual text) at buffer
//! positions. Uses `ViewportContext::buffer_id` to filter diagnostics
//! for the focused buffer only.
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        render_backend::{RenderBackend, TuiExtension, ViewportContext},
    },
    serde::Deserialize,
};

/// Deserialized diagnostic notification payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticPayload {
    active: bool,
    #[serde(default)]
    diagnostics: Vec<BufferDiagnostics>,
}

/// Diagnostics for a single buffer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferDiagnostics {
    buffer_id: u64,
    items: Vec<DiagnosticItemPayload>,
}

/// A single diagnostic item from the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticItemPayload {
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    severity: String,
    message: String,
    #[serde(default)]
    #[allow(dead_code)]
    source: Option<String>,
}

/// Inline diagnostics TUI extension.
///
/// Renders diagnostic underlines at buffer positions and shows the first
/// error/warning message as virtual text at the end of the line.
pub struct DiagnosticsExtension {
    /// Whether diagnostics are active.
    active: bool,
    /// Diagnostics grouped by buffer.
    buffers: Vec<BufferDiagnostics>,
}

impl DiagnosticsExtension {
    /// Create a new diagnostics extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            buffers: Vec::new(),
        }
    }
}

impl Default for DiagnosticsExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// Map severity string to a color.
fn severity_color(severity: &str) -> Color {
    match severity {
        "error" => Color::Red,
        "warning" => Color::Yellow,
        "information" => Color::Cyan,
        "hint" => Color::Green,
        _ => Color::Grey,
    }
}

/// Map severity string to a display prefix.
fn severity_prefix(severity: &str) -> &'static str {
    match severity {
        "error" => "E",
        "warning" => "W",
        "information" => "I",
        "hint" => "H",
        _ => "?",
    }
}

impl TuiExtension for DiagnosticsExtension {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::DIAGNOSTICS
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<DiagnosticPayload>(data) else {
            return;
        };

        self.active = payload.active;

        if self.active {
            self.buffers = payload.diagnostics;
        } else {
            self.buffers.clear();
        }
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // Diagnostics require viewport context for buffer-position mapping.
        // The default render() is a no-op; rendering happens in render_with_viewport().
    }

    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, viewport: &ViewportContext) {
        let Some(focused_buffer_id) = viewport.buffer_id else {
            return;
        };

        // Find diagnostics for the focused buffer.
        let Some(buffer_diags) = self
            .buffers
            .iter()
            .find(|b| b.buffer_id == focused_buffer_id)
        else {
            return;
        };

        let (width, _height) = backend.size();
        if width == 0 {
            return;
        }

        // Clamp scroll_top to u32 range (files beyond 4B lines are truncated).
        #[allow(clippy::cast_possible_truncation)]
        let scroll_top = viewport.scroll_top.min(u32::MAX as usize) as u32;
        let visible_rows = u32::from(viewport.content_height);

        // Minimum column for virtual text (content area + gap).
        let virt_min_x = viewport.content_x.saturating_add(40);

        for diag in &buffer_diags.items {
            // Skip diagnostics outside the visible viewport.
            if diag.start_line < scroll_top
                || diag.start_line >= scroll_top.saturating_add(visible_rows)
            {
                continue;
            }

            #[allow(clippy::cast_possible_truncation)]
            let screen_row = (diag.start_line - scroll_top) as u16;
            let color = severity_color(&diag.severity);
            let underline_style = Style::default().fg(color);

            // Draw underline markers on the diagnostic range.
            // Multi-line diagnostics only underline on start_line == end_line;
            // multi-line spans show virtual text but no underline (by design).
            if diag.start_line == diag.end_line {
                #[allow(clippy::cast_possible_truncation)]
                let start_x = viewport.content_x.saturating_add(diag.start_col as u16);
                #[allow(clippy::cast_possible_truncation)]
                let end_x = viewport.content_x.saturating_add(diag.end_col as u16);
                for x in start_x..end_x.min(width) {
                    backend.apply_style(x, screen_row, &underline_style);
                }
            }

            // Draw virtual text (severity prefix + truncated message) at end of line.
            let prefix = severity_prefix(&diag.severity);
            let virt_text = format!(" {prefix}: {}", diag.message);
            let virt_style = Style::default().fg(color);

            // Position virtual text right-aligned, but not before virt_min_x.
            #[allow(clippy::cast_possible_truncation)]
            let virt_x = width
                .saturating_sub(virt_text.len().min(u16::MAX as usize) as u16)
                .max(virt_min_x);
            if virt_x < width {
                let max_chars = (width - virt_x) as usize;
                let display: String = virt_text.chars().take(max_chars).collect();
                backend.write_str(virt_x, screen_row, &display, &virt_style);
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
