//! Semantic text object resolution using treesitter queries
//!
//! Provides `TextObjectResolver` for finding bounds of semantic text objects
//! (functions, classes, parameters, etc.) based on treesitter AST queries.

use tree_sitter::{Point, Query, QueryCursor, StreamingIterator, Tree};

use crate::textobject::{SemanticTextObject, SemanticTextObjectSpec, TextObjectScope};

/// Position in the buffer (row, column), 0-indexed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: u32,
    pub col: u32,
}

impl Position {
    /// Create a new position
    #[must_use]
    pub const fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }

    /// Convert from tree-sitter Point
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_point(point: Point) -> Self {
        Self {
            row: point.row as u32,
            col: point.column as u32,
        }
    }
}

/// Bounds of a text object (start and end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjectBounds {
    pub start: Position,
    pub end: Position,
}

impl TextObjectBounds {
    /// Create new bounds
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// Candidate node for text object selection
struct Candidate {
    start: Point,
    end: Point,
    size: usize,
}

/// Resolver for semantic text objects using treesitter queries
pub struct TextObjectResolver;

impl TextObjectResolver {
    /// Find bounds for a semantic text object at the cursor position
    ///
    /// # Arguments
    /// * `tree` - The parsed syntax tree
    /// * `query` - The compiled text objects query
    /// * `source` - The source text
    /// * `cursor_row` - Cursor row (0-indexed)
    /// * `cursor_col` - Cursor column (0-indexed)
    /// * `spec` - The semantic text object specification
    ///
    /// # Returns
    /// The bounds of the text object if found, or None if no matching object contains the cursor
    #[must_use]
    pub fn find_bounds(
        tree: &Tree,
        query: &Query,
        source: &str,
        cursor_row: u32,
        cursor_col: u32,
        spec: &SemanticTextObjectSpec,
    ) -> Option<TextObjectBounds> {
        let capture_name = spec.capture_name();
        let source_bytes = source.as_bytes();

        // Find the capture index for our target
        let capture_names = query.capture_names();
        let capture_index = capture_names
            .iter()
            .position(|name| *name == capture_name)?;

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Collect all matching nodes that contain the cursor position
        let mut candidates: Vec<Candidate> = Vec::new();

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                if capture.index as usize == capture_index {
                    let node = capture.node;
                    let start = node.start_position();
                    let end = node.end_position();

                    // Check if cursor is within this node
                    if Self::position_in_range(cursor_row, cursor_col, &start, &end) {
                        candidates.push(Candidate {
                            start,
                            end,
                            size: Self::node_size(&start, &end),
                        });
                    }
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Find the smallest enclosing node
        candidates.sort_by_key(|c| c.size);

        let best = &candidates[0];

        // For inner scope, we're done
        // For outer scope, try to find a matching outer capture
        let (start, end) = if spec.scope == TextObjectScope::Around {
            Self::find_outer_bounds(tree, query, source, best, *spec)
                .unwrap_or((best.start, best.end))
        } else {
            (best.start, best.end)
        };

        Some(TextObjectBounds {
            start: Position::from_point(start),
            end: Position::from_point(end),
        })
    }

    /// Find bounds for an outer scope text object
    fn find_outer_bounds(
        tree: &Tree,
        query: &Query,
        source: &str,
        inner: &Candidate,
        spec: SemanticTextObjectSpec,
    ) -> Option<(Point, Point)> {
        // Build the outer capture name
        let outer_name = format!("{}.outer", spec.kind.capture_name());
        let source_bytes = source.as_bytes();

        // Find the capture index for outer
        let capture_names = query.capture_names();
        let capture_index = capture_names.iter().position(|name| *name == outer_name)?;

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Convert inner to byte positions for comparison
        let inner_start_row = inner.start.row;
        let inner_end_row = inner.end.row;

        let mut best: Option<(Point, Point, usize)> = None;

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                if capture.index as usize == capture_index {
                    let node = capture.node;
                    let start = node.start_position();
                    let end = node.end_position();

                    // Check if this outer node contains our inner node
                    // Compare by row/column positions
                    let contains = start.row <= inner_start_row
                        && end.row >= inner_end_row
                        && (start.row < inner_start_row || start.column <= inner.start.column)
                        && (end.row > inner_end_row || end.column >= inner.end.column);

                    if contains {
                        let size = Self::node_size(&start, &end);
                        if best.is_none() || size < best.as_ref().unwrap().2 {
                            best = Some((start, end, size));
                        }
                    }
                }
            }
        }

        best.map(|(start, end, _)| (start, end))
    }

    /// Check if a position is within a range
    const fn position_in_range(row: u32, col: u32, start: &Point, end: &Point) -> bool {
        let row = row as usize;
        let col = col as usize;

        if row < start.row || row > end.row {
            return false;
        }

        if row == start.row && col < start.column {
            return false;
        }

        if row == end.row && col > end.column {
            return false;
        }

        true
    }

    /// Calculate approximate node size for sorting
    const fn node_size(start: &Point, end: &Point) -> usize {
        // Simple heuristic: row difference * 1000 + column span
        (end.row.saturating_sub(start.row)) * 1000 + end.column.saturating_sub(start.column)
    }

    /// Resolve a semantic text object directly from text object type
    ///
    /// Convenience method that combines scope and kind into a spec.
    #[must_use]
    pub fn resolve(
        tree: &Tree,
        query: &Query,
        source: &str,
        cursor_row: u32,
        cursor_col: u32,
        kind: SemanticTextObject,
        scope: TextObjectScope,
    ) -> Option<TextObjectBounds> {
        let spec = SemanticTextObjectSpec::new(scope, kind);
        Self::find_bounds(tree, query, source, cursor_row, cursor_col, &spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_in_range() {
        let start = Point { row: 1, column: 5 };
        let end = Point { row: 3, column: 10 };

        // Inside
        assert!(TextObjectResolver::position_in_range(2, 0, &start, &end));
        assert!(TextObjectResolver::position_in_range(1, 5, &start, &end));
        assert!(TextObjectResolver::position_in_range(3, 10, &start, &end));

        // Outside
        assert!(!TextObjectResolver::position_in_range(0, 0, &start, &end));
        assert!(!TextObjectResolver::position_in_range(1, 4, &start, &end));
        assert!(!TextObjectResolver::position_in_range(3, 11, &start, &end));
        assert!(!TextObjectResolver::position_in_range(4, 0, &start, &end));
    }

    #[test]
    fn test_node_size() {
        let start = Point { row: 0, column: 0 };
        let end = Point { row: 2, column: 10 };
        let size = TextObjectResolver::node_size(&start, &end);
        assert_eq!(size, 2010); // 2 * 1000 + 10
    }
}
