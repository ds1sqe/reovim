//! Context provider for markdown heading hierarchy

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::RwLock,
};

use {
    reovim_core::context_provider::{ContextHierarchy, ContextItem, ContextProvider},
    tree_sitter::{Language, Parser, Query, StreamingIterator, Tree},
};

/// Context provider for markdown heading hierarchy
pub struct MarkdownContextProvider {
    /// Block-level parser (markdown grammar)
    parser: RwLock<Parser>,
    /// Query for extracting headings
    query: Query,
    /// Cached parse trees per buffer
    trees: RwLock<HashMap<usize, CachedTree>>,
}

struct CachedTree {
    tree: Tree,
    content_hash: u64,
}

impl MarkdownContextProvider {
    /// Create a new markdown context provider
    #[must_use]
    pub fn new() -> Option<Self> {
        let lang: Language = tree_sitter_md::LANGUAGE.into();
        let mut parser = Parser::new();
        parser.set_language(&lang).ok()?;

        // Query for atx headings (# ## ### etc.)
        let query = Query::new(&lang, "(atx_heading) @heading").ok()?;

        Some(Self {
            parser: RwLock::new(parser),
            query,
            trees: RwLock::new(HashMap::new()),
        })
    }

    /// Get or parse tree for buffer
    fn get_or_parse_tree(&self, buffer_id: usize, content: &str) -> Option<Tree> {
        let content_hash = self.hash_content(content);

        // Check cache
        {
            let trees = self.trees.read().unwrap();
            if let Some(cached) = trees.get(&buffer_id)
                && cached.content_hash == content_hash
            {
                return Some(cached.tree.clone());
            }
        }

        // Parse new tree
        let mut parser = self.parser.write().unwrap();
        let tree = parser.parse(content, None)?;

        // Cache it
        self.trees.write().unwrap().insert(
            buffer_id,
            CachedTree {
                tree: tree.clone(),
                content_hash,
            },
        );

        Some(tree)
    }

    /// Simple content hash for cache invalidation
    fn hash_content(&self, content: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.len().hash(&mut hasher);
        hasher.finish()
    }

    /// Extract heading text and level from tree-sitter node
    fn parse_heading(
        &self,
        node: tree_sitter::Node,
        content: &str,
    ) -> Option<(String, usize, u32, u32)> {
        let text = node.utf8_text(content.as_bytes()).ok()?;

        // Count # markers to determine level
        let level = text.chars().take_while(|&c| c == '#').count();
        if level == 0 || level > 6 {
            return None;
        }

        // Extract heading text (remove # markers and trim)
        let heading_text = text.trim_start_matches('#').trim().to_string();

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;

        Some((heading_text, level, start_line, end_line))
    }
}

impl ContextProvider for MarkdownContextProvider {
    fn get_context(
        &self,
        buffer_id: usize,
        line: u32,
        _col: u32,
        content: &str,
    ) -> Option<ContextHierarchy> {
        let tree = self.get_or_parse_tree(buffer_id, content)?;

        // Query all headings
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), content.as_bytes());

        // Build hierarchy stack
        let mut stack: Vec<ContextItem> = Vec::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;

                let Some((text, level, start_line, end_line)) = self.parse_heading(node, content)
                else {
                    continue;
                };

                // Only process headings above or at cursor line
                if start_line > line {
                    break;
                }

                // Pop headings at same or deeper level
                while let Some(top) = stack.last() {
                    if top.level >= level - 1 {
                        // level-1 for 0-indexed
                        stack.pop();
                    } else {
                        break;
                    }
                }

                // Push current heading
                stack.push(ContextItem {
                    text,
                    start_line,
                    end_line,
                    kind: "heading".to_string(),
                    level: level - 1, // Convert to 0-indexed
                });
            }
        }

        if stack.is_empty() {
            return None;
        }

        Some(ContextHierarchy::with_items(buffer_id, line, _col, stack))
    }

    fn name(&self) -> &'static str {
        "markdown"
    }

    fn supports_buffer(&self, _buffer_id: usize) -> bool {
        // TODO: Check if buffer is markdown file
        // For now, return true (will be filtered by decoration factory)
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_heading() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# Introduction\n\nSome text here.\n";

        let ctx = provider.get_context(0, 2, 0, content).unwrap();
        assert_eq!(ctx.items.len(), 1);
        assert_eq!(ctx.items[0].text, "Introduction");
        assert_eq!(ctx.items[0].level, 0);
    }

    #[test]
    fn test_nested_headings() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# Main\n\n## Subsection\n\nContent here.\n";

        let ctx = provider.get_context(0, 4, 0, content).unwrap();
        assert_eq!(ctx.items.len(), 2);
        assert_eq!(ctx.items[0].text, "Main");
        assert_eq!(ctx.items[1].text, "Subsection");
        assert_eq!(ctx.to_breadcrumb(" > "), "Main > Subsection");
    }

    #[test]
    fn test_no_context_before_heading() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "Some intro text.\n\n# First Heading\n";

        let ctx = provider.get_context(0, 0, 0, content);
        assert!(ctx.is_none());
    }

    #[test]
    fn test_cache_hit() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# Test\n\nContent.\n";

        // First call (cache miss)
        provider.get_context(0, 2, 0, content);

        // Second call (cache hit)
        let ctx = provider.get_context(0, 2, 0, content).unwrap();
        assert_eq!(ctx.items[0].text, "Test");
    }

    #[test]
    fn test_multiple_h1s() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# First\n\nContent.\n\n# Second\n\nMore content.\n";

        // Cursor at line 6 (under Second)
        let ctx = provider.get_context(0, 6, 0, content).unwrap();
        assert_eq!(ctx.items.len(), 1);
        assert_eq!(ctx.items[0].text, "Second");
    }

    #[test]
    fn test_deeply_nested() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# H1\n## H2\n### H3\n#### H4\n\nContent.\n";

        let ctx = provider.get_context(0, 5, 0, content).unwrap();
        assert_eq!(ctx.items.len(), 4);
        assert_eq!(ctx.items[0].text, "H1");
        assert_eq!(ctx.items[1].text, "H2");
        assert_eq!(ctx.items[2].text, "H3");
        assert_eq!(ctx.items[3].text, "H4");
    }

    #[test]
    fn test_heading_with_special_chars() {
        let provider = MarkdownContextProvider::new().unwrap();
        let content = "# Introduction: Getting Started\n\nContent.\n";

        let ctx = provider.get_context(0, 2, 0, content).unwrap();
        assert_eq!(ctx.items[0].text, "Introduction: Getting Started");
    }
}
