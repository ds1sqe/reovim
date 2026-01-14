//! Markdown render stage for decorations (conceals, backgrounds)

use reovim_core::{
    component::RenderContext,
    decoration::DecorationStore,
    highlight::{Color, Style},
    render::{
        Decoration, DecorationKind, RenderData, RenderStage, VirtualLineEntry, VirtualLinePosition,
    },
};

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

/// Markdown render stage - populates decorations (conceals, backgrounds)
///
/// This stage reads decorations from DecorationStore and populates
/// the decorations field in RenderData for each line.
pub struct MarkdownRenderStage {
    decoration_store: Arc<RwLock<DecorationStore>>,
}

impl MarkdownRenderStage {
    /// Create new markdown render stage
    pub fn new(decoration_store: Arc<RwLock<DecorationStore>>) -> Self {
        Self { decoration_store }
    }
}

/// Markdown table border render stage - adds virtual lines for table borders
///
/// This stage detects markdown tables and adds virtual lines for top/bottom borders.
/// It doesn't need DecorationStore - it works directly on the input lines.
pub struct MarkdownTableBorderStage;

impl MarkdownTableBorderStage {
    /// Create new table border stage
    pub fn new() -> Self {
        Self
    }

    /// Detect table ranges in the content
    ///
    /// Returns (start_line, end_line, delimiter_line) tuples
    fn detect_tables(lines: &[String]) -> Vec<(usize, usize, Option<usize>)> {
        let mut tables = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            // Check if this line starts a table (contains | and non-trivial content)
            if Self::is_table_line(&lines[i]) {
                let start = i;
                let mut end = i;
                let mut delimiter = None;

                // Find table extent
                while end < lines.len() && Self::is_table_line(&lines[end]) {
                    // Check if this is a delimiter row (|---|)
                    if delimiter.is_none() && Self::is_delimiter_row(&lines[end]) {
                        delimiter = Some(end);
                    }
                    end += 1;
                }

                // Only add if we have at least a header and delimiter
                if end > start && delimiter.is_some() {
                    tables.push((start, end - 1, delimiter));
                }

                i = end;
            } else {
                i += 1;
            }
        }

        tables
    }

    /// Check if a line looks like a table row
    fn is_table_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.contains('|') && !trimmed.is_empty()
    }

    /// Check if a line is a delimiter row (|---|---|)
    fn is_delimiter_row(line: &str) -> bool {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            return false;
        }
        // Should contain mostly dashes, pipes, colons, and spaces
        trimmed
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
            && trimmed.contains('-')
    }

    /// Parse cells from a table row (content between pipes)
    fn parse_cells(line: &str) -> Vec<String> {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            return Vec::new();
        }
        // Split by | and get cell content, skip first/last empty
        trimmed[1..trimmed.len() - 1]
            .split('|')
            .map(|s| s.trim().to_string())
            .collect()
    }

    /// Get column widths from a line (character count between pipes, excluding padding)
    fn get_column_widths(line: &str) -> Vec<usize> {
        Self::parse_cells(line)
            .iter()
            .map(|cell| cell.chars().count())
            .collect()
    }

    /// Calculate max column widths across all rows in a table
    fn calculate_max_column_widths(lines: &[String], start: usize, end: usize) -> Vec<usize> {
        let mut max_widths: Vec<usize> = Vec::new();

        for line in lines.iter().take(end + 1).skip(start) {
            // Skip delimiter rows for width calculation (they only have dashes)
            if Self::is_delimiter_row(line) {
                continue;
            }

            let widths = Self::get_column_widths(line);
            for (i, w) in widths.into_iter().enumerate() {
                if i >= max_widths.len() {
                    max_widths.push(w);
                } else if w > max_widths[i] {
                    max_widths[i] = w;
                }
            }
        }

        max_widths
    }

    /// Generate border from column widths
    fn generate_border_from_widths(
        widths: &[usize],
        start_char: char,
        mid_char: char,
        end_char: char,
    ) -> String {
        if widths.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        result.push(start_char);

        for (i, &w) in widths.iter().enumerate() {
            // Add 2 for padding (space on each side)
            for _ in 0..(w + 2) {
                result.push('─');
            }
            if i < widths.len() - 1 {
                result.push(mid_char);
            }
        }

        result.push(end_char);
        result
    }

    /// Build column mapping for cursor position (buffer_col → visual_col)
    ///
    /// Maps each byte position in the original line to the corresponding
    /// visual position in the expanded table row.
    fn build_column_mapping(original: &str, max_widths: &[usize]) -> Vec<u16> {
        let orig_len = original.len();
        if orig_len == 0 || max_widths.is_empty() {
            return Vec::new();
        }

        // Find pipe byte positions in original
        let orig_pipes: Vec<usize> = original
            .bytes()
            .enumerate()
            .filter(|(_, b)| *b == b'|')
            .map(|(i, _)| i)
            .collect();

        if orig_pipes.len() < 2 {
            return (0..orig_len as u16).collect();
        }

        // Calculate visual pipe positions
        // Pattern: │ + " content " + │ + ...
        // Each cell takes: (max_width + 2) chars for content, then │
        // pipe[i] = pipe[i-1] + width[i-1] + 3
        let mut visual_pipes = vec![0u16];
        for &width in max_widths {
            let prev = *visual_pipes.last().unwrap();
            visual_pipes.push(prev + (width as u16) + 3);
        }

        // Build mapping for each buffer position
        (0..orig_len)
            .map(|buf_col| Self::map_to_visual(buf_col, &orig_pipes, &visual_pipes))
            .collect()
    }

    /// Map a buffer column to visual column using pipe positions
    ///
    /// This mapping ensures cursor NEVER appears in padding areas:
    /// - Pipe positions map exactly to visual pipe positions
    /// - Leading space maps to visual leading space
    /// - Content characters map at same offset from cell start
    /// - Trailing space maps to visual trailing space (skipping padding)
    fn map_to_visual(buf_col: usize, orig_pipes: &[usize], visual_pipes: &[u16]) -> u16 {
        // Exact pipe match
        if let Some(idx) = orig_pipes.iter().position(|&p| p == buf_col) {
            return visual_pipes.get(idx).copied().unwrap_or(buf_col as u16);
        }

        // Find which segment (between pipes) we're in
        let seg = orig_pipes
            .iter()
            .position(|&p| p > buf_col)
            .unwrap_or(orig_pipes.len());

        if seg == 0 || seg > orig_pipes.len() {
            return buf_col as u16;
        }

        // We're in the cell between orig_pipes[seg-1] and orig_pipes[seg]
        let buf_cell_start = orig_pipes[seg - 1] + 1;
        let buf_cell_end = orig_pipes[seg];
        let buf_cell_len = buf_cell_end - buf_cell_start;

        let vis_cell_start = visual_pipes[seg - 1] + 1;
        let vis_cell_end = visual_pipes[seg];

        let pos_in_buf_cell = buf_col - buf_cell_start;

        // Map based on position type:
        // - pos 0: leading space → visual leading space
        // - last pos (buf_cell_len-1): trailing space → visual trailing space (skips padding)
        // - middle positions: content → same offset, clamped to content area
        if pos_in_buf_cell == 0 {
            // Leading space
            vis_cell_start
        } else if pos_in_buf_cell >= buf_cell_len.saturating_sub(1) {
            // Trailing space - map to position right before next pipe (skips padding)
            vis_cell_end.saturating_sub(1)
        } else {
            // Content - same offset from cell start, but clamped to content area
            // Content area ends 1 before trailing space (which is 1 before pipe)
            let max_content_pos = vis_cell_end.saturating_sub(2);
            (vis_cell_start + pos_in_buf_cell as u16).min(max_content_pos)
        }
    }

    /// Build expanded row with proper alignment
    /// - Header rows: centered
    /// - Data rows: left-aligned
    fn build_expanded_row(line: &str, max_widths: &[usize], is_header: bool) -> String {
        let cells = Self::parse_cells(line);
        if cells.is_empty() {
            return line.to_string();
        }

        let mut result = String::from("│");

        for (i, cell) in cells.iter().enumerate() {
            let width = max_widths.get(i).copied().unwrap_or(cell.chars().count());
            let cell_len = cell.chars().count();

            let padded = if is_header {
                // CENTER align for headers
                let total_pad = width.saturating_sub(cell_len);
                let left_pad = total_pad / 2;
                let right_pad = total_pad - left_pad;
                format!(" {}{}{} ", " ".repeat(left_pad), cell, " ".repeat(right_pad))
            } else {
                // LEFT align for data rows
                format!(" {:width$} ", cell, width = width)
            };
            result.push_str(&padded);
            result.push('│');
        }

        result
    }

    /// Populate virtual lines for table borders and full-line conceals for all rows
    /// Lines in `skip_lines` will not receive decorations (insert mode cursor, visual selection)
    fn populate_table_borders(&self, input: &mut RenderData, skip_lines: &HashSet<usize>) {
        let tables = Self::detect_tables(&input.lines);

        // Default style for borders (dim gray)
        let border_style = Style::new().fg(Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        });

        for (start, end, delimiter) in tables {
            let Some(delim_idx) = delimiter else {
                continue;
            };

            // Calculate max column widths across ALL rows
            let max_widths = Self::calculate_max_column_widths(&input.lines, start, end);
            if max_widths.is_empty() {
                continue;
            }

            // Generate top border from max widths
            let top_border = Self::generate_border_from_widths(&max_widths, '┌', '┬', '┐');
            if !top_border.is_empty() {
                input.virtual_lines.insert(
                    (start, VirtualLinePosition::Before),
                    VirtualLineEntry {
                        text: top_border,
                        style: border_style.clone(),
                        priority: 100,
                    },
                );
            }

            // Generate bottom border from max widths
            let bottom_border = Self::generate_border_from_widths(&max_widths, '└', '┴', '┘');
            if !bottom_border.is_empty() {
                input.virtual_lines.insert(
                    (end, VirtualLinePosition::After),
                    VirtualLineEntry {
                        text: bottom_border,
                        style: border_style.clone(),
                        priority: 100,
                    },
                );
            }

            // Apply full-line conceals for ALL table rows
            for line_idx in start..=end {
                if line_idx >= input.decorations.len() || line_idx >= input.lines.len() {
                    continue;
                }

                // Skip decorations on lines marked for skipping (insert mode, visual selection)
                if skip_lines.contains(&line_idx) {
                    continue;
                }

                let line = &input.lines[line_idx];

                let (replacement, col_mapping) = if line_idx == delim_idx {
                    // Delimiter row → middle border (no cursor mapping needed)
                    (Self::generate_border_from_widths(&max_widths, '├', '┼', '┤'), None)
                } else if line_idx < delim_idx {
                    // Header row → centered content
                    let mapping = Self::build_column_mapping(line, &max_widths);
                    (Self::build_expanded_row(line, &max_widths, true), Some(mapping))
                } else {
                    // Data row → left-aligned content
                    let mapping = Self::build_column_mapping(line, &max_widths);
                    (Self::build_expanded_row(line, &max_widths, false), Some(mapping))
                };

                if !replacement.is_empty() {
                    // Clear existing decorations and add our full-line replacement
                    input.decorations[line_idx].clear();
                    input.decorations[line_idx].push(Decoration {
                        start_col: 0,
                        end_col: line.len(),
                        kind: DecorationKind::Conceal {
                            replacement: Some(replacement),
                            col_mapping,
                        },
                    });
                }
            }
        }
    }
}

impl Default for MarkdownTableBorderStage {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderStage for MarkdownTableBorderStage {
    fn transform(&self, mut input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        // Clone skip_decoration_lines to avoid borrow conflict
        let skip_lines = input.skip_decoration_lines.clone();
        self.populate_table_borders(&mut input, &skip_lines);
        input
    }

    fn name(&self) -> &'static str {
        "markdown-table-borders"
    }
}

impl RenderStage for MarkdownRenderStage {
    fn transform(&self, mut input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        let buffer_id = input.buffer_id;

        // Lock decoration store for reading
        let store = match self.decoration_store.read() {
            Ok(store) => store,
            Err(_) => return input, // Poisoned lock - skip decorations
        };

        // Populate decorations for each line
        for (line_idx, line_decorations) in input.decorations.iter_mut().enumerate() {
            let line_num = line_idx as u32;

            // Get decorations from store
            let store_decorations = store.get_line_decorations(buffer_id, line_num);

            // Convert from decoration::Decoration to render::Decoration
            *line_decorations = store_decorations
                .into_iter()
                .filter_map(|deco| {
                    // Only process decorations that apply to this line
                    match deco {
                        reovim_core::decoration::Decoration::Conceal {
                            span,
                            replacement,
                            style,
                        } if span.start_line == line_num => {
                            let kind = if replacement.is_empty() {
                                DecorationKind::Conceal {
                                    replacement: None,
                                    col_mapping: None,
                                }
                            } else {
                                DecorationKind::Conceal {
                                    replacement: Some(replacement.clone()),
                                    col_mapping: None,
                                }
                            };

                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind,
                            })
                        }
                        reovim_core::decoration::Decoration::Hide { span }
                            if span.start_line == line_num =>
                        {
                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind: DecorationKind::Conceal {
                                    replacement: None,
                                    col_mapping: None,
                                },
                            })
                        }
                        reovim_core::decoration::Decoration::InlineStyle { span, style }
                            if span.start_line == line_num =>
                        {
                            // Inline styles can be represented as background decorations
                            Some(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind: DecorationKind::Background {
                                    style: style.clone(),
                                },
                            })
                        }
                        _ => None,
                    }
                })
                .collect();
        }

        input
    }

    fn name(&self) -> &'static str {
        "markdown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tables() {
        let lines = vec![
            "Some text".to_string(),
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
            "More text".to_string(),
        ];

        let tables = MarkdownTableBorderStage::detect_tables(&lines);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0], (1, 3, Some(2))); // start=1, end=3, delimiter=2
    }

    #[test]
    fn test_parse_cells() {
        let cells = MarkdownTableBorderStage::parse_cells("| A | B |");
        assert_eq!(cells, vec!["A", "B"]);

        let cells = MarkdownTableBorderStage::parse_cells("| Long content | x |");
        assert_eq!(cells, vec!["Long content", "x"]);
    }

    #[test]
    fn test_get_column_widths() {
        let widths = MarkdownTableBorderStage::get_column_widths("| A | B |");
        assert_eq!(widths, vec![1, 1]);

        let widths = MarkdownTableBorderStage::get_column_widths("| Long | x |");
        assert_eq!(widths, vec![4, 1]);
    }

    #[test]
    fn test_calculate_max_column_widths() {
        let lines = vec![
            "| h1 | h2 |".to_string(),
            "|---|---|".to_string(),
            "| Long content | x |".to_string(),
        ];
        let max_widths = MarkdownTableBorderStage::calculate_max_column_widths(&lines, 0, 2);
        assert_eq!(max_widths, vec![12, 2]); // "Long content" = 12, "h2" = 2
    }

    #[test]
    fn test_generate_border_from_widths() {
        let widths = vec![1, 1];
        let top = MarkdownTableBorderStage::generate_border_from_widths(&widths, '┌', '┬', '┐');
        assert_eq!(top, "┌───┬───┐"); // width 1 + 2 padding = 3

        let widths = vec![4, 2];
        let middle = MarkdownTableBorderStage::generate_border_from_widths(&widths, '├', '┼', '┤');
        assert_eq!(middle, "├──────┼────┤"); // 4+2=6, 2+2=4
    }

    #[test]
    fn test_build_expanded_row_header_centered() {
        let max_widths = vec![10, 4];
        let row = MarkdownTableBorderStage::build_expanded_row("| h1 | h2 |", &max_widths, true);
        // "h1" (2 chars) centered in 10 → 4 left pad, 4 right pad
        // "h2" (2 chars) centered in 4 → 1 left pad, 1 right pad
        assert_eq!(row, "│     h1     │  h2  │");
    }

    #[test]
    fn test_build_expanded_row_data_left_aligned() {
        let max_widths = vec![10, 4];
        let row = MarkdownTableBorderStage::build_expanded_row("| data | x |", &max_widths, false);
        // "data" (4 chars) left-aligned in 10 → 6 right pad
        // "x" (1 char) left-aligned in 4 → 3 right pad
        assert_eq!(row, "│ data       │ x    │");
    }

    #[test]
    fn test_is_delimiter_row() {
        assert!(MarkdownTableBorderStage::is_delimiter_row("|---|---|"));
        assert!(MarkdownTableBorderStage::is_delimiter_row("| --- | --- |"));
        assert!(MarkdownTableBorderStage::is_delimiter_row("|:---|---:|"));
        assert!(!MarkdownTableBorderStage::is_delimiter_row("| A | B |"));
        assert!(!MarkdownTableBorderStage::is_delimiter_row("no pipes"));
    }

    #[test]
    fn test_populate_table_borders() {
        use reovim_core::render::{Bounds, LineVisibility};

        let stage = MarkdownTableBorderStage::new();

        let mut input = RenderData {
            lines: vec![
                "| A | B |".to_string(),
                "|---|---|".to_string(),
                "| 1 | 2 |".to_string(),
            ],
            visibility: vec![LineVisibility::Visible; 3],
            highlights: vec![Vec::new(); 3],
            decorations: vec![Vec::new(); 3],
            signs: vec![None; 3],
            virtual_texts: vec![None; 3],
            virtual_lines: std::collections::BTreeMap::new(),
            buffer_id: 1,
            window_id: 1,
            window_bounds: Bounds {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            cursor: (0, 0),
            skip_decoration_lines: HashSet::new(),
        };

        stage.populate_table_borders(&mut input, &HashSet::new());

        // Should have top border before line 0
        assert!(
            input
                .virtual_lines
                .contains_key(&(0, VirtualLinePosition::Before))
        );
        let top = input
            .virtual_lines
            .get(&(0, VirtualLinePosition::Before))
            .unwrap();
        assert_eq!(top.text, "┌───┬───┐");

        // Should have bottom border after line 2
        assert!(
            input
                .virtual_lines
                .contains_key(&(2, VirtualLinePosition::After))
        );
        let bottom = input
            .virtual_lines
            .get(&(2, VirtualLinePosition::After))
            .unwrap();
        assert_eq!(bottom.text, "└───┴───┘");

        // Header row should have centered content with box drawing pipes
        let header_deco = &input.decorations[0][0];
        if let DecorationKind::Conceal {
            replacement: Some(header),
            ..
        } = &header_deco.kind
        {
            assert_eq!(header, "│ A │ B │");
        } else {
            panic!("Expected Conceal decoration with replacement for header");
        }

        // Delimiter row should have middle border
        let delim_deco = &input.decorations[1][0];
        if let DecorationKind::Conceal {
            replacement: Some(middle),
            ..
        } = &delim_deco.kind
        {
            assert_eq!(middle, "├───┼───┤");
        } else {
            panic!("Expected Conceal decoration with replacement for delimiter");
        }

        // Data row should have left-aligned content with box drawing pipes
        let data_deco = &input.decorations[2][0];
        if let DecorationKind::Conceal {
            replacement: Some(data),
            ..
        } = &data_deco.kind
        {
            assert_eq!(data, "│ 1 │ 2 │");
        } else {
            panic!("Expected Conceal decoration with replacement for data");
        }
    }

    #[test]
    fn test_populate_table_borders_unbalanced() {
        use reovim_core::render::{Bounds, LineVisibility};

        let stage = MarkdownTableBorderStage::new();

        // Unbalanced table - data row is wider than header
        // Max widths: ["Abcdeffase" = 11, "dfsfs" = 5, "dffdf" = 5]
        let mut input = RenderData {
            lines: vec![
                "| h1 | h2 | h3 |".to_string(),
                "|----|----|----|".to_string(),
                "| Abcdeffase | dfsfs | dffdf |".to_string(),
            ],
            visibility: vec![LineVisibility::Visible; 3],
            highlights: vec![Vec::new(); 3],
            decorations: vec![Vec::new(); 3],
            signs: vec![None; 3],
            virtual_texts: vec![None; 3],
            virtual_lines: std::collections::BTreeMap::new(),
            buffer_id: 1,
            window_id: 1,
            window_bounds: Bounds {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            cursor: (0, 0),
            skip_decoration_lines: HashSet::new(),
        };

        stage.populate_table_borders(&mut input, &HashSet::new());

        // Borders now use MAX column widths: [10, 5, 5]
        // "Abcdeffase" = 10 chars, "dfsfs" = 5 chars, "dffdf" = 5 chars
        // Border width = content_width + 2 (for padding)

        // Top border from max widths: 10+2=12, 5+2=7, 5+2=7
        let top = input
            .virtual_lines
            .get(&(0, VirtualLinePosition::Before))
            .unwrap();
        assert_eq!(top.text, "┌────────────┬───────┬───────┐");

        // Bottom border from max widths
        let bottom = input
            .virtual_lines
            .get(&(2, VirtualLinePosition::After))
            .unwrap();
        assert_eq!(bottom.text, "└────────────┴───────┴───────┘");

        // Middle border from max widths
        let delim_decoration = &input.decorations[1][0];
        if let DecorationKind::Conceal {
            replacement: Some(middle),
            ..
        } = &delim_decoration.kind
        {
            assert_eq!(middle, "├────────────┼───────┼───────┤");
        } else {
            panic!("Expected Conceal decoration with replacement");
        }

        // Header row should be centered within expanded widths
        let header_decoration = &input.decorations[0][0];
        if let DecorationKind::Conceal {
            replacement: Some(header),
            ..
        } = &header_decoration.kind
        {
            // "h1" (2 chars) centered in 10: pad = (10-2)=8, left=4, right=4
            // format " {left_pad}{cell}{right_pad} " → " ____h1____ " (12 chars)
            // "h2" (2 chars) centered in 5: pad = (5-2)=3, left=1, right=2
            // format " {left_pad}{cell}{right_pad} " → "  h2   " (7 chars)
            assert_eq!(header, "│     h1     │  h2   │  h3   │");
        } else {
            panic!("Expected Conceal decoration with replacement for header");
        }

        // Data row should be left-aligned within expanded widths
        let data_decoration = &input.decorations[2][0];
        if let DecorationKind::Conceal {
            replacement: Some(data),
            ..
        } = &data_decoration.kind
        {
            assert_eq!(data, "│ Abcdeffase │ dfsfs │ dffdf │");
        } else {
            panic!("Expected Conceal decoration with replacement for data");
        }
    }

    #[test]
    fn test_build_column_mapping() {
        // Simple table: | A | B |
        // Original pipes at: 0, 4, 8
        // max_widths = [1, 1]
        // Visual:  │ A │ B │
        // Visual pipes at: 0, 4, 8 (same as original for equal widths)
        let mapping = MarkdownTableBorderStage::build_column_mapping("| A | B |", &[1, 1]);
        assert_eq!(mapping[0], 0); // First pipe
        assert_eq!(mapping[4], 4); // Second pipe
        assert_eq!(mapping[8], 8); // Third pipe
    }

    #[test]
    fn test_build_column_mapping_expanded() {
        // Table with expanded widths
        // Original: | h1 | h2 |
        // Pipes at byte positions: 0, 5, 10
        // max_widths = [4, 2]
        // Visual: │ h1 │ h2 │  (padded to widths)
        // Visual pipes: [0, 7, 12]
        // pipe[0] = 0
        // pipe[1] = 0 + 4 + 3 = 7
        // pipe[2] = 7 + 2 + 3 = 12
        let mapping = MarkdownTableBorderStage::build_column_mapping("| h1 | h2 |", &[4, 2]);

        // Original: | h 1   |   h 2   |
        // Position: 0 1 2 3 4 5 6 7 8 9 10
        // Pipes at 0, 5, 10
        assert_eq!(mapping[0], 0); // First pipe → 0
        assert_eq!(mapping[5], 7); // Second pipe → 7
        assert_eq!(mapping[10], 12); // Third pipe → 12
    }

    #[test]
    fn test_column_mapping_in_decoration() {
        use reovim_core::render::{Bounds, LineVisibility};

        let stage = MarkdownTableBorderStage::new();

        let mut input = RenderData {
            lines: vec![
                "| A | B |".to_string(),
                "|---|---|".to_string(),
                "| 1 | 2 |".to_string(),
            ],
            visibility: vec![LineVisibility::Visible; 3],
            highlights: vec![Vec::new(); 3],
            decorations: vec![Vec::new(); 3],
            signs: vec![None; 3],
            virtual_texts: vec![None; 3],
            virtual_lines: std::collections::BTreeMap::new(),
            buffer_id: 1,
            window_id: 1,
            window_bounds: Bounds {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            cursor: (0, 0),
            skip_decoration_lines: HashSet::new(),
        };

        stage.populate_table_borders(&mut input, &HashSet::new());

        // Header row should have column mapping
        let header_deco = &input.decorations[0][0];
        if let DecorationKind::Conceal { col_mapping, .. } = &header_deco.kind {
            assert!(col_mapping.is_some(), "Header should have column mapping");
            let mapping = col_mapping.as_ref().unwrap();
            // Original: | A | B |
            // Pipes at: 0, 4, 8
            assert_eq!(mapping[0], 0); // First pipe
            assert_eq!(mapping[4], 4); // Second pipe
            assert_eq!(mapping[8], 8); // Third pipe
        } else {
            panic!("Expected Conceal decoration for header");
        }

        // Delimiter row should NOT have column mapping
        let delim_deco = &input.decorations[1][0];
        if let DecorationKind::Conceal { col_mapping, .. } = &delim_deco.kind {
            assert!(col_mapping.is_none(), "Delimiter should not have column mapping");
        }

        // Data row should have column mapping
        let data_deco = &input.decorations[2][0];
        if let DecorationKind::Conceal { col_mapping, .. } = &data_deco.kind {
            assert!(col_mapping.is_some(), "Data row should have column mapping");
        }
    }

    #[test]
    fn test_column_mapping_skips_padding() {
        // Test that trailing space in buffer maps to visual trailing space, NOT padding
        // Original: | foo | (7 chars)
        // Pipes at: 0, 6
        // Cell content: " foo " (positions 1-5)
        // max_widths = [10] (expanded)
        // Visual: │ foo          │ (14 chars)
        // Visual pipes: [0, 13]
        // Visual cell: positions 1-12
        //   - Leading space: 1
        //   - Content "foo": 2-4
        //   - Padding: 5-11
        //   - Trailing space: 12
        let mapping = MarkdownTableBorderStage::build_column_mapping("| foo |", &[10]);

        // Pipes map exactly
        assert_eq!(mapping[0], 0, "First pipe should map to 0");
        assert_eq!(mapping[6], 13, "Last pipe should map to 13");

        // Leading space
        assert_eq!(mapping[1], 1, "Leading space should map to 1");

        // Content "foo"
        assert_eq!(mapping[2], 2, "f should map to 2");
        assert_eq!(mapping[3], 3, "o should map to 3");
        assert_eq!(mapping[4], 4, "o should map to 4");

        // Trailing space - should skip padding and map to position 12
        assert_eq!(mapping[5], 12, "Trailing space should skip padding and map to 12");
    }

    #[test]
    fn test_column_mapping_no_padding_overlap() {
        // Test that no buffer position maps to padding area (positions 5-11 in visual)
        let mapping = MarkdownTableBorderStage::build_column_mapping("| foo |", &[10]);

        // Collect all mapped visual positions
        let visual_positions: Vec<u16> = mapping.to_vec();

        // Padding area is 5-11 - no buffer position should map there
        for pos in &visual_positions {
            assert!(
                *pos < 5 || *pos > 11,
                "Buffer position should not map to padding area (5-11), got {}",
                pos
            );
        }
    }
}
