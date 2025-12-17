//! Highlight generation from treesitter queries

use tree_sitter::{Point, Query, QueryCursor, StreamingIterator, Tree};

use crate::highlight::{Highlight, HighlightGroup, Span};

use super::theme::TreesitterTheme;

/// Generates highlights from treesitter parse trees
pub struct Highlighter {
    theme: TreesitterTheme,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// Create a new highlighter with default theme
    #[must_use]
    pub fn new() -> Self {
        Self {
            theme: TreesitterTheme::new(),
        }
    }

    /// Create a highlighter with a custom theme
    #[must_use]
    pub const fn with_theme(theme: TreesitterTheme) -> Self {
        Self { theme }
    }

    /// Generate highlights for a range of lines
    ///
    /// Only generates highlights for nodes that intersect with the given line range.
    /// This is useful for only highlighting visible lines.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn highlight_range(
        &self,
        tree: &Tree,
        query: &Query,
        source: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<Highlight> {
        let mut highlights = Vec::new();
        let mut cursor = QueryCursor::new();

        // Limit query to the visible range for performance
        cursor.set_point_range(
            Point::new(start_line as usize, 0)..Point::new(end_line as usize + 1, 0),
        );

        let capture_names = query.capture_names();

        // Use matches() with StreamingIterator
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &capture_names[capture.index as usize];

                // Skip captures that don't have a style
                let Some(style) = self.theme.style_for_capture(capture_name) else {
                    continue;
                };

                let node = capture.node;
                let start = node.start_position();
                let end = node.end_position();

                // Create span from node positions
                // Note: tree-sitter uses byte offsets for columns, but we use character offsets
                // For ASCII this is the same, but for UTF-8 we'd need conversion
                let span = Span::new(
                    start.row as u32,
                    start.column as u32,
                    end.row as u32,
                    end.column as u32,
                );

                highlights.push(Highlight::new(span, style.clone(), HighlightGroup::Syntax));
            }
        }

        highlights
    }

    /// Generate highlights for the entire buffer
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn highlight_all(&self, tree: &Tree, query: &Query, source: &str) -> Vec<Highlight> {
        let line_count = source.lines().count() as u32;
        self.highlight_range(tree, query, source, 0, line_count)
    }
}
