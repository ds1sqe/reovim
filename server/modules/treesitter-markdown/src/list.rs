//! List bullet annotations with depth-dependent categories.
//!
//! Implements [`DecorationProvider`] for list markers: walks up from each
//! marker node counting `list` ancestors to determine nesting depth, then
//! emits an annotation with a depth-encoded category (`markup.list.bullet.{N}`).
//!
//! The client decides which glyph to render for each depth level.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::Range,
    sync::{Arc, RwLock},
};

use {
    reovim_driver_syntax_treesitter::{DecorationProvider, Node, Query, QueryCursor, Tree},
    reovim_driver_text_syntax::{Annotation, HighlightCategory},
    streaming_iterator::StreamingIterator,
};

// ── Public API ──

/// Decoration provider that emits depth-encoded list bullet annotations.
pub struct ListDecorationProvider {
    list_query: Arc<Query>,
    cache: RwLock<Vec<CachedList>>,
}

impl ListDecorationProvider {
    /// Create a new list decoration provider.
    ///
    /// The `list_query` should match list marker nodes with `@marker` captures.
    #[must_use]
    pub const fn new(list_query: Arc<Query>) -> Self {
        Self {
            list_query,
            cache: RwLock::new(Vec::new()),
        }
    }
}

impl DecorationProvider for ListDecorationProvider {
    #[cfg_attr(coverage_nightly, coverage(off))] // RwLock poison + MC/DC cache-find branches unreachable
    fn decorations(&self, tree: &Tree, content: &str, byte_range: Range<usize>) -> Vec<Annotation> {
        let clamped = byte_range.start.min(content.len())..byte_range.end.min(content.len());
        let content_hash = hash_content(&content[clamped]);

        // Check cache
        if let Ok(cache) = self.cache.read()
            && let Some(cached) = cache
                .iter()
                .find(|c| c.byte_range == byte_range && c.content_hash == content_hash)
        {
            return cached.annotations.clone();
        }

        // Cache miss: compute
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range.clone());

        let mut annotations = Vec::new();
        let mut matches = cursor.matches(&self.list_query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let depth = nesting_depth(node);
                let category = format!("markup.list.bullet.{depth}");

                annotations.push(Annotation::new(
                    node.start_byte(),
                    node.end_byte(),
                    HighlightCategory::new(category),
                ));
            }
        }

        // Store in cache
        if let Ok(mut cache) = self.cache.write() {
            cache.retain(|c| c.byte_range != byte_range);
            cache.push(CachedList {
                content_hash,
                byte_range,
                annotations: annotations.clone(),
            });
        }

        annotations
    }
}

// ── Helpers ──

/// Calculate nesting depth by counting `list` ancestors.
///
/// The first `list` parent is depth 0. Each additional `list` ancestor
/// increments the depth.
fn nesting_depth(node: Node) -> usize {
    let mut depth: usize = 0;
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "list" {
            depth += 1;
        }
        current = parent;
    }
    // First list parent = depth 0
    depth.saturating_sub(1)
}

// ── Cache ──

struct CachedList {
    content_hash: u64,
    byte_range: Range<usize>,
    annotations: Vec<Annotation>,
}

fn hash_content(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
