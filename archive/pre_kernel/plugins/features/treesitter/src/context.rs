//! Treesitter context provider for scope detection
//!
//! Provides hierarchical context information by analyzing the treesitter AST
//! using language-specific context queries.

use std::sync::Arc;

use {
    reovim_core::context_provider::{ContextHierarchy, ContextItem, ContextProvider},
    tree_sitter::{Node, Point, StreamingIterator},
};

use crate::{queries::QueryType, state::SharedTreesitterManager};

/// Treesitter-based context provider
///
/// Analyzes the parse tree using language-specific context queries to determine
/// the scope hierarchy at a cursor position. Works for all languages that provide
/// a `context_query()` in their `LanguageSupport` implementation.
pub struct TreesitterContextProvider {
    manager: Arc<SharedTreesitterManager>,
}

impl TreesitterContextProvider {
    /// Create a new treesitter context provider
    pub fn new(manager: Arc<SharedTreesitterManager>) -> Self {
        Self { manager }
    }
}

impl ContextProvider for TreesitterContextProvider {
    fn get_context(
        &self,
        buffer_id: usize,
        line: u32,
        col: u32,
        _content: &str,
    ) -> Option<ContextHierarchy> {
        self.manager.with(|mgr| {
            // Get the language ID for this buffer
            let Some(language_id) = mgr.buffer_language(buffer_id) else {
                tracing::debug!(buffer_id, "TreesitterContextProvider: no language for buffer");
                return None;
            };
            tracing::debug!(buffer_id, %language_id, line, col, "TreesitterContextProvider: getting context");

            // Get the parse tree for this buffer
            let Some(tree) = mgr.get_tree(buffer_id) else {
                tracing::debug!(buffer_id, %language_id, "TreesitterContextProvider: no tree for buffer");
                return None;
            };

            // Get the source content (treesitter manager has cached source)
            let Some(source) = mgr.get_source(buffer_id) else {
                tracing::debug!(buffer_id, %language_id, "TreesitterContextProvider: no source for buffer");
                return None;
            };

            // Get the context query for this language
            let Some(query) = mgr.get_cached_query(&language_id, QueryType::Context) else {
                tracing::debug!(buffer_id, %language_id, "TreesitterContextProvider: no context query for language");
                return None;
            };

            // Find the @context capture index
            let context_capture_idx = query
                .capture_names()
                .iter()
                .position(|name| *name == "context")?;

            // Find the @name capture index (optional)
            let name_capture_idx = query
                .capture_names()
                .iter()
                .position(|name| *name == "name");

            // Execute the query
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

            // Collect all context nodes that contain the cursor position
            let cursor_point = Point {
                row: line as usize,
                column: col as usize,
            };

            let mut scopes: Vec<ContextItem> = Vec::new();

            #[allow(clippy::cast_possible_truncation)]
            while let Some(match_) = matches.next() {
                // Find the @context node
                let context_node = match_
                    .captures
                    .iter()
                    .find(|c| c.index as usize == context_capture_idx)
                    .map(|c| c.node);

                let Some(context_node) = context_node else {
                    continue;
                };

                // Check if cursor is within this context node
                let start = context_node.start_position();
                let end = context_node.end_position();

                if !contains_point(start, end, cursor_point) {
                    continue;
                }

                // Extract the name from @name capture if available
                let name_text = name_capture_idx.and_then(|idx| {
                    match_
                        .captures
                        .iter()
                        .find(|c| c.index as usize == idx)
                        .and_then(|c| c.node.utf8_text(source.as_bytes()).ok())
                });

                // Build the display text
                let display_text = build_display_text(&context_node, name_text, source.as_bytes());

                scopes.push(ContextItem {
                    text: display_text,
                    start_line: start.row as u32,
                    end_line: end.row as u32,
                    kind: context_node.kind().to_string(),
                    level: 0, // Will be assigned after sorting
                });
            }

            // Return None if no scopes found (allows fallback to other providers)
            if scopes.is_empty() {
                return None;
            }

            // Sort by start line (outer scopes first), then by end line descending (larger scope first)
            scopes.sort_by(|a, b| {
                a.start_line
                    .cmp(&b.start_line)
                    .then_with(|| b.end_line.cmp(&a.end_line))
            });

            // Deduplicate overlapping scopes at the same start line
            scopes.dedup_by(|a, b| a.start_line == b.start_line);

            // Assign levels based on sorted order
            for (i, scope) in scopes.iter_mut().enumerate() {
                scope.level = i;
            }

            Some(ContextHierarchy::with_items(buffer_id, line, col, scopes))
        })
    }

    fn name(&self) -> &'static str {
        "treesitter"
    }

    fn supports_buffer(&self, buffer_id: usize) -> bool {
        self.manager.with(|mgr| {
            // Support this buffer if we have a parser AND a context query for its language
            if !mgr.has_parser(buffer_id) {
                return false;
            }
            let Some(lang_id) = mgr.buffer_language(buffer_id) else {
                return false;
            };
            // Check if context query exists for this language
            mgr.get_cached_query(&lang_id, QueryType::Context).is_some()
        })
    }
}

/// Check if a point is contained within a range (inclusive start, exclusive end)
fn contains_point(start: Point, end: Point, point: Point) -> bool {
    if point.row < start.row || point.row > end.row {
        return false;
    }
    if point.row == start.row && point.column < start.column {
        return false;
    }
    if point.row == end.row && point.column >= end.column {
        return false;
    }
    true
}

/// Build a display text for a context node
fn build_display_text(node: &Node, name_text: Option<&str>, source: &[u8]) -> String {
    let kind = simplify_kind(node.kind());

    if let Some(name) = name_text {
        return format!("{} {}", kind, name);
    }

    // For impl blocks, try to find the type being implemented
    if node.kind() == "impl_item" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "type_identifier" | "generic_type")
                && let Ok(text) = child.utf8_text(source)
            {
                return format!("impl {}", text);
            }
        }
    }

    // For headings, extract the heading content
    if node.kind() == "atx_heading" {
        // Get the heading level from marker
        let mut cursor = node.walk();
        let mut level = 1;
        for child in node.children(&mut cursor) {
            if child.kind().starts_with("atx_h") && child.kind().ends_with("_marker") {
                // Extract number from "atx_h1_marker", etc.
                if let Some(num) = child.kind().chars().nth(5) {
                    level = num.to_digit(10).unwrap_or(1);
                }
            }
        }

        // Get heading content
        if let Ok(text) = node.utf8_text(source) {
            let heading_text = text.trim_start_matches('#').trim();
            if heading_text.len() > 50 {
                return format!("H{} {}...", level, &heading_text[..47]);
            }
            return format!("H{} {}", level, heading_text);
        }
    }

    // Fallback: use first line of node text, truncated
    if let Ok(text) = node.utf8_text(source) {
        let first_line = text.lines().next().unwrap_or(node.kind());
        if first_line.len() > 50 {
            return format!("{}...", &first_line[..47]);
        }
        return first_line.to_string();
    }

    kind.to_string()
}

/// Simplify node kind to a short, readable form
fn simplify_kind(kind: &str) -> &str {
    match kind {
        // Rust
        "function_item" => "fn",
        "impl_item" => "impl",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "mod_item" => "mod",
        // Python
        "function_definition" => "fn",
        "class_definition" => "class",
        // JavaScript/TypeScript
        "function_declaration" => "fn",
        "class_declaration" => "class",
        "method_definition" => "method",
        "arrow_function" => "fn",
        // C/C++
        "struct_specifier" => "struct",
        "class_specifier" => "class",
        // Go
        "method_declaration" => "method",
        // Java
        "interface_declaration" => "interface",
        // Markdown
        "atx_heading" => "heading",
        _ => kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test simplify_kind for all supported languages
    #[test]
    fn test_simplify_kind_rust() {
        assert_eq!(simplify_kind("function_item"), "fn");
        assert_eq!(simplify_kind("impl_item"), "impl");
        assert_eq!(simplify_kind("struct_item"), "struct");
        assert_eq!(simplify_kind("enum_item"), "enum");
        assert_eq!(simplify_kind("trait_item"), "trait");
        assert_eq!(simplify_kind("mod_item"), "mod");
    }

    #[test]
    fn test_simplify_kind_python() {
        assert_eq!(simplify_kind("function_definition"), "fn");
        assert_eq!(simplify_kind("class_definition"), "class");
    }

    #[test]
    fn test_simplify_kind_javascript() {
        assert_eq!(simplify_kind("function_declaration"), "fn");
        assert_eq!(simplify_kind("class_declaration"), "class");
        assert_eq!(simplify_kind("method_definition"), "method");
        assert_eq!(simplify_kind("arrow_function"), "fn");
    }

    #[test]
    fn test_simplify_kind_markdown() {
        assert_eq!(simplify_kind("atx_heading"), "heading");
    }

    #[test]
    fn test_simplify_kind_unknown() {
        assert_eq!(simplify_kind("unknown_node_type"), "unknown_node_type");
        assert_eq!(simplify_kind("some_random_thing"), "some_random_thing");
    }

    // Test contains_point
    #[test]
    fn test_contains_point_inside() {
        let start = Point { row: 10, column: 0 };
        let end = Point { row: 20, column: 0 };
        let point = Point { row: 15, column: 5 };
        assert!(contains_point(start, end, point));
    }

    #[test]
    fn test_contains_point_at_start() {
        let start = Point { row: 10, column: 5 };
        let end = Point { row: 20, column: 0 };
        let point = Point { row: 10, column: 5 };
        assert!(contains_point(start, end, point));
    }

    #[test]
    fn test_contains_point_before_start() {
        let start = Point { row: 10, column: 5 };
        let end = Point { row: 20, column: 0 };
        let point = Point { row: 10, column: 4 };
        assert!(!contains_point(start, end, point));
    }

    #[test]
    fn test_contains_point_at_end() {
        let start = Point { row: 10, column: 0 };
        let end = Point { row: 20, column: 5 };
        let point = Point { row: 20, column: 5 };
        assert!(!contains_point(start, end, point)); // End is exclusive
    }

    #[test]
    fn test_contains_point_after_end() {
        let start = Point { row: 10, column: 0 };
        let end = Point { row: 20, column: 0 };
        let point = Point { row: 21, column: 0 };
        assert!(!contains_point(start, end, point));
    }

    // Test provider interface
    #[test]
    fn test_provider_interface() {
        use crate::state::SharedTreesitterManager;

        let manager = Arc::new(SharedTreesitterManager::new());
        let provider = TreesitterContextProvider::new(manager);

        // Verify provider name
        assert_eq!(provider.name(), "treesitter");

        // Verify supports_buffer returns false for non-existent buffer
        assert!(!provider.supports_buffer(999));
    }

    // Test context hierarchy methods (from reovim_core)
    #[test]
    fn test_breadcrumb_format() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            15,
            10,
            vec![
                ContextItem {
                    text: "mod core".to_string(),
                    start_line: 0,
                    end_line: 100,
                    kind: "mod_item".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "impl Runtime".to_string(),
                    start_line: 10,
                    end_line: 50,
                    kind: "impl_item".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "fn run".to_string(),
                    start_line: 15,
                    end_line: 30,
                    kind: "function_item".to_string(),
                    level: 2,
                },
            ],
        );

        assert_eq!(hierarchy.to_breadcrumb(" > "), "mod core > impl Runtime > fn run");
        assert_eq!(hierarchy.to_breadcrumb(" / "), "mod core / impl Runtime / fn run");
        assert_eq!(hierarchy.to_breadcrumb("::"), "mod core::impl Runtime::fn run");
    }

    // Test empty hierarchy
    #[test]
    fn test_empty_hierarchy() {
        let hierarchy = ContextHierarchy::with_items(1, 0, 0, vec![]);

        assert!(hierarchy.is_empty());
        assert_eq!(hierarchy.len(), 0);
        assert_eq!(hierarchy.to_breadcrumb(" > "), "");
        assert!(hierarchy.current_scope().is_none());
        assert!(hierarchy.max_level().is_none());
    }

    // Test single-level hierarchy
    #[test]
    fn test_single_level_hierarchy() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            5,
            vec![ContextItem {
                text: "fn main".to_string(),
                start_line: 0,
                end_line: 20,
                kind: "function_item".to_string(),
                level: 0,
            }],
        );

        assert!(!hierarchy.is_empty());
        assert_eq!(hierarchy.len(), 1);
        assert_eq!(hierarchy.to_breadcrumb(" > "), "fn main");
        assert_eq!(hierarchy.current_scope().unwrap().text, "fn main");
        assert_eq!(hierarchy.max_level(), Some(0));
    }
}
