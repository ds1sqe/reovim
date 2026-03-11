//! Layered annotation cache implementation.
//!
//! Stores syntax tokens per buffer across multiple layers, converting byte
//! offsets to positions. Each layer (syntax, LSP, decorations) has its own
//! priority, and queries merge tokens across layers in priority order.

use std::collections::HashMap;

// =============================================================================
// CachedAnnotationKind - Visual effect type
// =============================================================================

/// What an annotation does visually.
///
/// Mirrors the protocol `AnnotationKind` message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CachedAnnotationKind {
    /// Style overlay (the common case for syntax highlighting).
    #[default]
    Highlight,
    /// Conceal the text range, optionally replacing with different text.
    Conceal {
        /// Replacement text (if any).
        replacement: Option<String>,
    },
    /// Background highlight (independent of text style).
    Background,
    /// Virtual text inserted at this position (not in buffer).
    VirtualText {
        /// The virtual text content.
        text: String,
    },
}

// =============================================================================
// CachedToken - Position-based token
// =============================================================================

/// A syntax token with position info (converted from byte offsets).
///
/// Created by converting `TokenSpan` from the protocol.
#[derive(Debug, Clone)]
pub struct CachedToken {
    /// Line number (0-indexed).
    pub line: u32,
    /// Start column (0-indexed, in characters).
    pub start_col: u32,
    /// End column (exclusive, in characters).
    pub end_col: u32,
    /// Token category (e.g., "keyword", "function.builtin").
    pub category: String,
    /// Visual effect kind.
    pub kind: CachedAnnotationKind,
}

// =============================================================================
// TokenSpan - Protocol type (simplified for this module)
// =============================================================================

/// Token span from the protocol (byte-based).
///
/// This mirrors the proto `TokenSpan` message.
#[derive(Debug, Clone)]
pub struct TokenSpan {
    /// Start byte offset.
    pub start_byte: u32,
    /// End byte offset.
    pub end_byte: u32,
    /// Category string (e.g., "keyword").
    pub category: String,
    /// Visual effect kind.
    pub kind: CachedAnnotationKind,
}

// =============================================================================
// LayerCache - Per-layer token storage (internal)
// =============================================================================

/// Per-layer token cache.
///
/// Stores tokens sorted by (line, `start_col`) for efficient line queries.
#[derive(Debug, Default)]
struct LayerCache {
    /// Layer priority (higher = on top).
    priority: u32,
    /// Cached tokens sorted by (line, `start_col`).
    tokens: Vec<CachedToken>,
}

// =============================================================================
// LayeredTokenCache - Per-buffer layered cache
// =============================================================================

/// Per-buffer layered token cache with byte-to-position conversion.
///
/// Supports multiple annotation layers (e.g., syntax, LSP, decorations).
/// Each layer has its own priority. Queries merge tokens across all layers
/// in priority order (lowest first) for `Style.merge()` compositing.
#[derive(Debug, Default)]
pub struct LayeredTokenCache {
    /// Per-layer caches keyed by layer name.
    layers: HashMap<String, LayerCache>,
    /// Line start byte offsets for byte→position conversion.
    /// Index i = byte offset where line i begins.
    line_offsets: Vec<usize>,
    /// Total byte length of content (for validation).
    content_len: usize,
}

impl LayeredTokenCache {
    /// Create a new empty layered token cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild line byte offsets from content.
    ///
    /// Call this when buffer content changes.
    pub fn rebuild_line_offsets(&mut self, content: &str) {
        self.line_offsets.clear();
        self.line_offsets.push(0); // Line 0 starts at byte 0

        for (i, c) in content.char_indices() {
            if c == '\n' {
                self.line_offsets.push(i + 1);
            }
        }

        self.content_len = content.len();
    }

    /// Apply a token update for a specific layer.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer name (e.g., "syntax", "lsp.semantic").
    /// * `priority` - Layer priority (higher = on top).
    /// * `tokens` - Token spans from the protocol (byte offsets).
    /// * `start_line` - Affected range start line.
    /// * `end_line` - Affected range end line.
    /// * `full_refresh` - If true, replace all tokens in this layer.
    /// * `content` - Buffer content for byte→position conversion.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_layer_update(
        &mut self,
        layer: &str,
        priority: u32,
        tokens: &[TokenSpan],
        start_line: u64,
        end_line: u64,
        full_refresh: bool,
        content: &str,
    ) {
        // Rebuild offsets if content changed
        if content.len() != self.content_len {
            self.rebuild_line_offsets(content);
        }

        // Convert tokens first (borrows self immutably via byte_span_to_cached)
        let cached_tokens: Vec<CachedToken> = tokens
            .iter()
            .flat_map(|span| self.byte_span_to_cached(span, content))
            .collect();

        // Then modify the layer (borrows self mutably)
        let layer_cache = self.layers.entry(layer.to_string()).or_default();
        layer_cache.priority = priority;

        if full_refresh {
            layer_cache.tokens.clear();
        } else {
            // Remove tokens in affected range (only in this layer)
            #[allow(clippy::cast_possible_truncation)]
            let start = start_line as u32;
            #[allow(clippy::cast_possible_truncation)]
            let end = end_line as u32;
            layer_cache
                .tokens
                .retain(|t| t.line < start || t.line > end);
        }

        layer_cache.tokens.extend(cached_tokens);

        // Re-sort by (line, start_col)
        layer_cache.tokens.sort_by_key(|t| (t.line, t.start_col));
    }

    /// Apply a token update to the default "syntax" layer.
    ///
    /// Convenience method for backward compatibility.
    pub fn apply_update(
        &mut self,
        tokens: &[TokenSpan],
        start_line: u64,
        end_line: u64,
        full_refresh: bool,
        content: &str,
    ) {
        self.apply_layer_update("syntax", 0, tokens, start_line, end_line, full_refresh, content);
    }

    /// Convert a byte-based token span to position-based cached tokens.
    ///
    /// Multi-line tokens are split into per-line tokens so that each line
    /// gets its own `CachedToken`. This is essential for Background annotations
    /// on multi-line ranges (e.g., code block backgrounds).
    fn byte_span_to_cached(&self, span: &TokenSpan, content: &str) -> Vec<CachedToken> {
        let Some((start_line, start_col)) =
            self.byte_to_position(span.start_byte as usize, content)
        else {
            return Vec::new();
        };
        let Some((end_line, end_col)) = self.byte_to_position(span.end_byte as usize, content)
        else {
            return Vec::new();
        };

        // Single-line token: most common case
        if start_line == end_line {
            return vec![CachedToken {
                line: start_line,
                start_col,
                end_col,
                category: span.category.clone(),
                kind: span.kind.clone(),
            }];
        }

        // Multi-line token: split into per-line tokens
        let mut tokens = Vec::with_capacity((end_line - start_line + 1) as usize);

        // First line: from start_col to end of line
        let first_line_end = self.line_end_col(start_line, content);
        tokens.push(CachedToken {
            line: start_line,
            start_col,
            end_col: first_line_end,
            category: span.category.clone(),
            kind: span.kind.clone(),
        });

        // Middle lines: full line
        for line in (start_line + 1)..end_line {
            let line_end = self.line_end_col(line, content);
            tokens.push(CachedToken {
                line,
                start_col: 0,
                end_col: line_end,
                category: span.category.clone(),
                kind: span.kind.clone(),
            });
        }

        // Last line: from start of line to end_col
        if end_col > 0 {
            tokens.push(CachedToken {
                line: end_line,
                start_col: 0,
                end_col,
                category: span.category.clone(),
                kind: span.kind.clone(),
            });
        }

        tokens
    }

    /// Convert byte offset to (line, col) position.
    fn byte_to_position(&self, byte: usize, content: &str) -> Option<(u32, u32)> {
        if byte > content.len() {
            return None;
        }

        // Binary search for the line containing this byte
        let line_idx = match self.line_offsets.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        let line_start = self.line_offsets.get(line_idx).copied()?;
        let col_bytes = byte - line_start;

        // Convert byte offset within line to character column
        let line_content = content.get(line_start..)?;
        let col = line_content
            .char_indices()
            .take_while(|(i, _)| *i < col_bytes)
            .count();

        #[allow(clippy::cast_possible_truncation)]
        Some((line_idx as u32, col as u32))
    }

    /// Get the character column index at end of a line.
    ///
    /// Returns CHARACTER count (not byte length), consistent with
    /// `byte_to_position` which returns character-based columns.
    fn line_end_col(&self, line: u32, content: &str) -> u32 {
        let line_idx = line as usize;
        let start = self.line_offsets.get(line_idx).copied().unwrap_or(0);
        let end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(content.len());

        // Get the line content (excluding trailing newline)
        let line_content = content.get(start..end).unwrap_or("");
        let trimmed = line_content.strip_suffix('\n').unwrap_or(line_content);

        // Count characters, not bytes
        #[allow(clippy::cast_possible_truncation)]
        {
            trimmed.chars().count() as u32
        }
    }

    /// Get tokens for a specific line, merged across all layers.
    ///
    /// Returns tokens from all layers sorted by layer priority (lowest
    /// first, then alphabetical by name for same-priority layers).
    /// The caller applies `Style.merge()` in order so that higher-priority
    /// layers override lower-priority ones.
    #[must_use]
    pub fn tokens_for_line(&self, line: u32) -> Vec<&CachedToken> {
        let mut sorted_layers: Vec<_> = self.layers.iter().collect();
        sorted_layers.sort_by(|(name_a, layer_a), (name_b, layer_b)| {
            layer_a
                .priority
                .cmp(&layer_b.priority)
                .then_with(|| name_a.cmp(name_b))
        });

        let mut result = Vec::new();
        for (_, layer) in sorted_layers {
            // Since tokens are sorted by (line, start_col), use binary search
            let start = layer.tokens.partition_point(|t| t.line < line);
            let end = layer.tokens.partition_point(|t| t.line <= line);
            result.extend(&layer.tokens[start..end]);
        }
        result
    }

    /// Get decoration tokens for a line (conceal, virtual text, background).
    ///
    /// Returns only non-Highlight tokens, sorted by layer priority.
    #[must_use]
    pub fn decorations_for_line(&self, line: u32) -> Vec<&CachedToken> {
        self.tokens_for_line(line)
            .into_iter()
            .filter(|t| t.kind != CachedAnnotationKind::Highlight)
            .collect()
    }

    /// Check if all layers are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.values().all(|l| l.tokens.is_empty())
    }

    /// Get total token count across all layers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.values().map(|l| l.tokens.len()).sum()
    }

    /// Clear all layers.
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    /// Get the number of active layers.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

// =============================================================================
// AnnotationCacheManager - Multi-buffer management
// =============================================================================

/// Manages layered annotation caches for multiple buffers.
///
/// Provides LRU-style eviction when cache limit is reached.
#[derive(Debug)]
pub struct AnnotationCacheManager {
    /// Per-buffer caches.
    caches: HashMap<u64, LayeredTokenCache>,
    /// Maximum number of buffers to cache.
    max_buffers: usize,
}

impl AnnotationCacheManager {
    /// Create a new cache manager.
    ///
    /// Default limit: 10 buffers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            caches: HashMap::new(),
            max_buffers: 10,
        }
    }

    /// Create a cache manager with custom buffer limit.
    #[must_use]
    pub fn with_max_buffers(max_buffers: usize) -> Self {
        Self {
            caches: HashMap::new(),
            max_buffers,
        }
    }

    /// Get or create a cache for a buffer.
    ///
    /// # Panics
    ///
    /// Panics if the cache map reports non-zero length but has no first key
    /// (structurally impossible for `HashMap`).
    pub fn get_or_create(&mut self, buffer_id: u64) -> &mut LayeredTokenCache {
        // Simple eviction: remove oldest if at limit
        if !self.caches.contains_key(&buffer_id) && self.caches.len() >= self.max_buffers {
            // Remove first entry (arbitrary for now)
            // len >= max_buffers >= 1, so keys().next() is always Some
            let first_id = *self
                .caches
                .keys()
                .next()
                .expect("non-empty after length check");
            self.caches.remove(&first_id);
        }

        self.caches.entry(buffer_id).or_default()
    }

    /// Get a cache for a buffer if it exists.
    #[must_use]
    pub fn get(&self, buffer_id: u64) -> Option<&LayeredTokenCache> {
        self.caches.get(&buffer_id)
    }

    /// Remove a buffer's cache.
    pub fn remove(&mut self, buffer_id: u64) {
        self.caches.remove(&buffer_id);
    }

    /// Apply a token update for a specific layer.
    ///
    /// Empty layer name defaults to "syntax" with priority 0.
    /// This is the main entry point for handling `TokenUpdate` messages.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_token_update(
        &mut self,
        buffer_id: u64,
        tokens: &[TokenSpan],
        start_line: u64,
        end_line: u64,
        full_refresh: bool,
        content: &str,
        layer: &str,
        priority: u32,
    ) {
        let effective_layer = if layer.is_empty() { "syntax" } else { layer };
        let cache = self.get_or_create(buffer_id);
        cache.apply_layer_update(
            effective_layer,
            priority,
            tokens,
            start_line,
            end_line,
            full_refresh,
            content,
        );
    }

    /// Get tokens for a line in a buffer, merged across all layers.
    #[must_use]
    pub fn tokens_for_line(&self, buffer_id: u64, line: u32) -> Vec<&CachedToken> {
        self.caches
            .get(&buffer_id)
            .map(|c| c.tokens_for_line(line))
            .unwrap_or_default()
    }

    /// Clear all caches.
    pub fn clear(&mut self) {
        self.caches.clear();
    }
}

impl Default for AnnotationCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a highlight token span.
    fn span(start: u32, end: u32, cat: &str) -> TokenSpan {
        TokenSpan {
            start_byte: start,
            end_byte: end,
            category: cat.to_string(),
            kind: CachedAnnotationKind::Highlight,
        }
    }

    // =========================================================================
    // CachedAnnotationKind tests
    // =========================================================================

    #[test]
    fn test_cached_annotation_kind_default() {
        assert_eq!(CachedAnnotationKind::default(), CachedAnnotationKind::Highlight);
    }

    #[test]
    fn test_cached_annotation_kind_variants() {
        let highlight = CachedAnnotationKind::Highlight;
        let conceal_none = CachedAnnotationKind::Conceal { replacement: None };
        let conceal_some = CachedAnnotationKind::Conceal {
            replacement: Some("…".to_string()),
        };
        let background = CachedAnnotationKind::Background;
        let virtual_text = CachedAnnotationKind::VirtualText {
            text: "hint".to_string(),
        };

        // All variants are distinct
        assert_ne!(highlight, conceal_none);
        assert_ne!(conceal_none, conceal_some);
        assert_ne!(highlight, background);
        assert_ne!(highlight, virtual_text);

        // Same variant with same data is equal
        assert_eq!(CachedAnnotationKind::Conceal { replacement: None }, conceal_none);
        assert_eq!(
            CachedAnnotationKind::VirtualText {
                text: "hint".to_string()
            },
            virtual_text
        );
    }

    #[test]
    fn test_cached_annotation_kind_clone() {
        let original = CachedAnnotationKind::VirtualText {
            text: "test".to_string(),
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // =========================================================================
    // LayeredTokenCache - Basic tests (single layer)
    // =========================================================================

    #[test]
    fn test_empty_cache() {
        let cache = LayeredTokenCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.layer_count(), 0);
    }

    #[test]
    fn test_rebuild_line_offsets() {
        let mut cache = LayeredTokenCache::new();
        cache.rebuild_line_offsets("hello\nworld\n");
        assert_eq!(cache.line_offsets, vec![0, 6, 12]);
    }

    #[test]
    fn test_rebuild_line_offsets_empty() {
        let mut cache = LayeredTokenCache::new();
        cache.rebuild_line_offsets("");
        assert_eq!(cache.line_offsets, vec![0]);
    }

    #[test]
    fn test_rebuild_line_offsets_single_line() {
        let mut cache = LayeredTokenCache::new();
        cache.rebuild_line_offsets("hello world");
        assert_eq!(cache.line_offsets, vec![0]);
    }

    #[test]
    fn test_byte_to_position_simple() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello\nworld";
        cache.rebuild_line_offsets(content);

        assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));
        assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));
        assert_eq!(cache.byte_to_position(6, content), Some((1, 0)));
        assert_eq!(cache.byte_to_position(10, content), Some((1, 4)));
    }

    #[test]
    fn test_byte_to_position_past_end() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";
        cache.rebuild_line_offsets(content);
        assert!(cache.byte_to_position(100, content).is_none());
    }

    #[test]
    fn test_byte_to_position_at_boundary() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";
        cache.rebuild_line_offsets(content);

        let pos = cache.byte_to_position(5, content);
        assert!(pos.is_some());
        let (line, col) = pos.unwrap();
        assert_eq!(line, 0);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_unicode_byte_to_position() {
        let mut cache = LayeredTokenCache::new();
        let content = "café";
        cache.rebuild_line_offsets(content);

        assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));
        assert_eq!(cache.byte_to_position(1, content), Some((0, 1)));
        assert_eq!(cache.byte_to_position(2, content), Some((0, 2)));
        assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));
    }

    #[test]
    fn test_line_end_col() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello\nworld\n";
        cache.rebuild_line_offsets(content);

        assert_eq!(cache.line_end_col(0, content), 5);
        assert_eq!(cache.line_end_col(1, content), 5);
    }

    #[test]
    fn test_line_end_col_no_trailing_newline() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello\nworld";
        cache.rebuild_line_offsets(content);
        assert_eq!(cache.line_end_col(1, content), 5);
    }

    #[test]
    fn test_line_end_col_empty_trailing_line() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello\n";
        cache.rebuild_line_offsets(content);
        assert_eq!(cache.line_end_col(1, content), 0);
    }

    // =========================================================================
    // LayeredTokenCache - apply_update (backward compat)
    // =========================================================================

    #[test]
    fn test_apply_update_full_refresh() {
        let mut cache = LayeredTokenCache::new();
        let content = "fn main() {}";

        let tokens = vec![span(0, 2, "keyword"), span(3, 7, "function")];

        cache.apply_update(&tokens, 0, 0, true, content);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.layer_count(), 1);

        let line_tokens = cache.tokens_for_line(0);
        assert_eq!(line_tokens.len(), 2);
        assert_eq!(line_tokens[0].category, "keyword");
        assert_eq!(line_tokens[0].start_col, 0);
        assert_eq!(line_tokens[0].end_col, 2);
        assert_eq!(line_tokens[0].kind, CachedAnnotationKind::Highlight);
    }

    #[test]
    fn test_apply_update_incremental() {
        let mut cache = LayeredTokenCache::new();
        let content = "let x = 1;\nlet y = 2;";

        let tokens1 = vec![span(0, 3, "keyword")];
        cache.apply_update(&tokens1, 0, 0, true, content);

        let tokens2 = vec![span(11, 14, "keyword")];
        cache.apply_update(&tokens2, 1, 1, false, content);

        assert_eq!(cache.tokens_for_line(0).len(), 1);
        assert_eq!(cache.tokens_for_line(1).len(), 1);
    }

    #[test]
    fn test_apply_update_incremental_replaces_range() {
        let mut cache = LayeredTokenCache::new();
        let content = "line0\nline1\nline2";

        let tokens = vec![span(0, 5, "a"), span(6, 11, "b"), span(12, 17, "c")];
        cache.apply_update(&tokens, 0, 2, true, content);
        assert_eq!(cache.len(), 3);

        let new_tokens = vec![span(6, 11, "replaced")];
        cache.apply_update(&new_tokens, 1, 1, false, content);

        let line0 = cache.tokens_for_line(0);
        assert_eq!(line0.len(), 1);
        assert_eq!(line0[0].category, "a");

        let line1 = cache.tokens_for_line(1);
        assert_eq!(line1.len(), 1);
        assert_eq!(line1[0].category, "replaced");

        let line2 = cache.tokens_for_line(2);
        assert_eq!(line2.len(), 1);
        assert_eq!(line2[0].category, "c");
    }

    #[test]
    fn test_multi_line_tokens() {
        let mut cache = LayeredTokenCache::new();
        let content = "fn main() {\n    println!(\"Hello\");\n}";

        let tokens = vec![
            span(0, 2, "keyword"),
            span(3, 7, "function"),
            span(16, 24, "function.builtin"),
            span(25, 32, "string"),
        ];

        cache.apply_update(&tokens, 0, 2, true, content);

        let line0 = cache.tokens_for_line(0);
        assert_eq!(line0.len(), 2);
        assert_eq!(line0[0].category, "keyword");
        assert_eq!(line0[1].category, "function");

        let line1 = cache.tokens_for_line(1);
        assert_eq!(line1.len(), 2);
        assert_eq!(line1[0].category, "function.builtin");
        assert_eq!(line1[1].category, "string");

        assert!(cache.tokens_for_line(2).is_empty());
    }

    #[test]
    fn test_multi_line_token_span() {
        let mut cache = LayeredTokenCache::new();
        let content = "line1\nline2\nline3";

        let tokens = vec![TokenSpan {
            start_byte: 0,
            end_byte: 8,
            category: "comment".to_string(),
            kind: CachedAnnotationKind::Highlight,
        }];

        cache.apply_update(&tokens, 0, 2, true, content);
        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].category, "comment");
        assert_eq!(tokens[0].end_col, 5);
    }

    #[test]
    fn test_token_category_hierarchical_lookup() {
        let mut cache = LayeredTokenCache::new();
        let content = "fn main() {}";

        let tokens = vec![
            span(0, 2, "keyword.function"),
            span(3, 7, "function.definition"),
        ];

        cache.apply_update(&tokens, 0, 0, true, content);

        let line_tokens = cache.tokens_for_line(0);
        assert_eq!(line_tokens.len(), 2);
        assert_eq!(line_tokens[0].category, "keyword.function");
        assert_eq!(line_tokens[1].category, "function.definition");
        assert_eq!(line_tokens[0].start_col, 0);
        assert_eq!(line_tokens[0].end_col, 2);
        assert_eq!(line_tokens[1].start_col, 3);
        assert_eq!(line_tokens[1].end_col, 7);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = LayeredTokenCache::new();
        let content = "fn main() {}";
        cache.apply_update(&[span(0, 2, "keyword")], 0, 0, true, content);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.layer_count(), 1);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.layer_count(), 0);
    }

    #[test]
    fn test_byte_span_to_cached_returns_none() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";
        cache.rebuild_line_offsets(content);

        let invalid_span = TokenSpan {
            start_byte: 100,
            end_byte: 200,
            category: "error".to_string(),
            kind: CachedAnnotationKind::Highlight,
        };

        cache.apply_update(&[invalid_span], 0, 0, true, content);
        assert!(cache.tokens_for_line(0).is_empty());
    }

    // =========================================================================
    // LayeredTokenCache - Multi-layer tests
    // =========================================================================

    #[test]
    fn test_multi_layer_merge() {
        let mut cache = LayeredTokenCache::new();
        let content = "let x = 42;";

        // Syntax layer (priority 0)
        cache.apply_layer_update("syntax", 0, &[span(0, 3, "keyword")], 0, 0, true, content);

        // LSP semantic layer (priority 10)
        cache.apply_layer_update(
            "lsp.semantic",
            10,
            &[span(4, 5, "variable.local")],
            0,
            0,
            true,
            content,
        );

        assert_eq!(cache.layer_count(), 2);
        assert_eq!(cache.len(), 2);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 2);
        // Lower priority (syntax) first
        assert_eq!(tokens[0].category, "keyword");
        // Higher priority (lsp.semantic) second
        assert_eq!(tokens[1].category, "variable.local");
    }

    #[test]
    fn test_multi_layer_priority_order() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";

        // Insert high priority first
        cache.apply_layer_update("high", 100, &[span(0, 5, "from_high")], 0, 0, true, content);
        // Insert low priority second
        cache.apply_layer_update("low", 0, &[span(0, 5, "from_low")], 0, 0, true, content);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 2);
        // Low priority comes first regardless of insertion order
        assert_eq!(tokens[0].category, "from_low");
        assert_eq!(tokens[1].category, "from_high");
    }

    #[test]
    fn test_same_priority_alphabetical_order() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";

        // Both layers have same priority, ordered alphabetically
        cache.apply_layer_update("zeta", 0, &[span(0, 5, "from_zeta")], 0, 0, true, content);
        cache.apply_layer_update("alpha", 0, &[span(0, 5, "from_alpha")], 0, 0, true, content);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 2);
        // Alphabetical: alpha before zeta
        assert_eq!(tokens[0].category, "from_alpha");
        assert_eq!(tokens[1].category, "from_zeta");
    }

    #[test]
    fn test_full_refresh_clears_only_layer() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello world";

        // Two layers
        cache.apply_layer_update("syntax", 0, &[span(0, 5, "keyword")], 0, 0, true, content);
        cache.apply_layer_update("lsp", 10, &[span(6, 11, "variable")], 0, 0, true, content);
        assert_eq!(cache.len(), 2);

        // Full refresh of syntax layer only
        cache.apply_layer_update("syntax", 0, &[span(0, 5, "new_keyword")], 0, 0, true, content);

        assert_eq!(cache.len(), 2);
        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 2);
        // Syntax layer was refreshed
        assert_eq!(tokens[0].category, "new_keyword");
        // LSP layer was NOT touched
        assert_eq!(tokens[1].category, "variable");
    }

    #[test]
    fn test_incremental_update_specific_layer() {
        let mut cache = LayeredTokenCache::new();
        let content = "line0\nline1\nline2";

        // Syntax layer on all lines
        let tokens = vec![span(0, 5, "a"), span(6, 11, "b"), span(12, 17, "c")];
        cache.apply_layer_update("syntax", 0, &tokens, 0, 2, true, content);

        // LSP layer on line 1
        cache.apply_layer_update("lsp", 10, &[span(6, 11, "lsp_b")], 0, 2, true, content);

        // Incremental update to syntax layer line 1 only
        cache.apply_layer_update("syntax", 0, &[span(6, 11, "replaced")], 1, 1, false, content);

        // Syntax line 0 preserved
        let line0 = cache.tokens_for_line(0);
        assert_eq!(line0.len(), 1);
        assert_eq!(line0[0].category, "a");

        // Syntax line 1 replaced, LSP line 1 preserved
        let line1 = cache.tokens_for_line(1);
        assert_eq!(line1.len(), 2);
        assert_eq!(line1[0].category, "replaced"); // syntax (priority 0)
        assert_eq!(line1[1].category, "lsp_b"); // lsp (priority 10)

        // Syntax line 2 preserved
        let line2 = cache.tokens_for_line(2);
        assert_eq!(line2.len(), 1);
        assert_eq!(line2[0].category, "c");
    }

    #[test]
    fn test_decorations_for_line() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello world";

        // Syntax highlight
        cache.apply_layer_update("syntax", 0, &[span(0, 5, "keyword")], 0, 0, true, content);

        // Decoration: virtual text
        let vt_span = TokenSpan {
            start_byte: 5,
            end_byte: 5,
            category: "hint".to_string(),
            kind: CachedAnnotationKind::VirtualText {
                text: " -> ()".to_string(),
            },
        };
        cache.apply_layer_update("decorations", 20, &[vt_span], 0, 0, true, content);

        // tokens_for_line returns all
        assert_eq!(cache.tokens_for_line(0).len(), 2);

        // decorations_for_line returns only non-Highlight
        let decorations = cache.decorations_for_line(0);
        assert_eq!(decorations.len(), 1);
        assert_eq!(
            decorations[0].kind,
            CachedAnnotationKind::VirtualText {
                text: " -> ()".to_string()
            }
        );
    }

    #[test]
    fn test_decorations_for_line_empty() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";

        cache.apply_layer_update("syntax", 0, &[span(0, 5, "keyword")], 0, 0, true, content);

        // No decorations when all tokens are Highlight
        assert!(cache.decorations_for_line(0).is_empty());
    }

    #[test]
    fn test_token_span_with_conceal() {
        let mut cache = LayeredTokenCache::new();
        let content = "```rust\ncode\n```";

        let conceal_span = TokenSpan {
            start_byte: 0,
            end_byte: 7,
            category: "markup.raw".to_string(),
            kind: CachedAnnotationKind::Conceal {
                replacement: Some("▍".to_string()),
            },
        };

        cache.apply_layer_update("syntax", 0, &[conceal_span], 0, 0, true, content);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0].kind,
            CachedAnnotationKind::Conceal {
                replacement: Some("▍".to_string())
            }
        );
    }

    #[test]
    fn test_token_span_with_background() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";

        let bg_span = TokenSpan {
            start_byte: 0,
            end_byte: 5,
            category: "search.match".to_string(),
            kind: CachedAnnotationKind::Background,
        };

        cache.apply_layer_update("search", 50, &[bg_span], 0, 0, true, content);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CachedAnnotationKind::Background);
        assert_eq!(tokens[0].category, "search.match");
    }

    #[test]
    fn test_multi_line_background_splits_to_all_lines() {
        let mut cache = LayeredTokenCache::new();
        let content = "line1\nline2\nline3\nline4";

        // Background spanning lines 0-3 (bytes 0..22)
        let bg_span = TokenSpan {
            start_byte: 0,
            end_byte: 22,
            category: "markup.raw.block".to_string(),
            kind: CachedAnnotationKind::Background,
        };

        cache.apply_layer_update("decoration", 5, &[bg_span], 0, 3, true, content);

        // All 4 lines should have a Background token
        for line in 0..4 {
            let tokens = cache.tokens_for_line(line);
            assert!(!tokens.is_empty(), "Line {line} should have a Background token");
            assert_eq!(
                tokens[0].kind,
                CachedAnnotationKind::Background,
                "Line {line} token should be Background"
            );
            assert_eq!(tokens[0].category, "markup.raw.block");
        }

        // Line 0: start_col=0, end_col=5
        let t0 = &cache.tokens_for_line(0)[0];
        assert_eq!((t0.start_col, t0.end_col), (0, 5));

        // Line 1: start_col=0, end_col=5
        let t1 = &cache.tokens_for_line(1)[0];
        assert_eq!((t1.start_col, t1.end_col), (0, 5));

        // Line 3 (last): start_col=0, end_col=4 ("line4" is 4 chars, no trailing newline)
        let t3 = &cache.tokens_for_line(3)[0];
        assert_eq!((t3.start_col, t3.end_col), (0, 4));
    }

    #[test]
    fn test_layer_priority_update() {
        let mut cache = LayeredTokenCache::new();
        let content = "hello";

        // Create layer with priority 0
        cache.apply_layer_update("lsp", 0, &[span(0, 5, "var")], 0, 0, true, content);

        // Update same layer with new priority
        cache.apply_layer_update("lsp", 10, &[span(0, 5, "var")], 0, 0, true, content);

        // Layer count should still be 1
        assert_eq!(cache.layer_count(), 1);
    }

    #[test]
    fn test_tokens_for_line_no_tokens() {
        let cache = LayeredTokenCache::new();
        assert!(cache.tokens_for_line(0).is_empty());
    }

    // =========================================================================
    // AnnotationCacheManager tests
    // =========================================================================

    #[test]
    fn test_manager_basic() {
        let mut manager = AnnotationCacheManager::new();

        let cache = manager.get_or_create(1);
        assert!(cache.is_empty());

        let cache2 = manager.get_or_create(1);
        assert!(cache2.is_empty());

        let _cache3 = manager.get_or_create(2);

        assert!(manager.get(1).is_some());
        assert!(manager.get(2).is_some());
    }

    #[test]
    fn test_manager_eviction() {
        let mut manager = AnnotationCacheManager::with_max_buffers(2);

        manager.get_or_create(1);
        manager.get_or_create(2);
        manager.get_or_create(3);

        let count = [1_u64, 2, 3]
            .iter()
            .filter(|&&id| manager.get(id).is_some())
            .count();
        assert_eq!(count, 2);
        assert!(manager.get(3).is_some());
    }

    #[test]
    fn test_manager_eviction_with_max_one() {
        let mut manager = AnnotationCacheManager::with_max_buffers(1);

        manager.get_or_create(1);
        assert!(manager.get(1).is_some());

        manager.get_or_create(2);
        assert!(manager.get(1).is_none());
        assert!(manager.get(2).is_some());
    }

    #[test]
    fn test_manager_tokens_for_line_empty() {
        let manager = AnnotationCacheManager::new();
        assert!(manager.tokens_for_line(999, 0).is_empty());
    }

    #[test]
    fn test_manager_remove() {
        let mut manager = AnnotationCacheManager::new();
        manager.get_or_create(1);
        assert!(manager.get(1).is_some());

        manager.remove(1);
        assert!(manager.get(1).is_none());
    }

    #[test]
    fn test_manager_clear() {
        let mut manager = AnnotationCacheManager::new();
        manager.get_or_create(1);
        manager.get_or_create(2);
        manager.get_or_create(3);

        manager.clear();
        assert!(manager.get(1).is_none());
        assert!(manager.get(2).is_none());
        assert!(manager.get(3).is_none());
    }

    #[test]
    fn test_manager_apply_token_update() {
        let mut manager = AnnotationCacheManager::new();
        let content = "fn main() {}";
        let tokens = vec![span(0, 2, "keyword")];

        manager.apply_token_update(1, &tokens, 0, 0, true, content, "syntax", 0);

        let line_tokens = manager.tokens_for_line(1, 0);
        assert_eq!(line_tokens.len(), 1);
        assert_eq!(line_tokens[0].category, "keyword");
    }

    #[test]
    fn test_manager_empty_layer_defaults_to_syntax() {
        let mut manager = AnnotationCacheManager::new();
        let content = "hello";

        // Empty layer name should default to "syntax"
        manager.apply_token_update(1, &[span(0, 5, "keyword")], 0, 0, true, content, "", 0);

        let cache = manager.get(1).unwrap();
        assert_eq!(cache.layer_count(), 1);

        let tokens = cache.tokens_for_line(0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].category, "keyword");
    }

    #[test]
    fn test_manager_eviction_at_limit() {
        let mut manager = AnnotationCacheManager::with_max_buffers(2);
        manager.get_or_create(1);
        manager.get_or_create(2);

        manager.get_or_create(3);

        let count = [1u64, 2, 3]
            .iter()
            .filter(|&&id| manager.get(id).is_some())
            .count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_manager_default() {
        let manager = AnnotationCacheManager::default();
        assert!(manager.get(999).is_none());
    }

    #[test]
    fn test_manager_with_max_buffers() {
        let manager = AnnotationCacheManager::with_max_buffers(5);
        assert!(manager.get(0).is_none());
    }

    #[test]
    fn test_manager_get_or_create_existing_at_limit() {
        let mut manager = AnnotationCacheManager::with_max_buffers(2);
        manager.get_or_create(1);
        manager.get_or_create(2);

        // At limit. Accessing existing buffer should NOT trigger eviction.
        manager.get_or_create(1);
        assert!(manager.get(1).is_some());
        assert!(manager.get(2).is_some());
    }
}
