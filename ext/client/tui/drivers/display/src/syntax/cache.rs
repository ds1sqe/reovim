//! Layered annotation cache implementation.
//!
//! Stores syntax tokens per buffer across multiple layers, converting byte
//! offsets to positions. Each layer (syntax, LSP, decorations) has its own
//! priority, and queries merge tokens across layers in priority order.

use std::collections::HashMap;

// =============================================================================
// CachedToken - Position-based token
// =============================================================================

/// A syntax token with position info (converted from byte offsets).
///
/// Created by converting `TokenSpan` from the protocol.
/// The client's `render_behavior()` function decides visual rendering
/// based on the category string (mechanism vs policy separation).
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
        });

        // Middle lines: full line
        for line in (start_line + 1)..end_line {
            let line_end = self.line_end_col(line, content);
            tokens.push(CachedToken {
                line,
                start_col: 0,
                end_col: line_end,
                category: span.category.clone(),
            });
        }

        // Last line: from start of line to end_col
        if end_col > 0 {
            tokens.push(CachedToken {
                line: end_line,
                start_col: 0,
                end_col,
                category: span.category.clone(),
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
#[path = "cache_tests.rs"]
mod tests;
