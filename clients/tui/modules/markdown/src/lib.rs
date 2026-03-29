#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Markdown rendering module for the TUI.
//!
//! Owns ALL markdown rendering policy:
//! - Token classification (heading icons, bullet glyphs, code conceals)
//! - Table detection, virtual borders, expanded rows, cursor mapping
//!
//! The viewport renderer knows HOW to conceal/highlight/background (mechanism).
//! This module decides WHICH categories trigger which behavior (policy).
//!
//! Migrated from `MarkdownRenderExtension` (`TuiExtension`) to native
//! `ClientModule` as part of M6 (#637).

mod behaviors;
mod detect;
mod layout;
mod mapping;

use {
    detect::{TableRegion, detect_tables},
    layout::{build_expanded_row, generate_border},
    mapping::build_column_mapping,
    reovim_arch::Color,
    reovim_client_driver::{
        BufferId, BufferUpdateEvent, ClientModule, ClientModuleError, ModuleContext, ProbeResult,
        RenderBehavior, Style, TransformedLine, Version, VirtualLine, VirtualLinePosition,
    },
    std::collections::HashMap,
};

/// Markdown rendering module.
///
/// Handles all markdown-specific rendering policy:
/// - `classify_token()`: heading icons, bullet glyphs, horizontal rules, code conceals
/// - `on_buffer_update()`: table detection
/// - `virtual_lines()`: table top/bottom borders
/// - `transform_line()`: expanded table rows with box-drawing
/// - `map_cursor_column()`: cursor positioning inside table rows
pub struct MarkdownModule {
    /// Detected tables per buffer.
    tables: HashMap<usize, Vec<TableRegion>>,
    /// Stored buffer lines per buffer (needed for cursor column mapping).
    buffer_lines: HashMap<usize, Vec<String>>,
    /// Active buffer ID.
    active_buffer_id: Option<usize>,
    /// Cached virtual lines for the active buffer.
    cached_virtual_lines: Vec<VirtualLine>,
    /// Current cursor line (for insert-mode bypass).
    cursor_line: Option<usize>,
    /// Whether in insert mode.
    is_insert: bool,
}

impl MarkdownModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            buffer_lines: HashMap::new(),
            active_buffer_id: None,
            cached_virtual_lines: Vec::new(),
            cursor_line: None,
            is_insert: false,
        }
    }

    /// Rebuild cached virtual lines from detected tables.
    fn rebuild_virtual_lines(&mut self, buffer_id: usize) {
        self.cached_virtual_lines.clear();
        let Some(tables) = self.tables.get(&buffer_id) else {
            return;
        };
        let border_style = Style::new().fg(Color::DarkGrey);
        for table in tables {
            // Top border before the first table row
            self.cached_virtual_lines.push(VirtualLine {
                buffer_line: table.start_line,
                position: VirtualLinePosition::Before,
                content: table.top_border.clone(),
                style: border_style.clone(),
            });
            // Bottom border after the last table row
            self.cached_virtual_lines.push(VirtualLine {
                buffer_line: table.end_line,
                position: VirtualLinePosition::After,
                content: table.bottom_border.clone(),
                style: border_style.clone(),
            });
        }
    }

    /// Find the table containing a given line in the specified buffer.
    fn table_at_line(&self, buffer_id: usize, line_idx: usize) -> Option<&TableRegion> {
        self.tables.get(&buffer_id).and_then(|tables| {
            tables
                .iter()
                .find(|t| line_idx >= t.start_line && line_idx <= t.end_line)
        })
    }
}

impl Default for MarkdownModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for MarkdownModule {
    fn id(&self) -> &'static str {
        "markdown"
    }

    fn kind(&self) -> &'static str {
        "markdown"
    }

    fn name(&self) -> &'static str {
        "Markdown"
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
        true // Lightweight -- no-op when no tables detected
    }

    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        behaviors::classify_markdown_token(category)
    }

    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        let buffer_id = event.buffer_id.0;
        let detected = detect_tables(&event.new_lines);
        self.tables.insert(buffer_id, detected);
        self.buffer_lines.insert(buffer_id, event.new_lines.clone());
        self.active_buffer_id = Some(buffer_id);
        self.rebuild_virtual_lines(buffer_id);
    }

    fn virtual_lines(&self) -> &[VirtualLine] {
        &self.cached_virtual_lines
    }

    fn transform_line(
        &self,
        buf: BufferId,
        line_idx: usize,
        line: &str,
    ) -> Option<TransformedLine> {
        let table = self.table_at_line(buf.0, line_idx)?;

        // Insert-mode bypass: show raw text on the cursor line
        if self.is_insert && self.cursor_line == Some(line_idx) {
            return None;
        }

        let expanded = if line_idx == table.delimiter_line {
            generate_border(&table.col_widths, '\u{251C}', '\u{253C}', '\u{2524}')
        } else if line_idx <= table.delimiter_line {
            build_expanded_row(line, &table.col_widths, true) // header
        } else {
            build_expanded_row(line, &table.col_widths, false) // data
        };

        // Build segments: group consecutive chars by whether they are border chars
        let border_style = Some(Style::new().fg(Color::DarkGrey));
        let mut segments: Vec<(String, Option<Style>)> = Vec::new();
        let mut current_text = String::new();
        let mut current_is_border = false;

        for ch in expanded.chars() {
            let is_border = "\u{2502}\u{251C}\u{2524}\u{253C}\u{250C}\u{2510}\u{2514}\u{2518}\u{252C}\u{2534}\u{2500}".contains(ch);
            if is_border != current_is_border && !current_text.is_empty() {
                let style = if current_is_border {
                    border_style.clone()
                } else {
                    None
                };
                segments.push((std::mem::take(&mut current_text), style));
            }
            current_text.push(ch);
            current_is_border = is_border;
        }
        if !current_text.is_empty() {
            let style = if current_is_border {
                border_style
            } else {
                None
            };
            segments.push((current_text, style));
        }

        Some(TransformedLine { segments })
    }

    fn map_cursor_column(&self, buf: BufferId, line_idx: usize, buffer_col: usize) -> Option<u16> {
        let table = self.table_at_line(buf.0, line_idx)?;

        // In insert mode on cursor line, we show raw text (no mapping)
        if self.is_insert && self.cursor_line == Some(line_idx) {
            return None;
        }

        let original_line = self
            .buffer_lines
            .get(&buf.0)
            .and_then(|lines| lines.get(line_idx))?;

        let mapping = build_column_mapping(original_line, &table.col_widths);
        mapping.get(buffer_col).copied()
    }

    fn on_cursor_update(&mut self, _buffer_id: BufferId, line: usize, _col: usize) {
        self.cursor_line = Some(line);
    }

    fn on_mode_change(&mut self, mode: &str) {
        self.is_insert = mode.eq_ignore_ascii_case("insert");
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
