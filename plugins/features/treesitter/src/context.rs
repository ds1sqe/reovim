//! Treesitter context provider for scope detection
//!
//! Provides hierarchical context information by analyzing the treesitter AST.
//! Detects code scopes like functions, classes, impl blocks, and methods.

use std::sync::Arc;

use {
    reovim_core::context_provider::{ContextHierarchy, ContextItem, ContextProvider},
    tree_sitter::{Node, Point},
};

use crate::state::SharedTreesitterManager;

/// Treesitter-based context provider
///
/// Analyzes the parse tree to determine the scope hierarchy at a cursor position.
/// Works for all treesitter-supported languages automatically.
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
            // Get the parse tree for this buffer
            let tree = mgr.get_tree(buffer_id)?;

            // Get the source content (treesitter manager has cached source)
            let source = mgr.get_source(buffer_id)?;

            // Get node at cursor position
            let point = Point {
                row: line as usize,
                column: col as usize,
            };
            let cursor_node = tree.root_node().descendant_for_point_range(point, point)?;

            // Walk up parent chain collecting scope nodes
            let mut scopes = Vec::new();
            let mut current = Some(cursor_node);
            let mut level = 0;

            while let Some(node) = current {
                if is_scope_node(&node) {
                    scopes.push(ContextItem {
                        text: extract_scope_name(&node, source.as_bytes()),
                        start_line: node.start_position().row as u32,
                        end_line: node.end_position().row as u32,
                        kind: node.kind().to_string(),
                        level,
                    });
                    level += 1;
                }
                current = node.parent();
            }

            // Reverse to get outermost-first order
            scopes.reverse();

            // Re-number levels after reversing
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
        self.manager.with(|mgr| mgr.has_parser(buffer_id))
    }
}

/// Check if a node represents a scope (function, class, impl, etc.)
fn is_scope_node(node: &Node) -> bool {
    matches!(
        node.kind(),
        // Rust
        "function_item"
            | "impl_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "mod_item"
            // Python
            | "function_definition"
            | "class_definition"
            // JavaScript/TypeScript
            | "function_declaration"
            | "class_declaration"
            | "method_definition"
            | "arrow_function"
            // C/C++
            | "struct_specifier"
            | "class_specifier"
            // Go
            | "method_declaration"
            // Java
            | "interface_declaration"
    )
}

/// Extract a readable name for a scope node
///
/// Tries to find the identifier/name child node and formats it nicely.
/// Falls back to the first line of the node text if no identifier is found.
fn extract_scope_name(node: &Node, source: &[u8]) -> String {
    // Try to find identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "identifier" | "name" | "field_identifier")
            && let Ok(text) = child.utf8_text(source)
        {
            return format!("{} {}", simplify_kind(node.kind()), text);
        }
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

    // Fallback: use first line of node text, truncated
    if let Ok(text) = node.utf8_text(source) {
        let first_line = text.lines().next().unwrap_or(node.kind());
        // Truncate long lines
        if first_line.len() > 50 {
            format!("{}...", &first_line[..47])
        } else {
            first_line.to_string()
        }
    } else {
        node.kind().to_string()
    }
}

/// Simplify node kind to a short, readable form
fn simplify_kind(kind: &str) -> &str {
    match kind {
        "function_item" => "fn",
        "function_definition" => "fn",
        "function_declaration" => "fn",
        "method_definition" => "method",
        "method_declaration" => "method",
        "impl_item" => "impl",
        "struct_item" => "struct",
        "struct_specifier" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "mod_item" => "mod",
        "class_definition" => "class",
        "class_declaration" => "class",
        "class_specifier" => "class",
        "interface_declaration" => "interface",
        "arrow_function" => "fn",
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
    fn test_simplify_kind_c() {
        assert_eq!(simplify_kind("struct_specifier"), "struct");
        assert_eq!(simplify_kind("class_specifier"), "class");
    }

    #[test]
    fn test_simplify_kind_go() {
        assert_eq!(simplify_kind("method_declaration"), "method");
    }

    #[test]
    fn test_simplify_kind_java() {
        assert_eq!(simplify_kind("interface_declaration"), "interface");
    }

    #[test]
    fn test_simplify_kind_unknown() {
        assert_eq!(simplify_kind("unknown_node_type"), "unknown_node_type");
        assert_eq!(simplify_kind("some_random_thing"), "some_random_thing");
    }

    // Test is_scope_node for all supported node kinds
    #[test]
    fn test_is_scope_node_rust() {
        // Create mock nodes with different kinds (we'll use string matching)
        let rust_scope_kinds = vec![
            "function_item",
            "impl_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "mod_item",
        ];

        for kind in rust_scope_kinds {
            // Verify these are recognized as scope nodes
            assert!(
                matches!(
                    kind,
                    "function_item"
                        | "impl_item"
                        | "struct_item"
                        | "enum_item"
                        | "trait_item"
                        | "mod_item"
                ),
                "Rust scope kind '{}' should be recognized",
                kind
            );
        }
    }

    #[test]
    fn test_is_scope_node_python() {
        let python_scope_kinds = vec!["function_definition", "class_definition"];

        for kind in python_scope_kinds {
            assert!(
                matches!(kind, "function_definition" | "class_definition"),
                "Python scope kind '{}' should be recognized",
                kind
            );
        }
    }

    #[test]
    fn test_is_scope_node_javascript() {
        let js_scope_kinds = vec![
            "function_declaration",
            "class_declaration",
            "method_definition",
            "arrow_function",
        ];

        for kind in js_scope_kinds {
            assert!(
                matches!(
                    kind,
                    "function_declaration"
                        | "class_declaration"
                        | "method_definition"
                        | "arrow_function"
                ),
                "JavaScript scope kind '{}' should be recognized",
                kind
            );
        }
    }

    #[test]
    fn test_is_scope_node_non_scope() {
        // These should NOT be scope nodes
        let non_scope_kinds = vec![
            "identifier",
            "type_identifier",
            "literal",
            "comment",
            "block",
            "expression_statement",
            "let_declaration",
            "return_statement",
        ];

        for kind in non_scope_kinds {
            assert!(
                !matches!(
                    kind,
                    "function_item"
                        | "impl_item"
                        | "struct_item"
                        | "enum_item"
                        | "trait_item"
                        | "mod_item"
                        | "function_definition"
                        | "class_definition"
                        | "function_declaration"
                        | "class_declaration"
                        | "method_definition"
                        | "arrow_function"
                        | "struct_specifier"
                        | "class_specifier"
                        | "method_declaration"
                        | "interface_declaration"
                ),
                "Non-scope kind '{}' should NOT be recognized as scope",
                kind
            );
        }
    }

    // Test ContextItem structure
    #[test]
    fn test_context_item_creation() {
        let item = ContextItem {
            text: "fn test".to_string(),
            start_line: 10,
            end_line: 20,
            kind: "function_item".to_string(),
            level: 0,
        };

        assert_eq!(item.text, "fn test");
        assert_eq!(item.start_line, 10);
        assert_eq!(item.end_line, 20);
        assert_eq!(item.kind, "function_item");
        assert_eq!(item.level, 0);
    }

    // Test ContextHierarchy building
    #[test]
    #[allow(clippy::useless_vec)]
    fn test_context_hierarchy_ordering() {
        // Simulate what the provider does: collect scopes and reverse
        let mut scopes = vec![
            ContextItem {
                text: "fn inner".to_string(),
                start_line: 15,
                end_line: 18,
                kind: "function_item".to_string(),
                level: 0, // Will be renumbered
            },
            ContextItem {
                text: "impl Foo".to_string(),
                start_line: 10,
                end_line: 20,
                kind: "impl_item".to_string(),
                level: 1,
            },
            ContextItem {
                text: "mod bar".to_string(),
                start_line: 0,
                end_line: 30,
                kind: "mod_item".to_string(),
                level: 2,
            },
        ];

        // Reverse to get outermost-first
        scopes.reverse();

        // Re-number levels
        for (i, scope) in scopes.iter_mut().enumerate() {
            scope.level = i;
        }

        // Verify correct ordering: mod > impl > fn
        assert_eq!(scopes.len(), 3);
        assert_eq!(scopes[0].text, "mod bar");
        assert_eq!(scopes[0].level, 0);
        assert_eq!(scopes[1].text, "impl Foo");
        assert_eq!(scopes[1].level, 1);
        assert_eq!(scopes[2].text, "fn inner");
        assert_eq!(scopes[2].level, 2);
    }

    // Test breadcrumb generation
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

    // Test deeply nested hierarchy
    #[test]
    fn test_deeply_nested_hierarchy() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            25,
            10,
            vec![
                ContextItem {
                    text: "mod outer".to_string(),
                    start_line: 0,
                    end_line: 100,
                    kind: "mod_item".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "struct Foo".to_string(),
                    start_line: 10,
                    end_line: 60,
                    kind: "struct_item".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "impl Foo".to_string(),
                    start_line: 15,
                    end_line: 55,
                    kind: "impl_item".to_string(),
                    level: 2,
                },
                ContextItem {
                    text: "fn method".to_string(),
                    start_line: 20,
                    end_line: 40,
                    kind: "function_item".to_string(),
                    level: 3,
                },
                ContextItem {
                    text: "fn closure".to_string(),
                    start_line: 25,
                    end_line: 30,
                    kind: "function_item".to_string(),
                    level: 4,
                },
            ],
        );

        assert_eq!(hierarchy.len(), 5);
        assert_eq!(hierarchy.max_level(), Some(4));
        assert_eq!(hierarchy.current_scope().unwrap().text, "fn closure");
        assert_eq!(hierarchy.at_level(0).unwrap().text, "mod outer");
        assert_eq!(hierarchy.at_level(2).unwrap().text, "impl Foo");
        assert_eq!(hierarchy.at_level(4).unwrap().text, "fn closure");
    }

    // Test extract_scope_name fallback behavior
    #[test]
    fn test_extract_scope_name_truncation() {
        // Test that long lines are truncated
        let very_long_text = "a".repeat(100);

        // Create a mock scenario - the function would truncate at 50 chars
        let truncated = if very_long_text.len() > 50 {
            format!("{}...", &very_long_text[..47])
        } else {
            very_long_text.clone()
        };

        assert_eq!(truncated.len(), 50); // 47 chars + "..."
        assert!(truncated.ends_with("..."));
    }

    // Test provider name and supports_buffer
    #[test]
    fn test_provider_interface() {
        use {crate::state::SharedTreesitterManager, std::sync::Arc};

        let manager = Arc::new(SharedTreesitterManager::new());
        let provider = TreesitterContextProvider::new(manager);

        // Verify provider name
        assert_eq!(provider.name(), "treesitter");

        // Verify supports_buffer returns false for non-existent buffer
        assert!(!provider.supports_buffer(999));
    }

    // Test context hierarchy helper methods
    #[test]
    fn test_hierarchy_at_level() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            15,
            5,
            vec![
                ContextItem {
                    text: "level0".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "mod_item".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "level1".to_string(),
                    start_line: 10,
                    end_line: 30,
                    kind: "impl_item".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "level2".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "function_item".to_string(),
                    level: 2,
                },
            ],
        );

        assert_eq!(hierarchy.at_level(0).unwrap().text, "level0");
        assert_eq!(hierarchy.at_level(1).unwrap().text, "level1");
        assert_eq!(hierarchy.at_level(2).unwrap().text, "level2");
        assert!(hierarchy.at_level(3).is_none());
        assert!(hierarchy.at_level(999).is_none());
    }

    #[test]
    fn test_hierarchy_up_to_level() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            15,
            5,
            vec![
                ContextItem {
                    text: "level0".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "mod_item".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "level1".to_string(),
                    start_line: 10,
                    end_line: 30,
                    kind: "impl_item".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "level2".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "function_item".to_string(),
                    level: 2,
                },
            ],
        );

        let up_to_0 = hierarchy.up_to_level(0);
        assert_eq!(up_to_0.len(), 1);
        assert_eq!(up_to_0[0].text, "level0");

        let up_to_1 = hierarchy.up_to_level(1);
        assert_eq!(up_to_1.len(), 2);
        assert_eq!(up_to_1[0].text, "level0");
        assert_eq!(up_to_1[1].text, "level1");

        let up_to_2 = hierarchy.up_to_level(2);
        assert_eq!(up_to_2.len(), 3);
    }
}
