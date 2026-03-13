#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Markdown rendering extension for the TUI.
//!
//! Owns ALL markdown rendering policy:
//! - Token classification (heading icons, bullet glyphs, code conceals)
//! - Table detection, virtual borders, expanded rows, cursor mapping
//!
//! The render engine knows HOW to conceal/highlight/background (mechanism).
//! This extension decides WHICH categories trigger which behavior (policy).

mod behaviors;
mod detect;
mod layout;
mod mapping;

use {
    detect::{TableRegion, detect_tables},
    layout::{build_expanded_row, generate_border},
    mapping::build_column_mapping,
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        render_backend::{
            RenderBackend, RenderBehavior, TransformedLine, TuiExtension, VirtualLine,
            VirtualLinePosition,
        },
    },
    std::collections::HashMap,
};

/// Markdown rendering extension.
///
/// Handles all markdown-specific rendering policy:
/// - `classify_token()`: heading icons, bullet glyphs, horizontal rules, code conceals
/// - `on_buffer_update()`: table detection
/// - `virtual_lines()`: table top/bottom borders
/// - `transform_line()`: expanded table rows with box-drawing
/// - `map_cursor_column()`: cursor positioning inside table rows
const KIND: &str = "markdown";

pub struct MarkdownRenderExtension {
    /// Detected tables per buffer.
    tables: HashMap<u64, Vec<TableRegion>>,
    /// Stored buffer lines per buffer (needed for cursor column mapping).
    buffer_lines: HashMap<u64, Vec<String>>,
    /// Active buffer ID.
    active_buffer_id: Option<u64>,
    /// Cached virtual lines for the active buffer.
    cached_virtual_lines: Vec<VirtualLine>,
    /// Current cursor line (for insert-mode bypass).
    cursor_line: Option<usize>,
    /// Whether in insert mode.
    is_insert: bool,
}

impl MarkdownRenderExtension {
    /// Create a new markdown render extension.
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
    fn rebuild_virtual_lines(&mut self, buffer_id: u64) {
        self.cached_virtual_lines.clear();
        let Some(tables) = self.tables.get(&buffer_id) else {
            return;
        };
        let border_style = Style::default().fg(Color::DarkGrey);
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

    /// Find the table containing a given line in the active buffer.
    fn table_at_line(&self, buffer_id: u64, line_idx: usize) -> Option<&TableRegion> {
        self.tables.get(&buffer_id).and_then(|tables| {
            tables
                .iter()
                .find(|t| line_idx >= t.start_line && line_idx <= t.end_line)
        })
    }
}

impl Default for MarkdownRenderExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for MarkdownRenderExtension {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn is_active(&self) -> bool {
        true // Lightweight — no-op when no tables detected
    }

    fn apply_notification(&mut self, _data: &str) {
        // No server notifications needed — purely client-side policy
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // All rendering handled through generic hooks
    }

    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        behaviors::classify_markdown_token(category)
    }

    fn on_buffer_update(&mut self, buffer_id: u64, lines: &[String]) {
        let detected = detect_tables(lines);
        self.tables.insert(buffer_id, detected);
        self.buffer_lines.insert(buffer_id, lines.to_vec());
        self.active_buffer_id = Some(buffer_id);
        self.rebuild_virtual_lines(buffer_id);
    }

    fn virtual_lines(&self) -> &[VirtualLine] {
        &self.cached_virtual_lines
    }

    fn transform_line(
        &self,
        buffer_id: u64,
        line_idx: usize,
        line: &str,
    ) -> Option<TransformedLine> {
        let table = self.table_at_line(buffer_id, line_idx)?;

        // Insert-mode bypass: show raw text on the cursor line
        if self.is_insert && self.cursor_line == Some(line_idx) {
            return None;
        }

        let expanded = if line_idx == table.delimiter_line {
            generate_border(&table.col_widths, '├', '┼', '┤')
        } else if line_idx <= table.delimiter_line {
            build_expanded_row(line, &table.col_widths, true) // header
        } else {
            build_expanded_row(line, &table.col_widths, false) // data
        };

        let border_style = Some(Style::default().fg(Color::DarkGrey));
        let styles: Vec<Option<Style>> = expanded
            .chars()
            .map(|ch| {
                if "│├┤┼┌┐└┘┬┴─".contains(ch) {
                    border_style.clone()
                } else {
                    None // Default style for content
                }
            })
            .collect();

        Some(TransformedLine {
            text: expanded,
            styles,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn map_cursor_column(&self, buffer_id: u64, line_idx: usize, buffer_col: usize) -> Option<u16> {
        let table = self.table_at_line(buffer_id, line_idx)?;

        // In insert mode on cursor line, we show raw text (no mapping)
        if self.is_insert && self.cursor_line == Some(line_idx) {
            return None;
        }

        let original_line = self
            .buffer_lines
            .get(&buffer_id)
            .and_then(|lines| lines.get(line_idx))?;

        let mapping = build_column_mapping(original_line, &table.col_widths);
        mapping.get(buffer_col).copied()
    }

    fn on_cursor_update(&mut self, _buffer_id: u64, line: usize, _col: usize) {
        self.cursor_line = Some(line);
    }

    fn on_mode_change(&mut self, _mode_name: &str, is_insert: bool) {
        self.is_insert = is_insert;
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
