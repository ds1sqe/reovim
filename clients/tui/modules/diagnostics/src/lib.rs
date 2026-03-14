//! Diagnostics inline rendering module.
//!
//! Renders LSP diagnostic markers (underlines + virtual text) at buffer
//! positions. Underlines are delivered via `inline_decorations()`, diagnostic
//! messages via `virtual_lines()`.
//!
//! Migrated from `DiagnosticsExtension` (`TuiExtension`) to native
//! `ClientModule` as part of M6 (#637).

use std::collections::HashMap;

use reovim_client_driver::{
    BufferId, ClientModule, ClientModuleError, InlineDecoration, ModuleContext, ProbeResult, Style,
    Version, VirtualLine, VirtualLinePosition,
};

use reovim_arch::Color;
use serde::Deserialize;

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

/// Map severity string to a color.
const fn severity_color(severity: &str) -> Color {
    match severity.as_bytes() {
        b"error" => Color::Red,
        b"warning" => Color::Yellow,
        b"information" => Color::Cyan,
        b"hint" => Color::Green,
        _ => Color::Grey,
    }
}

/// Map severity string to a display prefix.
const fn severity_prefix(severity: &str) -> &str {
    match severity.as_bytes() {
        b"error" => "E",
        b"warning" => "W",
        b"information" => "I",
        b"hint" => "H",
        _ => "?",
    }
}

/// Diagnostics inline rendering module.
///
/// Renders diagnostic underlines at buffer positions and shows
/// diagnostic messages as virtual lines below the affected line.
///
/// Kind: `"diagnostics"` (matches server's diagnostics bridge).
pub struct DiagnosticsModule {
    active: bool,
    buffers: Vec<BufferDiagnostics>,
    active_buffer_id: Option<u64>,
    /// Cached inline decorations grouped by line.
    decorations_by_line: HashMap<usize, Vec<InlineDecoration>>,
    /// Cached virtual lines for diagnostic messages.
    cached_virtual_lines: Vec<VirtualLine>,
}

impl DiagnosticsModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            buffers: Vec::new(),
            active_buffer_id: None,
            decorations_by_line: HashMap::new(),
            cached_virtual_lines: Vec::new(),
        }
    }

    /// Rebuild caches from current state.
    #[allow(clippy::cast_possible_truncation)]
    fn rebuild_caches(&mut self) {
        self.decorations_by_line.clear();
        self.cached_virtual_lines.clear();

        if !self.active {
            return;
        }

        let Some(buf_id) = self.active_buffer_id else {
            return;
        };

        let Some(buffer_diags) = self.buffers.iter().find(|b| b.buffer_id == buf_id) else {
            return;
        };

        for diag in &buffer_diags.items {
            let color = severity_color(&diag.severity);

            // Underline decorations (single-line diagnostics only)
            if diag.start_line == diag.end_line {
                let style = Style::new().fg(color);
                self.decorations_by_line
                    .entry(diag.start_line as usize)
                    .or_default()
                    .push(InlineDecoration {
                        col_start: diag.start_col as u16,
                        col_end: diag.end_col as u16,
                        style,
                    });
            }

            // Virtual line with diagnostic message
            let prefix = severity_prefix(&diag.severity);
            let content = format!(" {prefix}: {}", diag.message);
            let style = Style::new().fg(color);

            self.cached_virtual_lines.push(VirtualLine {
                buffer_line: diag.start_line as usize,
                position: VirtualLinePosition::After,
                content,
                style,
            });
        }
    }
}

impl Default for DiagnosticsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for DiagnosticsModule {
    fn id(&self) -> &'static str {
        "diagnostics"
    }

    fn kind(&self) -> &'static str {
        "diagnostics"
    }

    fn name(&self) -> &'static str {
        "Diagnostics"
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

    fn has_buffer_contrib(&self) -> bool {
        self.active
    }

    fn on_notification(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<DiagnosticPayload>(data) else {
            return;
        };

        self.active = payload.active;

        if self.active {
            self.buffers = payload.diagnostics;
        } else {
            self.buffers.clear();
        }

        self.rebuild_caches();
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.active_buffer_id = Some(buffer_id.0 as u64);
        self.rebuild_caches();
    }

    fn inline_decorations(&self, line: usize) -> &[InlineDecoration] {
        self.decorations_by_line
            .get(&line)
            .map_or(&[], Vec::as_slice)
    }

    fn virtual_lines(&self) -> &[VirtualLine] {
        &self.cached_virtual_lines
    }
}

#[cfg(test)]
mod tests;
