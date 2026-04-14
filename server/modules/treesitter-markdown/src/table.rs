//! Semantic table annotations.
//!
//! Implements [`DecorationProvider`] for pipe tables: emits semantic
//! byte-range annotations for the table extent, pipe positions, and
//! delimiter row. No visual rendering — the client decides how to display.
//!
//! # Categories emitted
//!
//! | Category | Scope |
//! |----------|-------|
//! | `markup.table` | Whole table node |
//! | `markup.table.pipe` | Each `\|` byte in header/data rows |
//! | `markup.table.delimiter` | Entire delimiter row |

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

/// Decoration provider that emits semantic annotations for pipe tables.
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
    #[cfg_attr(coverage_nightly, coverage(off))] // RwLock poison + MC/DC cache-find branches unreachable
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

                // Cache miss: generate semantic annotations
                let annotations = table_semantic_annotations(node, content);

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

// ── Semantic annotation generation ──

/// Generate semantic annotations from a `pipe_table` node.
///
/// Emits:
/// 1. `markup.table` for the whole table extent
/// 2. `markup.table.pipe` for each `|` byte in header/data rows
/// 3. `markup.table.delimiter` for the entire delimiter row
#[cfg_attr(coverage_nightly, coverage(off))]
fn table_semantic_annotations(table_node: Node, content: &str) -> Vec<Annotation> {
    let mut annotations = Vec::new();

    // Mark whole table
    annotations.push(Annotation::new(
        table_node.start_byte(),
        table_node.end_byte(),
        HighlightCategory::new("markup.table"),
    ));

    // Walk children for pipes and delimiter
    let mut cursor = table_node.walk();
    for child in table_node.children(&mut cursor) {
        match child.kind() {
            "pipe_table_header" | "pipe_table_row" => {
                let row_start = child.start_byte();
                let row_end = child.end_byte();
                if row_end <= content.len() {
                    let row_text = &content[row_start..row_end];
                    for (offset, byte) in row_text.bytes().enumerate() {
                        if byte == b'|' {
                            let pos = row_start + offset;
                            annotations.push(Annotation::new(
                                pos,
                                pos + 1,
                                HighlightCategory::new("markup.table.pipe"),
                            ));
                        }
                    }
                }
            }
            "pipe_table_delimiter_row" => {
                annotations.push(Annotation::new(
                    child.start_byte(),
                    child.end_byte(),
                    HighlightCategory::new("markup.table.delimiter"),
                ));
            }
            _ => {}
        }
    }

    annotations
}

// ── Tests ──

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
