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
        "markdown"
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
mod tests {
    use super::*;

    #[test]
    fn test_new_extension() {
        let ext = MarkdownRenderExtension::new();
        assert_eq!(ext.kind(), "markdown");
        assert!(ext.is_active());
        assert!(ext.virtual_lines().is_empty());
    }

    #[test]
    fn test_default_extension() {
        let ext = MarkdownRenderExtension::default();
        assert_eq!(ext.kind(), "markdown");
    }

    #[test]
    fn test_classify_token_delegates() {
        let ext = MarkdownRenderExtension::new();
        assert!(ext.classify_token("markup.heading.1").is_some());
        assert!(ext.classify_token("keyword").is_none());
    }

    #[test]
    fn test_on_buffer_update_detects_tables() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);
        assert!(!ext.virtual_lines().is_empty());
        assert_eq!(ext.virtual_lines().len(), 2); // top + bottom border
    }

    #[test]
    fn test_on_buffer_update_no_tables() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec!["hello".to_string(), "world".to_string()];
        ext.on_buffer_update(1, &lines);
        assert!(ext.virtual_lines().is_empty());
    }

    #[test]
    fn test_virtual_lines_top_bottom_borders() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        let vlines = ext.virtual_lines();
        assert_eq!(vlines.len(), 2);

        // Top border before first table row
        assert_eq!(vlines[0].buffer_line, 0);
        assert_eq!(vlines[0].position, VirtualLinePosition::Before);
        assert!(vlines[0].content.starts_with('┌'));

        // Bottom border after last table row
        assert_eq!(vlines[1].buffer_line, 2);
        assert_eq!(vlines[1].position, VirtualLinePosition::After);
        assert!(vlines[1].content.starts_with('└'));
    }

    #[test]
    fn test_transform_line_header_centered() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        let result = ext.transform_line(1, 0, "| A | B |");
        assert!(result.is_some());
        let transformed = result.unwrap();
        assert!(transformed.text.starts_with('│'));
        assert!(transformed.text.ends_with('│'));
    }

    #[test]
    fn test_transform_line_delimiter_mid_border() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        let result = ext.transform_line(1, 1, "|---|---|");
        assert!(result.is_some());
        let transformed = result.unwrap();
        assert!(transformed.text.starts_with('├'));
        assert!(transformed.text.contains('┼'));
        assert!(transformed.text.ends_with('┤'));
    }

    #[test]
    fn test_transform_line_data_left_aligned() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        let result = ext.transform_line(1, 2, "| 1 | 2 |");
        assert!(result.is_some());
        let transformed = result.unwrap();
        assert!(transformed.text.starts_with('│'));
        assert!(transformed.text.contains(" 1 "));
    }

    #[test]
    fn test_transform_line_outside_table() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "hello".to_string(),
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        assert!(ext.transform_line(1, 0, "hello").is_none());
    }

    #[test]
    fn test_transform_line_insert_mode_bypass() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        // Enable insert mode and set cursor to line 0
        ext.on_mode_change("INSERT", true);
        ext.on_cursor_update(1, 0, 0);

        // Should return None (raw text shown in insert mode)
        assert!(ext.transform_line(1, 0, "| A | B |").is_none());
    }

    #[test]
    fn test_transform_line_normal_mode_cursor() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        // Normal mode on cursor line — still transforms
        ext.on_mode_change("NORMAL", false);
        ext.on_cursor_update(1, 0, 0);
        assert!(ext.transform_line(1, 0, "| A | B |").is_some());
    }

    #[test]
    fn test_transform_line_styles_border_chars() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        let result = ext.transform_line(1, 0, "| A | B |").unwrap();
        // First char is '│' — should have a border style (DarkGrey)
        assert!(result.styles[0].is_some());
        let border_style = result.styles[0].as_ref().unwrap();
        assert_eq!(border_style.fg, Some(Color::DarkGrey));
    }

    #[test]
    fn test_map_cursor_column_outside_table() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec!["hello".to_string()];
        ext.on_buffer_update(1, &lines);
        assert!(ext.map_cursor_column(1, 0, 3).is_none());
    }

    #[test]
    fn test_map_cursor_column_insert_mode_bypass() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);
        ext.on_mode_change("INSERT", true);
        ext.on_cursor_update(1, 0, 0);
        assert!(ext.map_cursor_column(1, 0, 3).is_none());
    }

    #[test]
    fn test_map_cursor_column_with_stored_lines() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        // Pipe at buffer col 0 should map to visual col 0
        let result = ext.map_cursor_column(1, 0, 0);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_map_cursor_column_pipe_maps_to_border() {
        let mut ext = MarkdownRenderExtension::new();
        let lines = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines);

        // Middle pipe at buffer col 4 should map to a visual border position
        let result = ext.map_cursor_column(1, 0, 4);
        assert!(result.is_some());
        // Visual pipe position depends on col_widths — just verify it maps
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn test_on_cursor_update() {
        let mut ext = MarkdownRenderExtension::new();
        ext.on_cursor_update(1, 5, 10);
        assert_eq!(ext.cursor_line, Some(5));
    }

    #[test]
    fn test_on_mode_change() {
        let mut ext = MarkdownRenderExtension::new();
        ext.on_mode_change("INSERT", true);
        assert!(ext.is_insert);
        ext.on_mode_change("NORMAL", false);
        assert!(!ext.is_insert);
    }

    #[test]
    fn test_apply_notification_noop() {
        let mut ext = MarkdownRenderExtension::new();
        ext.apply_notification("anything");
        // Should not change state
        assert!(ext.is_active());
    }

    #[test]
    fn test_multiple_buffers() {
        let mut ext = MarkdownRenderExtension::new();

        let lines1 = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        ext.on_buffer_update(1, &lines1);

        let lines2 = vec!["no tables".to_string()];
        ext.on_buffer_update(2, &lines2);

        // Buffer 1 has tables
        assert!(ext.transform_line(1, 0, "| A | B |").is_some());
        // Buffer 2 has no tables
        assert!(ext.transform_line(2, 0, "no tables").is_none());
    }
}
