//! List bullet rendering with depth-dependent glyphs.
//!
//! Implements [`DecorationProvider`] for list markers: walks up from each
//! marker node counting `list` ancestors to determine nesting depth, then
//! emits a `Conceal` annotation with a depth-appropriate glyph.
//!
//! # Glyphs by depth
//!
//! | Depth | Glyph | Unicode |
//! |-------|-------|---------|
//! | 0     | `\u{2022}` (bullet) | BLACK CIRCLE |
//! | 1     | `\u{25E6}` (white bullet) | WHITE BULLET |
//! | 2     | `\u{25AA}` (black small square) | BLACK SMALL SQUARE |
//! | 3+    | `\u{25AB}` (white small square) | WHITE SMALL SQUARE |

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

// ── Glyph table ──

/// Depth 0: black bullet
const BULLET_DEPTH_0: &str = "\u{2022} ";
/// Depth 1: white bullet (circle)
const BULLET_DEPTH_1: &str = "\u{25E6} ";
/// Depth 2: black small square
const BULLET_DEPTH_2: &str = "\u{25AA} ";
/// Depth 3+: white small square
const BULLET_DEPTH_3: &str = "\u{25AB} ";

const fn bullet_for_depth(depth: usize) -> &'static str {
    match depth {
        0 => BULLET_DEPTH_0,
        1 => BULLET_DEPTH_1,
        2 => BULLET_DEPTH_2,
        _ => BULLET_DEPTH_3,
    }
}

// ── Public API ──

/// Decoration provider that renders list bullets with depth-dependent glyphs.
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
                let glyph = bullet_for_depth(depth);

                annotations.push(Annotation::new(
                    node.start_byte(),
                    node.end_byte(),
                    HighlightCategory::new("markup.list"),
                    AnnotationKind::Conceal {
                        replacement: Some(glyph.into()),
                    },
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
mod tests {
    use super::*;

    fn parse_md(content: &str) -> (Tree, Arc<Query>) {
        let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(content, None).unwrap();
        let query = Arc::new(
            Query::new(
                &language,
                "(list_marker_minus) @marker (list_marker_plus) @marker (list_marker_star) @marker",
            )
            .unwrap(),
        );
        (tree, query)
    }

    #[test]
    fn test_list_depth_0_bullet() {
        let content = "- item\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(annotations.len(), 1);
        assert_eq!(
            annotations[0].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_0.into()),
            }
        );
    }

    #[test]
    fn test_list_depth_1_circle() {
        let content = "- outer\n  - inner\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(annotations.len(), 2);
        // First bullet: depth 0
        assert_eq!(
            annotations[0].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_0.into()),
            }
        );
        // Second bullet: depth 1
        assert_eq!(
            annotations[1].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_1.into()),
            }
        );
    }

    #[test]
    fn test_list_depth_2_square() {
        let content = "- a\n  - b\n    - c\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(annotations.len(), 3);
        assert_eq!(
            annotations[2].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_2.into()),
            }
        );
    }

    #[test]
    fn test_list_depth_3_white_square() {
        let content = "- a\n  - b\n    - c\n      - d\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(annotations.len(), 4);
        assert_eq!(
            annotations[3].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_3.into()),
            }
        );
    }

    #[test]
    fn test_list_mixed_markers_nested() {
        let content = "* outer\n  + middle\n    - inner\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(annotations.len(), 3);
        // Depth 0, 1, 2 regardless of marker character
        assert_eq!(
            annotations[0].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_0.into()),
            }
        );
        assert_eq!(
            annotations[1].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_1.into()),
            }
        );
        assert_eq!(
            annotations[2].kind,
            AnnotationKind::Conceal {
                replacement: Some(BULLET_DEPTH_2.into()),
            }
        );
    }

    #[test]
    fn test_list_provider_caching() {
        let content = "- item\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);

        let a1 = provider.decorations(&tree, content, 0..content.len());
        let a2 = provider.decorations(&tree, content, 0..content.len());

        assert_eq!(a1.len(), a2.len());
        assert_eq!(a1[0].kind, a2[0].kind);
    }

    #[test]
    fn test_list_provider_skips_out_of_range() {
        let content = "- item\n\n- second\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);

        // Only query first line
        let annotations = provider.decorations(&tree, content, 0..7);
        assert_eq!(annotations.len(), 1);
    }

    #[test]
    fn test_nesting_depth_flat() {
        let content = "- a\n- b\n";
        let (tree, query) = parse_md(content);
        let provider = ListDecorationProvider::new(query);
        let annotations = provider.decorations(&tree, content, 0..content.len());

        // Both at depth 0
        assert_eq!(annotations.len(), 2);
        for ann in &annotations {
            assert_eq!(
                ann.kind,
                AnnotationKind::Conceal {
                    replacement: Some(BULLET_DEPTH_0.into()),
                }
            );
        }
    }

    #[test]
    fn test_bullet_for_depth_values() {
        assert_eq!(bullet_for_depth(0), "\u{2022} ");
        assert_eq!(bullet_for_depth(1), "\u{25E6} ");
        assert_eq!(bullet_for_depth(2), "\u{25AA} ");
        assert_eq!(bullet_for_depth(3), "\u{25AB} ");
        assert_eq!(bullet_for_depth(10), "\u{25AB} ");
    }
}
