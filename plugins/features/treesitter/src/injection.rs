//! Tree-sitter language injection support
//!
//! This module provides support for language injections - embedding one language
//! inside another (e.g., code blocks in markdown, inline scripts in HTML).
//!
//! # Architecture
//!
//! The injection system works in three layers:
//! 1. `InjectionDetector` - analyzes parent tree to find injection regions
//! 2. `InjectionRegion` - describes a region with its target language
//! 3. `InjectionLayer` - manages parsing and highlighting for an injected language

use std::{collections::HashMap, ops::Range, sync::Arc};

use {
    reovim_core::highlight::Highlight,
    tree_sitter::{Parser, Query, QueryCursor, StreamingIterator, Tree},
};

use crate::{highlighter::Highlighter, queries::QueryType, state::SharedTreesitterManager};

/// Describes a region where a different language should be highlighted
#[derive(Debug, Clone)]
pub struct InjectionRegion {
    /// The language ID to use for highlighting
    pub language_id: String,
    /// Byte range in the source document
    pub byte_range: Range<usize>,
    /// Start row (0-indexed)
    pub start_row: u32,
    /// End row (0-indexed, inclusive)
    pub end_row: u32,
    /// Start column (0-indexed)
    pub start_col: u32,
    /// End column (0-indexed)
    pub end_col: u32,
}

impl InjectionRegion {
    /// Check if this region overlaps with a line range
    #[must_use]
    pub fn overlaps_lines(&self, start_line: u32, end_line: u32) -> bool {
        self.start_row <= end_line && self.end_row >= start_line
    }
}

/// Detects injection points from a parsed tree
///
/// Uses the parent language's injection query to identify regions
/// that should be highlighted with a different language.
pub struct InjectionDetector {
    /// Compiled injection query
    query: Arc<Query>,
    /// Capture index for injection content
    content_capture_idx: u32,
    /// Capture index for injection language (if captured from tree)
    language_capture_idx: Option<u32>,
}

impl InjectionDetector {
    /// Create a new injection detector from a compiled query
    ///
    /// Returns None if the query doesn't have the required captures.
    #[must_use]
    pub fn new(query: Arc<Query>) -> Option<Self> {
        // Find the required capture indices
        let mut content_idx = None;
        let mut language_idx = None;

        for (idx, name) in query.capture_names().iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            match *name {
                "injection.content" => content_idx = Some(idx as u32),
                "injection.language" => language_idx = Some(idx as u32),
                _ => {}
            }
        }

        let content_capture_idx = content_idx?;

        Some(Self {
            query,
            content_capture_idx,
            language_capture_idx: language_idx,
        })
    }

    /// Detect injection regions from a parse tree
    ///
    /// Returns ALL detected regions regardless of whether the language is registered.
    /// Unregistered languages will be skipped during highlighting (layer creation fails gracefully).
    /// When the language registers later, a re-render will retry layer creation.
    #[allow(clippy::cast_possible_truncation)]
    pub fn detect(&self, tree: &Tree, content: &str) -> Vec<InjectionRegion> {
        let mut regions = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), content.as_bytes());

        tracing::debug!(
            "InjectionDetector::detect: starting, content_capture_idx={}, language_capture_idx={:?}",
            self.content_capture_idx,
            self.language_capture_idx
        );

        while let Some(match_) = matches.next() {
            tracing::debug!(
                "InjectionDetector::detect: found match with {} captures",
                match_.captures.len()
            );
            let mut content_node = None;
            let mut language_id: Option<String> = None;

            // Extract content and language from captures
            for capture in match_.captures {
                if capture.index == self.content_capture_idx {
                    content_node = Some(capture.node);
                } else if Some(capture.index) == self.language_capture_idx {
                    // Language is captured from the tree (e.g., info string in fenced code)
                    let lang_text = capture
                        .node
                        .utf8_text(content.as_bytes())
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    if !lang_text.is_empty() {
                        language_id = Some(lang_text);
                    }
                }
            }

            // Check for #set! predicates that set injection.language
            if language_id.is_none() {
                for prop in self.query.property_settings(match_.pattern_index) {
                    if &*prop.key == "injection.language"
                        && let Some(value) = &prop.value
                    {
                        language_id = Some(value.to_string());
                    }
                }
            }

            // If we have both content and a valid language, create a region
            // NOTE: We don't filter by registry here - unregistered languages are stored
            // and will be skipped during highlighting (get_or_create_layer returns None)
            if let (Some(node), Some(lang)) = (content_node, language_id) {
                let start = node.start_position();
                let end = node.end_position();

                tracing::debug!(
                    language = %lang,
                    start_row = start.row,
                    end_row = end.row,
                    "Detected injection region"
                );

                regions.push(InjectionRegion {
                    language_id: lang,
                    byte_range: node.start_byte()..node.end_byte(),
                    start_row: start.row as u32,
                    end_row: end.row as u32,
                    start_col: start.column as u32,
                    end_col: end.column as u32,
                });
            }
        }

        // Sort by start position
        regions.sort_by_key(|r| (r.start_row, r.start_col));
        regions
    }
}

/// Manages parsing and highlighting for an injected language
///
/// Each injection layer has its own parser and can produce highlights
/// for its injected regions.
pub struct InjectionLayer {
    /// Language identifier
    language_id: String,
    /// Tree-sitter parser
    parser: Parser,
    /// Cached parse tree (keyed by byte range string for simplicity)
    trees: HashMap<String, Tree>,
    /// Highlights query for this language
    highlights_query: Option<Arc<Query>>,
}

impl InjectionLayer {
    /// Create a new injection layer for a language
    ///
    /// Returns None if the parser can't be set up.
    pub fn new(
        language_id: &str,
        ts_language: &tree_sitter::Language,
        highlights_query: Option<Arc<Query>>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(ts_language).ok()?;

        Some(Self {
            language_id: language_id.to_string(),
            parser,
            trees: HashMap::new(),
            highlights_query,
        })
    }

    /// Highlight a region of content
    ///
    /// Parses the region content and returns highlights with adjusted positions.
    #[allow(clippy::cast_possible_truncation)]
    pub fn highlight_region(
        &mut self,
        region: &InjectionRegion,
        full_content: &str,
        highlighter: &Highlighter,
    ) -> Vec<Highlight> {
        let Some(query) = &self.highlights_query else {
            return Vec::new();
        };

        // Extract the region content
        let region_content = &full_content[region.byte_range.clone()];

        // Use byte range as cache key
        let cache_key = format!("{}:{}", region.byte_range.start, region.byte_range.end);

        // Parse or get cached tree
        let tree = self.trees.entry(cache_key).or_insert_with(|| {
            self.parser
                .parse(region_content, None)
                .unwrap_or_else(|| self.parser.parse("", None).unwrap())
        });

        // Generate highlights (0-indexed within the region)
        let region_lines = region.end_row - region.start_row + 1;
        let mut highlights =
            highlighter.highlight_range(tree, query, region_content, 0, region_lines);

        // Adjust positions to be relative to the full document
        for highlight in &mut highlights {
            // Adjust line numbers
            highlight.span.start_line += region.start_row;
            highlight.span.end_line += region.start_row;

            // For the first line of the region, adjust column offset
            if highlight.span.start_line == region.start_row {
                highlight.span.start_col += region.start_col;
            }
            if highlight.span.end_line == region.start_row {
                highlight.span.end_col += region.start_col;
            }
        }

        highlights
    }

    /// Clear the parse cache
    pub fn clear_cache(&mut self) {
        self.trees.clear();
    }

    /// Get the language ID
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }
}

/// Manages injection detection and highlighting for a syntax provider
///
/// This is the main interface used by `TreeSitterSyntax` to handle injections.
pub struct InjectionManager {
    /// Injection detector for the parent language
    detector: Option<InjectionDetector>,
    /// Cached injection layers by language ID
    layers: HashMap<String, InjectionLayer>,
    /// Cached injection regions (invalidated on reparse)
    regions: Vec<InjectionRegion>,
    /// Whether regions need to be recomputed
    regions_dirty: bool,
}

impl InjectionManager {
    /// Create a new injection manager
    ///
    /// # Arguments
    /// * `injection_query` - Optional compiled injection query for the parent language
    #[must_use]
    pub fn new(injection_query: Option<Arc<Query>>) -> Self {
        let detector = injection_query.and_then(InjectionDetector::new);

        Self {
            detector,
            layers: HashMap::new(),
            regions: Vec::new(),
            regions_dirty: true,
        }
    }

    /// Check if injections are supported (detector is configured)
    #[must_use]
    pub fn has_detector(&self) -> bool {
        self.detector.is_some()
    }

    /// Mark regions as dirty (call after parent tree changes)
    pub fn invalidate(&mut self) {
        self.regions_dirty = true;
        // Clear layer caches too since content may have changed
        for layer in self.layers.values_mut() {
            layer.clear_cache();
        }
    }

    /// Eagerly saturate the injection system
    ///
    /// Pre-computes all injection regions and pre-creates all needed layers.
    /// Call this during parse() to avoid lazy initialization during highlight_range().
    /// This reduces lock contention by doing all mutation work upfront.
    pub fn saturate(&mut self, tree: &Tree, content: &str, manager: &SharedTreesitterManager) {
        if !self.has_detector() {
            return;
        }

        // Detect regions if dirty
        if self.regions_dirty {
            if let Some(detector) = &self.detector {
                self.regions = detector.detect(tree, content);
            } else {
                self.regions.clear();
            }
            self.regions_dirty = false;
        }

        // Pre-create layers for all detected languages
        let language_ids: Vec<String> = self
            .regions
            .iter()
            .map(|r| r.language_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for language_id in language_ids {
            // This creates the layer if it doesn't exist
            let _ = self.get_or_create_layer(&language_id, manager);
        }
    }

    /// Get or create an injection layer for a language
    fn get_or_create_layer(
        &mut self,
        language_id: &str,
        manager: &SharedTreesitterManager,
    ) -> Option<&mut InjectionLayer> {
        if !self.layers.contains_key(language_id) {
            // Get the language and highlights query from manager
            // Both must be available for injection highlighting to work
            let result = manager.with(|m| {
                let registered = m.registry().get(language_id);
                if registered.is_none() {
                    tracing::debug!(
                        language_id = %language_id,
                        "Injection layer skipped: language not registered"
                    );
                    return None;
                }
                let registered = registered.unwrap();
                let query = m.query_cache().get(language_id, QueryType::Highlights);
                if query.is_none() {
                    tracing::debug!(
                        language_id = %language_id,
                        "Injection layer skipped: highlights query not cached"
                    );
                    return None;
                }
                Some((registered.language().clone(), query))
            });

            let (ts_language, highlights_query) = result?;

            let layer = InjectionLayer::new(language_id, &ts_language, highlights_query)?;
            tracing::debug!(language_id = %language_id, "Created injection layer");
            self.layers.insert(language_id.to_string(), layer);
        }

        self.layers.get_mut(language_id)
    }

    /// Highlight injection regions that overlap the given line range
    ///
    /// Returns additional highlights from injected languages to be merged
    /// with the parent language's highlights.
    pub fn highlight_injections(
        &mut self,
        tree: &Tree,
        content: &str,
        start_line: u32,
        end_line: u32,
        manager: &SharedTreesitterManager,
        highlighter: &Highlighter,
    ) -> Vec<Highlight> {
        tracing::debug!(
            "InjectionManager::highlight_injections: dirty={}, has_detector={}, start_line={}, end_line={}",
            self.regions_dirty,
            self.detector.is_some(),
            start_line,
            end_line
        );

        // Update regions if dirty (need registry access)
        if self.regions_dirty {
            if let Some(detector) = &self.detector {
                self.regions = detector.detect(tree, content);
                tracing::debug!(
                    "InjectionManager::highlight_injections: detected {} regions",
                    self.regions.len()
                );
            } else {
                self.regions.clear();
            }
            self.regions_dirty = false;
        }

        let mut highlights = Vec::new();

        // Collect regions that overlap the requested line range (avoid borrow issues)
        let overlapping_regions: Vec<InjectionRegion> = self
            .regions
            .iter()
            .filter(|r| r.overlaps_lines(start_line, end_line))
            .cloned()
            .collect();

        // Process each region
        for region in &overlapping_regions {
            // Get or create layer for this language
            if let Some(layer) = self.get_or_create_layer(&region.language_id, manager) {
                let region_highlights = layer.highlight_region(region, content, highlighter);
                highlights.extend(region_highlights);
            }
        }

        highlights
    }

    /// Get the current injection regions
    #[must_use]
    pub fn regions(&self) -> &[InjectionRegion] {
        &self.regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_injection_region_overlaps() {
        let region = InjectionRegion {
            language_id: "rust".to_string(),
            byte_range: 0..100,
            start_row: 5,
            end_row: 10,
            start_col: 0,
            end_col: 3,
        };

        // Fully before
        assert!(!region.overlaps_lines(0, 4));
        // Fully after
        assert!(!region.overlaps_lines(11, 20));
        // Overlaps start
        assert!(region.overlaps_lines(3, 6));
        // Overlaps end
        assert!(region.overlaps_lines(9, 15));
        // Fully contained
        assert!(region.overlaps_lines(6, 8));
        // Contains region
        assert!(region.overlaps_lines(0, 20));
    }
}
