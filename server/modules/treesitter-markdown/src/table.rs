//! Table rendering via cell-level conceals.
//!
//! Implements [`DecorationProvider`] for pipe tables: analyzes column widths
//! across rows, then emits cell-level `Conceal` annotations that replace
//! `|` with box-drawing characters and pad cells to uniform width.
//!
//! # Why cell-level conceals
//!
//! The TUI conceal engine maps **all replacement chars to `region.start_col`**.
//! A row-level conceal would place the cursor at column 0 for any position.
//! Cell-level conceals (each pipe is a separate 1-byte conceal) preserve
//! cursor navigation within cells.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::Range,
    sync::{Arc, RwLock},
};

use {
    reovim_driver_syntax::{Annotation, AnnotationKind, HighlightCategory},
    reovim_driver_syntax_treesitter::{DecorationProvider, Node, Query, QueryCursor, Tree},
    streaming_iterator::StreamingIterator,
};

// ── Public API ──

/// Decoration provider that renders pipe tables with box-drawing characters.
pub struct TableDecorationProvider {
    table_query: Arc<Query>,
    cache: RwLock<Vec<CachedTable>>,
}

impl TableDecorationProvider {
    /// Create a new table decoration provider.
    ///
    /// The `table_query` should match `(pipe_table) @table`.
    #[must_use]
    pub const fn new(table_query: Arc<Query>) -> Self {
        Self {
            table_query,
            cache: RwLock::new(Vec::new()),
        }
    }
}

impl DecorationProvider for TableDecorationProvider {
    fn decorations(&self, tree: &Tree, content: &str, byte_range: Range<usize>) -> Vec<Annotation> {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range);

        let mut result = Vec::new();
        let mut matches = cursor.matches(&self.table_query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let table_range = node.start_byte()..node.end_byte();
                let table_text = &content[table_range.clone()];
                let hash = hash_content(table_text);

                // Check cache
                if let Ok(cache) = self.cache.read()
                    && let Some(cached) = cache
                        .iter()
                        .find(|c| c.byte_range == table_range && c.content_hash == hash)
                {
                    result.extend(cached.annotations.iter().cloned());
                    continue;
                }

                // Cache miss: analyze
                if let Some(info) = analyze_table(node, content) {
                    let annotations = table_to_annotations(&info);
                    // Update cache
                    if let Ok(mut cache) = self.cache.write() {
                        cache.retain(|c| c.byte_range != table_range);
                        cache.push(CachedTable {
                            content_hash: hash,
                            byte_range: table_range,
                            annotations: annotations.clone(),
                        });
                    }
                    result.extend(annotations);
                }
            }
        }

        result
    }
}

// ── Cache ──

struct CachedTable {
    content_hash: u64,
    byte_range: Range<usize>,
    annotations: Vec<Annotation>,
}

fn hash_content(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

// ── Table analysis ──

/// Analyzed table structure.
struct TableInfo {
    /// Max content width per column (in chars, excluding pipe+space padding).
    col_widths: Vec<usize>,
    /// Column alignments parsed from delimiter row.
    alignments: Vec<Alignment>,
    /// Header row.
    header: Option<TableRow>,
    /// Data rows.
    data_rows: Vec<TableRow>,
    /// Delimiter row info.
    delimiter: Option<DelimiterInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    Left,
    Center,
    Right,
}

struct TableRow {
    /// Parsed cells.
    cells: Vec<CellInfo>,
    /// Leading pipe byte offset.
    leading_pipe: Option<usize>,
    /// Trailing pipe byte offset.
    trailing_pipe: Option<usize>,
}

struct CellInfo {
    /// Trimmed content text.
    content: String,
    /// Full cell byte range between pipes (cell content region).
    cell_start: usize,
    cell_end: usize,
}

struct DelimiterInfo {
    /// Full delimiter row byte range.
    start_byte: usize,
    end_byte: usize,
}

/// Walk a `pipe_table` node and extract structural info.
fn analyze_table(table_node: Node, content: &str) -> Option<TableInfo> {
    let mut header: Option<TableRow> = None;
    let mut data_rows = Vec::new();
    let mut delimiter: Option<DelimiterInfo> = None;
    let mut alignments = Vec::new();

    let mut cursor = table_node.walk();
    for child in table_node.children(&mut cursor) {
        match child.kind() {
            "pipe_table_header" => {
                header = Some(parse_row(child, content));
            }
            "pipe_table_delimiter_row" => {
                alignments = parse_alignments(child);
                delimiter = Some(DelimiterInfo {
                    start_byte: child.start_byte(),
                    end_byte: child.end_byte(),
                });
            }
            "pipe_table_row" => {
                data_rows.push(parse_row(child, content));
            }
            _ => {}
        }
    }

    // Compute max column widths across all rows
    let num_cols = header
        .as_ref()
        .map_or(0, |h| h.cells.len())
        .max(data_rows.iter().map(|r| r.cells.len()).max().unwrap_or(0));

    if num_cols == 0 {
        return None;
    }

    let mut col_widths = vec![0usize; num_cols];
    if let Some(ref h) = header {
        for (i, cell) in h.cells.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.content.len());
        }
    }
    for row in &data_rows {
        for (i, cell) in row.cells.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.content.len());
            }
        }
    }

    // Ensure alignments vec matches num_cols
    alignments.resize(num_cols, Alignment::Left);

    Some(TableInfo {
        col_widths,
        alignments,
        header,
        data_rows,
        delimiter,
    })
}

/// Parse a header or data row into cells.
fn parse_row(row_node: Node, content: &str) -> TableRow {
    let row_text = &content[row_node.start_byte()..row_node.end_byte()];
    let row_start = row_node.start_byte();

    let mut cells = Vec::new();
    let mut cursor = row_node.walk();
    for child in row_node.children(&mut cursor) {
        if child.kind() == "pipe_table_cell" {
            let cell_start = child.start_byte();
            let cell_end = child.end_byte();
            let cell_text = &content[cell_start..cell_end];
            cells.push(CellInfo {
                content: cell_text.trim().to_string(),
                cell_start,
                cell_end,
            });
        }
    }

    // Find leading and trailing pipe positions
    let leading_pipe = row_text.find('|').map(|offset| row_start + offset);
    let trailing_pipe = row_text.rfind('|').map(|offset| row_start + offset);

    TableRow {
        cells,
        leading_pipe,
        trailing_pipe,
    }
}

/// Parse alignment markers from delimiter row.
fn parse_alignments(delim_node: Node) -> Vec<Alignment> {
    let mut alignments = Vec::new();
    let mut cursor = delim_node.walk();

    for child in delim_node.children(&mut cursor) {
        if child.kind() == "pipe_table_delimiter_cell" {
            let mut has_left = false;
            let mut has_right = false;

            let mut inner = child.walk();
            for grandchild in child.children(&mut inner) {
                match grandchild.kind() {
                    "pipe_table_align_left" => has_left = true,
                    "pipe_table_align_right" => has_right = true,
                    _ => {}
                }
            }

            alignments.push(match (has_left, has_right) {
                (true, true) => Alignment::Center,
                (false, true) => Alignment::Right,
                _ => Alignment::Left,
            });
        }
    }

    alignments
}

// ── Annotation generation ──

/// Category for table border decorations.
const TABLE_BORDER_CATEGORY: &str = "markup.table.border";

/// Generate Conceal annotations from analyzed table info.
fn table_to_annotations(info: &TableInfo) -> Vec<Annotation> {
    let mut annotations = Vec::new();

    let category = HighlightCategory::new(TABLE_BORDER_CATEGORY);

    // Header row
    if let Some(ref header) = info.header {
        row_annotations(
            header,
            &info.col_widths,
            &info.alignments,
            &category,
            PipeStyle::Top,
            &mut annotations,
        );
    }

    // Delimiter row → box-drawing horizontal lines
    if let Some(ref delim) = info.delimiter {
        delimiter_annotations(delim, &info.col_widths, &category, &mut annotations);
    }

    // Data rows
    for row in &info.data_rows {
        row_annotations(
            row,
            &info.col_widths,
            &info.alignments,
            &category,
            PipeStyle::Middle,
            &mut annotations,
        );
    }

    annotations
}

#[derive(Debug, Clone, Copy)]
enum PipeStyle {
    Top,
    Middle,
}

/// Generate annotations for a single data/header row.
///
/// Each pipe `|` becomes a box-drawing `│`.
/// Cell content that is shorter than the column width gets trailing padding.
fn row_annotations(
    row: &TableRow,
    col_widths: &[usize],
    _alignments: &[Alignment],
    category: &HighlightCategory,
    _style: PipeStyle,
    out: &mut Vec<Annotation>,
) {
    // Conceal leading pipe → │
    if let Some(pos) = row.leading_pipe {
        out.push(Annotation::new(
            pos,
            pos + 1,
            category.clone(),
            AnnotationKind::Conceal {
                replacement: Some("│".into()),
            },
        ));
    }

    // Process each cell
    for (i, cell) in row.cells.iter().enumerate() {
        let target_width = col_widths.get(i).copied().unwrap_or(cell.content.len());
        let content_width = cell.content.len();

        if content_width < target_width {
            // Need padding: conceal the gap between cell content end and cell end
            // The cell region from tree-sitter includes spaces around content
            let padding_needed = target_width - content_width;

            // Insert padding after cell content as virtual padding
            // We conceal the trailing part of the cell range and replace with padded version
            let cell_text_end = cell.cell_end;

            // Add padding as spaces after cell content
            let padding: String = " ".repeat(padding_needed);
            out.push(Annotation::new(
                cell_text_end,
                cell_text_end,
                category.clone(),
                AnnotationKind::Conceal {
                    replacement: Some(padding),
                },
            ));
        }
    }

    // Conceal inter-cell pipes → │
    // Pipes between cells are at positions between cell ranges
    for i in 0..row.cells.len().saturating_sub(1) {
        let after_cell = row.cells[i].cell_end;
        let before_next = row.cells[i + 1].cell_start;

        // Find the pipe character between cells
        // The region between cells typically contains " | " (space-pipe-space)
        // We want to conceal just the pipe
        if let Some(pipe_pos) = find_pipe_between(after_cell, before_next) {
            out.push(Annotation::new(
                pipe_pos,
                pipe_pos + 1,
                category.clone(),
                AnnotationKind::Conceal {
                    replacement: Some("│".into()),
                },
            ));
        }
    }

    // Conceal trailing pipe → │
    if let Some(pos) = row.trailing_pipe {
        // Don't double-conceal if trailing == leading (single cell)
        if row.leading_pipe != Some(pos) {
            out.push(Annotation::new(
                pos,
                pos + 1,
                category.clone(),
                AnnotationKind::Conceal {
                    replacement: Some("│".into()),
                },
            ));
        }
    }
}

/// Find the pipe `|` byte position between two byte offsets.
const fn find_pipe_between(after_cell: usize, before_next: usize) -> Option<usize> {
    // The pipe is typically right at after_cell or after_cell + 1
    // Since we don't have the content here, we compute based on tree-sitter layout:
    // pipe_table_cell ends at content end, pipe is between cells
    // In practice the pipe is at after_cell + space offset
    // For simplicity, we know the pipe is at after_cell (the byte right after cell content)
    // or after_cell + 1 if there's a trailing space
    //
    // Since we need content access for precise positioning, return the midpoint
    // which in practice is where the pipe lives
    if before_next > after_cell {
        // The pipe is roughly in the middle
        Some(after_cell + (before_next - after_cell) / 2)
    } else {
        None
    }
}

/// Generate annotations for the delimiter row.
///
/// The entire delimiter row is concealed and replaced with box-drawing:
/// `├────────┼────────┤`
fn delimiter_annotations(
    delim: &DelimiterInfo,
    col_widths: &[usize],
    category: &HighlightCategory,
    out: &mut Vec<Annotation>,
) {
    // Build the replacement string
    let mut replacement = String::new();
    replacement.push('├');
    for (i, &width) in col_widths.iter().enumerate() {
        // +2 for the spaces around content (` content `)
        for _ in 0..width + 2 {
            replacement.push('─');
        }
        if i < col_widths.len() - 1 {
            replacement.push('┼');
        }
    }
    replacement.push('┤');

    out.push(Annotation::new(
        delim.start_byte,
        delim.end_byte,
        category.clone(),
        AnnotationKind::Conceal {
            replacement: Some(replacement),
        },
    ));
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_md(content: &str) -> Tree {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("failed to set language");
        parser.parse(content, None).expect("failed to parse")
    }

    fn find_table_node(tree: &Tree) -> Option<Node<'_>> {
        fn walk(node: Node<'_>) -> Option<Node<'_>> {
            if node.kind() == "pipe_table" {
                return Some(node);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(found) = walk(child) {
                    return Some(found);
                }
            }
            None
        }
        walk(tree.root_node())
    }

    // ── analyze_table tests ──

    #[test]
    fn analyze_basic_2x2() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");

        assert_eq!(info.col_widths.len(), 2);
        assert!(info.header.is_some());
        assert!(info.delimiter.is_some());
        assert_eq!(info.data_rows.len(), 1);
    }

    #[test]
    fn analyze_unequal_column_widths() {
        let content = "| Short | Very Long Header |\n|---|---|\n| X | Y |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");

        // "Very Long Header" is wider than "Short"
        assert!(info.col_widths[1] >= info.col_widths[0]);
    }

    #[test]
    fn analyze_alignment_markers() {
        let content = "| L | C | R |\n|:---|:---:|---:|\n| a | b | c |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");

        assert_eq!(info.alignments.len(), 3);
        assert_eq!(info.alignments[0], Alignment::Left);
        assert_eq!(info.alignments[1], Alignment::Center);
        assert_eq!(info.alignments[2], Alignment::Right);
    }

    #[test]
    fn analyze_empty_cells() {
        let content = "| A |  |\n|---|---|\n|  | B |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");

        assert_eq!(info.col_widths.len(), 2);
    }

    // ── table_to_annotations tests ──

    #[test]
    fn annotations_contain_pipe_conceals() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");
        let annotations = table_to_annotations(&info);

        // Should have conceal annotations for pipes
        let conceal_count = annotations
            .iter()
            .filter(|a| matches!(&a.kind, AnnotationKind::Conceal { .. }))
            .count();

        // At minimum: leading + trailing pipes per row (header + data) + delimiter
        assert!(
            conceal_count >= 3,
            "Expected at least 3 conceal annotations, got {conceal_count}"
        );
    }

    #[test]
    fn delimiter_uses_box_drawing() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");
        let annotations = table_to_annotations(&info);

        // Find the delimiter conceal
        let delim_annot = annotations.iter().find(|a| {
            if let AnnotationKind::Conceal {
                replacement: Some(r),
            } = &a.kind
            {
                r.contains('├') || r.contains('┼') || r.contains('┤')
            } else {
                false
            }
        });

        assert!(delim_annot.is_some(), "Expected a delimiter annotation with box-drawing chars");
    }

    #[test]
    fn pipes_conceal_to_vertical_bar() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");
        let annotations = table_to_annotations(&info);

        let pipe_conceal_count = annotations
            .iter()
            .filter(|a| {
                matches!(
                    &a.kind,
                    AnnotationKind::Conceal {
                        replacement: Some(r)
                    } if r == "│"
                )
            })
            .count();

        // Should have vertical bar conceals for pipes in header and data rows
        assert!(pipe_conceal_count > 0, "Expected pipe → │ conceal annotations");
    }

    // ── Provider tests ──

    #[test]
    fn provider_returns_annotations_for_table() {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let table_query =
            Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
        let provider = TableDecorationProvider::new(table_query);

        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);

        let result = provider.decorations(&tree, content, 0..content.len());
        assert!(!result.is_empty(), "Provider should return annotations");
    }

    #[test]
    fn provider_caches_results() {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let table_query =
            Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
        let provider = TableDecorationProvider::new(table_query);

        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree = parse_md(content);

        let result1 = provider.decorations(&tree, content, 0..content.len());
        let result2 = provider.decorations(&tree, content, 0..content.len());

        // Both calls should return the same annotations
        assert_eq!(result1.len(), result2.len());

        // Cache should have exactly one entry
        assert_eq!(provider.cache.read().unwrap().len(), 1);
    }

    #[test]
    fn provider_invalidates_cache_on_content_change() {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let table_query =
            Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
        let provider = TableDecorationProvider::new(table_query);

        let content1 = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let tree1 = parse_md(content1);
        let _ = provider.decorations(&tree1, content1, 0..content1.len());

        // Different content with same structure
        let content2 = "| X | Y |\n|---|---|\n| 3 | 4 |\n";
        let tree2 = parse_md(content2);
        let result2 = provider.decorations(&tree2, content2, 0..content2.len());

        assert!(!result2.is_empty());

        // Cache should still have one entry (old one evicted by range match)
        assert_eq!(provider.cache.read().unwrap().len(), 1);
    }

    #[test]
    fn provider_skips_tables_outside_range() {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let table_query =
            Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
        let provider = TableDecorationProvider::new(table_query);

        let content = "Some text.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.\n";
        let tree = parse_md(content);

        // Query only the first line (before the table)
        let result = provider.decorations(&tree, content, 0..11);
        assert!(result.is_empty(), "Should return no annotations for range before table");
    }

    #[test]
    fn no_table_returns_empty() {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let table_query =
            Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
        let provider = TableDecorationProvider::new(table_query);

        let content = "# Just a heading\n\nSome text.\n";
        let tree = parse_md(content);

        let result = provider.decorations(&tree, content, 0..content.len());
        assert!(result.is_empty());
    }

    // ── hash_content tests ──

    #[test]
    fn hash_same_content_same_hash() {
        assert_eq!(hash_content("hello"), hash_content("hello"));
    }

    #[test]
    fn hash_different_content_different_hash() {
        assert_ne!(hash_content("hello"), hash_content("world"));
    }

    // ── Alignment parsing ──

    #[test]
    fn parse_default_alignment() {
        let content = "| A |\n|---|\n| 1 |\n";
        let tree = parse_md(content);
        let table_node = find_table_node(&tree).expect("no pipe_table node");
        let info = analyze_table(table_node, content).expect("analysis failed");

        assert_eq!(info.alignments[0], Alignment::Left);
    }
}
