//! Context provider system for scope detection
//!
//! This module provides the foundation for context/scope detection in buffers.
//! Plugins can implement the [`ContextProvider`] trait to provide hierarchical
//! context information (e.g., markdown headings, treesitter AST nodes, LSP symbols).
//!
//! ## Architecture
//!
//! - [`ContextItem`] - Single level in the context hierarchy (function, class, section)
//! - [`ContextHierarchy`] - Complete context stack at a cursor position
//! - [`ContextProvider`] - Trait for implementing context detection
//!
//! Multiple providers can coexist. The first provider that supports a buffer
//! and returns a result is used.
//!
//! ## Usage
//!
//! ```ignore
//! use reovim_core::context_provider::{ContextProvider, ContextHierarchy, ContextItem};
//! use std::sync::Arc;
//!
//! struct MyContextProvider;
//!
//! impl ContextProvider for MyContextProvider {
//!     fn get_context(&self, buffer_id: usize, line: u32, col: u32) -> Option<ContextHierarchy> {
//!         // Analyze buffer and return hierarchy
//!         Some(ContextHierarchy::with_items(
//!             buffer_id, line, col,
//!             vec![
//!                 ContextItem {
//!                     text: "MyClass".to_string(),
//!                     start_line: 0,
//!                     end_line: 50,
//!                     kind: "class".to_string(),
//!                     level: 0,
//!                 },
//!             ],
//!         ))
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my_provider"
//!     }
//!
//!     fn supports_buffer(&self, buffer_id: usize) -> bool {
//!         // Check if buffer is supported
//!         true
//!     }
//! }
//!
//! // Register with PluginStateRegistry
//! registry.register_context_provider(Arc::new(MyContextProvider));
//!
//! // Query context
//! if let Some(ctx) = registry.get_context(buffer_id, line, col) {
//!     let breadcrumb = ctx.to_breadcrumb(" > ");
//!     println!("Context: {}", breadcrumb);
//! }
//! ```

use std::sync::Arc;

/// A single item in the context hierarchy
///
/// Represents one level of the context stack (e.g., a function, class, or section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    /// Display text for this context level
    pub text: String,
    /// Starting line of this context (0-indexed, inclusive)
    pub start_line: u32,
    /// Ending line of this context (0-indexed, exclusive)
    pub end_line: u32,
    /// Context kind (e.g., "function", "class", "section", "heading")
    pub kind: String,
    /// Nesting level (0 = top level)
    pub level: usize,
}

/// Complete context hierarchy at a cursor position
///
/// Contains the full stack of nested contexts (from outermost to innermost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextHierarchy {
    /// Context items from outermost to innermost
    pub items: Vec<ContextItem>,
    /// Buffer ID this context belongs to
    pub buffer_id: usize,
    /// Line number where this context was queried (0-indexed)
    pub line: u32,
    /// Column number where this context was queried (0-indexed)
    pub col: u32,
}

impl ContextHierarchy {
    /// Create a new empty context hierarchy
    #[must_use]
    pub const fn new(buffer_id: usize, line: u32, col: u32) -> Self {
        Self {
            items: Vec::new(),
            buffer_id,
            line,
            col,
        }
    }

    /// Create a context hierarchy with items
    #[must_use]
    pub const fn with_items(
        buffer_id: usize,
        line: u32,
        col: u32,
        items: Vec<ContextItem>,
    ) -> Self {
        Self {
            items,
            buffer_id,
            line,
            col,
        }
    }

    /// Generate a breadcrumb string from the context hierarchy
    ///
    /// # Arguments
    /// * `separator` - String to place between context items (e.g., " > ", " / ")
    ///
    /// # Returns
    /// Breadcrumb string with items joined by separator, or empty string if no items
    ///
    /// # Example
    /// ```ignore
    /// let breadcrumb = hierarchy.to_breadcrumb(" > ");
    /// // "MyClass > my_method > inner_block"
    /// ```
    #[must_use]
    pub fn to_breadcrumb(&self, separator: &str) -> String {
        self.items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Get the current (innermost) scope
    ///
    /// Returns the last item in the hierarchy, representing the most specific context.
    #[must_use]
    pub fn current_scope(&self) -> Option<&ContextItem> {
        self.items.last()
    }

    /// Get context item at a specific level
    ///
    /// # Arguments
    /// * `level` - Nesting level to retrieve (0 = outermost)
    ///
    /// # Returns
    /// Reference to item at that level, or None if level is out of bounds
    #[must_use]
    pub fn at_level(&self, level: usize) -> Option<&ContextItem> {
        self.items.iter().find(|item| item.level == level)
    }

    /// Get all context items at or above a specific level
    ///
    /// Useful for showing partial breadcrumbs up to a certain depth.
    #[must_use]
    pub fn up_to_level(&self, max_level: usize) -> Vec<&ContextItem> {
        self.items
            .iter()
            .filter(|item| item.level <= max_level)
            .collect()
    }

    /// Check if hierarchy is empty
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of context levels
    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Get the deepest nesting level
    #[must_use]
    pub fn max_level(&self) -> Option<usize> {
        self.items.iter().map(|item| item.level).max()
    }
}

/// Provider trait for context/scope detection
///
/// Implementations analyze buffer content and provide hierarchical context
/// information at a cursor position. Multiple providers can coexist (markdown,
/// treesitter, LSP), with the first matching provider being used.
///
/// # Example
///
/// ```ignore
/// struct MarkdownContextProvider;
///
/// impl ContextProvider for MarkdownContextProvider {
///     fn get_context(&self, buffer_id: usize, line: u32, col: u32) -> Option<ContextHierarchy> {
///         // Parse markdown headings and return hierarchy
///     }
///
///     fn name(&self) -> &'static str {
///         "markdown"
///     }
///
///     fn supports_buffer(&self, buffer_id: usize) -> bool {
///         // Check if buffer is markdown
///     }
/// }
/// ```
pub trait ContextProvider: Send + Sync + 'static {
    /// Get context hierarchy at a cursor position
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer to query
    /// * `line` - Line number (0-indexed)
    /// * `col` - Column number (0-indexed)
    ///
    /// # Returns
    /// Some(hierarchy) if context can be determined, None otherwise
    fn get_context(&self, buffer_id: usize, line: u32, col: u32) -> Option<ContextHierarchy>;

    /// Provider name for debugging/logging
    fn name(&self) -> &'static str;

    /// Check if this provider supports a buffer
    ///
    /// Called before `get_context()` to filter applicable providers.
    fn supports_buffer(&self, buffer_id: usize) -> bool;
}

/// Shared reference to a context provider
pub type SharedContextProvider = Arc<dyn ContextProvider>;

#[cfg(test)]
mod tests {
    use super::*;

    // Test ContextItem creation
    #[test]
    fn test_context_item_creation() {
        let item = ContextItem {
            text: "my_function".to_string(),
            start_line: 10,
            end_line: 20,
            kind: "function".to_string(),
            level: 0,
        };

        assert_eq!(item.text, "my_function");
        assert_eq!(item.start_line, 10);
        assert_eq!(item.end_line, 20);
        assert_eq!(item.kind, "function");
        assert_eq!(item.level, 0);
    }

    // Test ContextHierarchy creation
    #[test]
    fn test_hierarchy_creation() {
        let hierarchy = ContextHierarchy::new(1, 15, 5);
        assert_eq!(hierarchy.buffer_id, 1);
        assert_eq!(hierarchy.line, 15);
        assert_eq!(hierarchy.col, 5);
        assert!(hierarchy.is_empty());
        assert_eq!(hierarchy.len(), 0);
    }

    // Test breadcrumb generation
    #[test]
    fn test_breadcrumb_empty() {
        let hierarchy = ContextHierarchy::new(1, 10, 0);
        assert_eq!(hierarchy.to_breadcrumb(" > "), "");
    }

    #[test]
    fn test_breadcrumb_single_item() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![ContextItem {
                text: "root".to_string(),
                start_line: 0,
                end_line: 100,
                kind: "module".to_string(),
                level: 0,
            }],
        );
        assert_eq!(hierarchy.to_breadcrumb(" > "), "root");
    }

    #[test]
    fn test_breadcrumb_multiple_items() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![
                ContextItem {
                    text: "MyClass".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "class".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "my_method".to_string(),
                    start_line: 10,
                    end_line: 30,
                    kind: "function".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "inner_block".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "block".to_string(),
                    level: 2,
                },
            ],
        );

        assert_eq!(hierarchy.to_breadcrumb(" > "), "MyClass > my_method > inner_block");
        assert_eq!(hierarchy.to_breadcrumb(" / "), "MyClass / my_method / inner_block");
        assert_eq!(hierarchy.to_breadcrumb("::"), "MyClass::my_method::inner_block");
    }

    // Test current_scope
    #[test]
    fn test_current_scope_empty() {
        let hierarchy = ContextHierarchy::new(1, 10, 0);
        assert!(hierarchy.current_scope().is_none());
    }

    #[test]
    fn test_current_scope_with_items() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![
                ContextItem {
                    text: "outer".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "function".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "inner".to_string(),
                    start_line: 10,
                    end_line: 20,
                    kind: "block".to_string(),
                    level: 1,
                },
            ],
        );

        let scope = hierarchy.current_scope().unwrap();
        assert_eq!(scope.text, "inner");
        assert_eq!(scope.level, 1);
    }

    // Test at_level
    #[test]
    fn test_at_level_found() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![
                ContextItem {
                    text: "level0".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "module".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "level1".to_string(),
                    start_line: 10,
                    end_line: 30,
                    kind: "class".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "level2".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "function".to_string(),
                    level: 2,
                },
            ],
        );

        assert_eq!(hierarchy.at_level(0).unwrap().text, "level0");
        assert_eq!(hierarchy.at_level(1).unwrap().text, "level1");
        assert_eq!(hierarchy.at_level(2).unwrap().text, "level2");
    }

    #[test]
    fn test_at_level_not_found() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![ContextItem {
                text: "level0".to_string(),
                start_line: 0,
                end_line: 50,
                kind: "module".to_string(),
                level: 0,
            }],
        );

        assert!(hierarchy.at_level(1).is_none());
        assert!(hierarchy.at_level(999).is_none());
    }

    // Test up_to_level
    #[test]
    fn test_up_to_level() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![
                ContextItem {
                    text: "level0".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "module".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "level1".to_string(),
                    start_line: 10,
                    end_line: 30,
                    kind: "class".to_string(),
                    level: 1,
                },
                ContextItem {
                    text: "level2".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "function".to_string(),
                    level: 2,
                },
            ],
        );

        let up_to_1 = hierarchy.up_to_level(1);
        assert_eq!(up_to_1.len(), 2);
        assert_eq!(up_to_1[0].text, "level0");
        assert_eq!(up_to_1[1].text, "level1");

        let up_to_0 = hierarchy.up_to_level(0);
        assert_eq!(up_to_0.len(), 1);
        assert_eq!(up_to_0[0].text, "level0");
    }

    // Test max_level
    #[test]
    fn test_max_level_empty() {
        let hierarchy = ContextHierarchy::new(1, 10, 0);
        assert!(hierarchy.max_level().is_none());
    }

    #[test]
    fn test_max_level_with_items() {
        let hierarchy = ContextHierarchy::with_items(
            1,
            10,
            0,
            vec![
                ContextItem {
                    text: "level0".to_string(),
                    start_line: 0,
                    end_line: 50,
                    kind: "module".to_string(),
                    level: 0,
                },
                ContextItem {
                    text: "level2".to_string(),
                    start_line: 15,
                    end_line: 20,
                    kind: "function".to_string(),
                    level: 2,
                },
            ],
        );

        assert_eq!(hierarchy.max_level(), Some(2));
    }

    // Test custom provider
    #[test]
    fn test_custom_provider() {
        struct TestProvider;

        impl ContextProvider for TestProvider {
            fn get_context(
                &self,
                buffer_id: usize,
                line: u32,
                _col: u32,
            ) -> Option<ContextHierarchy> {
                if buffer_id == 1 && (10..20).contains(&line) {
                    Some(ContextHierarchy::with_items(
                        buffer_id,
                        line,
                        0,
                        vec![ContextItem {
                            text: "test_context".to_string(),
                            start_line: 10,
                            end_line: 20,
                            kind: "test".to_string(),
                            level: 0,
                        }],
                    ))
                } else {
                    None
                }
            }

            fn name(&self) -> &'static str {
                "test"
            }

            fn supports_buffer(&self, buffer_id: usize) -> bool {
                buffer_id == 1
            }
        }

        let provider = TestProvider;

        // Should support buffer 1
        assert!(provider.supports_buffer(1));
        assert!(!provider.supports_buffer(2));

        // Should return context for lines 10-19
        let ctx = provider.get_context(1, 15, 0);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.items.len(), 1);
        assert_eq!(ctx.items[0].text, "test_context");

        // Should return None outside range
        assert!(provider.get_context(1, 5, 0).is_none());
        assert!(provider.get_context(1, 25, 0).is_none());
        assert!(provider.get_context(2, 15, 0).is_none());
    }

    // Test provider registry integration
    #[test]
    fn test_registry_integration() {
        use crate::plugin::PluginStateRegistry;

        struct Provider1;
        impl ContextProvider for Provider1 {
            fn get_context(
                &self,
                buffer_id: usize,
                line: u32,
                col: u32,
            ) -> Option<ContextHierarchy> {
                Some(ContextHierarchy::with_items(
                    buffer_id,
                    line,
                    col,
                    vec![ContextItem {
                        text: "provider1".to_string(),
                        start_line: 0,
                        end_line: 100,
                        kind: "test".to_string(),
                        level: 0,
                    }],
                ))
            }
            fn name(&self) -> &'static str {
                "provider1"
            }
            fn supports_buffer(&self, buffer_id: usize) -> bool {
                buffer_id == 1
            }
        }

        struct Provider2;
        impl ContextProvider for Provider2 {
            fn get_context(
                &self,
                buffer_id: usize,
                line: u32,
                col: u32,
            ) -> Option<ContextHierarchy> {
                Some(ContextHierarchy::with_items(
                    buffer_id,
                    line,
                    col,
                    vec![ContextItem {
                        text: "provider2".to_string(),
                        start_line: 0,
                        end_line: 100,
                        kind: "test".to_string(),
                        level: 0,
                    }],
                ))
            }
            fn name(&self) -> &'static str {
                "provider2"
            }
            fn supports_buffer(&self, buffer_id: usize) -> bool {
                buffer_id == 2
            }
        }

        let registry = PluginStateRegistry::new();

        // Register providers
        registry.register_context_provider(Arc::new(Provider1));
        registry.register_context_provider(Arc::new(Provider2));

        // Query buffer 1 - should get provider1
        let ctx = registry.get_context(1, 10, 0);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().items[0].text, "provider1");

        // Query buffer 2 - should get provider2
        let ctx = registry.get_context(2, 10, 0);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().items[0].text, "provider2");

        // Query buffer 3 - no provider supports it
        let ctx = registry.get_context(3, 10, 0);
        assert!(ctx.is_none());
    }

    // Test priority (first provider wins)
    #[test]
    fn test_provider_priority() {
        use crate::plugin::PluginStateRegistry;

        struct HighPriority;
        impl ContextProvider for HighPriority {
            fn get_context(
                &self,
                buffer_id: usize,
                line: u32,
                col: u32,
            ) -> Option<ContextHierarchy> {
                Some(ContextHierarchy::with_items(
                    buffer_id,
                    line,
                    col,
                    vec![ContextItem {
                        text: "high".to_string(),
                        start_line: 0,
                        end_line: 100,
                        kind: "test".to_string(),
                        level: 0,
                    }],
                ))
            }
            fn name(&self) -> &'static str {
                "high"
            }
            fn supports_buffer(&self, _buffer_id: usize) -> bool {
                true
            }
        }

        struct LowPriority;
        impl ContextProvider for LowPriority {
            fn get_context(
                &self,
                buffer_id: usize,
                line: u32,
                col: u32,
            ) -> Option<ContextHierarchy> {
                Some(ContextHierarchy::with_items(
                    buffer_id,
                    line,
                    col,
                    vec![ContextItem {
                        text: "low".to_string(),
                        start_line: 0,
                        end_line: 100,
                        kind: "test".to_string(),
                        level: 0,
                    }],
                ))
            }
            fn name(&self) -> &'static str {
                "low"
            }
            fn supports_buffer(&self, _buffer_id: usize) -> bool {
                true
            }
        }

        let registry = PluginStateRegistry::new();

        // Register high priority first
        registry.register_context_provider(Arc::new(HighPriority));
        registry.register_context_provider(Arc::new(LowPriority));

        // Should get high priority provider
        let ctx = registry.get_context(1, 10, 0);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().items[0].text, "high");
    }
}
