//! Token cache implementation.
//!
//! Stores syntax tokens per buffer and converts byte offsets to positions.

use std::collections::HashMap;

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
}

// =============================================================================
// TokenCache - Per-buffer cache
// =============================================================================

/// Per-buffer token cache with byte-to-position conversion.
///
/// Stores tokens sorted by (line, `start_col`) for efficient line queries.
#[derive(Debug, Default)]
pub struct TokenCache {
    /// Cached tokens sorted by (line, `start_col`).
    tokens: Vec<CachedToken>,
    /// Line start byte offsets for byte→position conversion.
    /// Index i = byte offset where line i begins.
    line_offsets: Vec<usize>,
    /// Total byte length of content (for validation).
    content_len: usize,
}

impl TokenCache {
    /// Create a new empty token cache.
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

    /// Apply a token update from the server.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Token spans from the protocol (byte offsets).
    /// * `start_line` - Affected range start line.
    /// * `end_line` - Affected range end line.
    /// * `full_refresh` - If true, replace all tokens.
    /// * `content` - Buffer content for byte→position conversion.
    pub fn apply_update(
        &mut self,
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

        if full_refresh {
            self.tokens.clear();
        } else {
            // Remove tokens in affected range
            #[allow(clippy::cast_possible_truncation)]
            let start = start_line as u32;
            #[allow(clippy::cast_possible_truncation)]
            let end = end_line as u32;
            self.tokens.retain(|t| t.line < start || t.line > end);
        }

        // Convert and insert new tokens
        for span in tokens {
            if let Some(token) = self.byte_span_to_cached(span, content) {
                self.tokens.push(token);
            }
        }

        // Re-sort by (line, start_col)
        self.tokens.sort_by_key(|t| (t.line, t.start_col));
    }

    /// Convert a byte-based token span to a position-based cached token.
    fn byte_span_to_cached(&self, span: &TokenSpan, content: &str) -> Option<CachedToken> {
        let (start_line, start_col) = self.byte_to_position(span.start_byte as usize, content)?;
        let (end_line, end_col) = self.byte_to_position(span.end_byte as usize, content)?;

        // For now, only handle single-line tokens
        // Multi-line tokens will be split at line boundaries in a future enhancement
        if start_line != end_line {
            // Return token that ends at end of first line
            let line_end = self.line_end_col(start_line, content);
            return Some(CachedToken {
                line: start_line,
                start_col,
                end_col: line_end,
                category: span.category.clone(),
            });
        }

        Some(CachedToken {
            line: start_line,
            start_col,
            end_col,
            category: span.category.clone(),
        })
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

    /// Get the column index at end of a line.
    fn line_end_col(&self, line: u32, content: &str) -> u32 {
        let line_idx = line as usize;
        let start = self.line_offsets.get(line_idx).copied().unwrap_or(0);
        let end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(content.len());

        // Exclude trailing newline
        let line_len = end.saturating_sub(start);
        let adjusted = if line_len > 0 && content.get(start..end).is_some_and(|s| s.ends_with('\n'))
        {
            line_len - 1
        } else {
            line_len
        };

        #[allow(clippy::cast_possible_truncation)]
        {
            adjusted as u32
        }
    }

    /// Get tokens for a specific line.
    ///
    /// Returns tokens sorted by start column.
    pub fn tokens_for_line(&self, line: u32) -> impl Iterator<Item = &CachedToken> {
        // Since tokens are sorted by (line, start_col), we can use binary search
        let start = self.tokens.partition_point(|t| t.line < line);
        let end = self.tokens.partition_point(|t| t.line <= line);
        self.tokens[start..end].iter()
    }

    /// Get all tokens.
    #[must_use]
    pub fn tokens(&self) -> &[CachedToken] {
        &self.tokens
    }

    /// Check if cache is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Get the number of cached tokens.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Clear all tokens.
    pub fn clear(&mut self) {
        self.tokens.clear();
    }
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
// TokenCacheManager - Multi-buffer management
// =============================================================================

/// Manages token caches for multiple buffers.
///
/// Provides LRU-style eviction when cache limit is reached.
#[derive(Debug)]
pub struct TokenCacheManager {
    /// Per-buffer caches.
    caches: HashMap<u64, TokenCache>,
    /// Maximum number of buffers to cache.
    max_buffers: usize,
}

impl TokenCacheManager {
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
    pub fn get_or_create(&mut self, buffer_id: u64) -> &mut TokenCache {
        // Simple eviction: remove oldest if at limit
        // TODO: Implement proper LRU eviction
        if !self.caches.contains_key(&buffer_id) && self.caches.len() >= self.max_buffers {
            // Remove first entry (arbitrary for now)
            if let Some(&first_id) = self.caches.keys().next() {
                self.caches.remove(&first_id);
            }
        }

        self.caches.entry(buffer_id).or_default()
    }

    /// Get a cache for a buffer if it exists.
    #[must_use]
    pub fn get(&self, buffer_id: u64) -> Option<&TokenCache> {
        self.caches.get(&buffer_id)
    }

    /// Remove a buffer's cache.
    pub fn remove(&mut self, buffer_id: u64) {
        self.caches.remove(&buffer_id);
    }

    /// Apply a token update from the protocol.
    ///
    /// This is the main entry point for handling `TokenUpdate` messages.
    pub fn apply_token_update(
        &mut self,
        buffer_id: u64,
        tokens: &[TokenSpan],
        start_line: u64,
        end_line: u64,
        full_refresh: bool,
        content: &str,
    ) {
        let cache = self.get_or_create(buffer_id);
        cache.apply_update(tokens, start_line, end_line, full_refresh, content);
    }

    /// Get tokens for a line in a buffer.
    pub fn tokens_for_line(&self, buffer_id: u64, line: u32) -> impl Iterator<Item = &CachedToken> {
        self.caches
            .get(&buffer_id)
            .into_iter()
            .flat_map(move |c| c.tokens_for_line(line))
    }

    /// Clear all caches.
    pub fn clear(&mut self) {
        self.caches.clear();
    }
}

impl Default for TokenCacheManager {
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

    #[test]
    fn test_token_cache_empty() {
        let cache = TokenCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_rebuild_line_offsets() {
        let mut cache = TokenCache::new();
        cache.rebuild_line_offsets("hello\nworld\n");

        assert_eq!(cache.line_offsets, vec![0, 6, 12]);
    }

    #[test]
    fn test_byte_to_position_simple() {
        let mut cache = TokenCache::new();
        let content = "hello\nworld";
        cache.rebuild_line_offsets(content);

        // 'h' at byte 0 -> (0, 0)
        assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));

        // 'l' at byte 3 -> (0, 3)
        assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));

        // 'w' at byte 6 -> (1, 0)
        assert_eq!(cache.byte_to_position(6, content), Some((1, 0)));

        // 'd' at byte 10 -> (1, 4)
        assert_eq!(cache.byte_to_position(10, content), Some((1, 4)));
    }

    #[test]
    fn test_apply_update_full_refresh() {
        let mut cache = TokenCache::new();
        let content = "fn main() {}";

        let tokens = vec![
            TokenSpan {
                start_byte: 0,
                end_byte: 2,
                category: "keyword".to_string(),
            },
            TokenSpan {
                start_byte: 3,
                end_byte: 7,
                category: "function".to_string(),
            },
        ];

        cache.apply_update(&tokens, 0, 0, true, content);

        assert_eq!(cache.len(), 2);

        let line_tokens: Vec<_> = cache.tokens_for_line(0).collect();
        assert_eq!(line_tokens.len(), 2);
        assert_eq!(line_tokens[0].category, "keyword");
        assert_eq!(line_tokens[0].start_col, 0);
        assert_eq!(line_tokens[0].end_col, 2);
    }

    #[test]
    fn test_apply_update_incremental() {
        let mut cache = TokenCache::new();
        let content = "let x = 1;\nlet y = 2;";

        // First update - line 0
        let tokens1 = vec![TokenSpan {
            start_byte: 0,
            end_byte: 3,
            category: "keyword".to_string(),
        }];
        cache.apply_update(&tokens1, 0, 0, true, content);

        // Second update - line 1 (incremental)
        let tokens2 = vec![TokenSpan {
            start_byte: 11,
            end_byte: 14,
            category: "keyword".to_string(),
        }];
        cache.apply_update(&tokens2, 1, 1, false, content);

        // Should have tokens from both lines
        assert_eq!(cache.tokens_for_line(0).count(), 1);
        assert_eq!(cache.tokens_for_line(1).count(), 1);
    }

    #[test]
    fn test_cache_manager_basic() {
        let mut manager = TokenCacheManager::new();

        let cache = manager.get_or_create(1);
        assert!(cache.is_empty());

        // Get same buffer again
        let cache2 = manager.get_or_create(1);
        assert!(cache2.is_empty());

        // Different buffer
        let _cache3 = manager.get_or_create(2);

        // Should have 2 caches
        assert!(manager.get(1).is_some());
        assert!(manager.get(2).is_some());
    }

    #[test]
    fn test_cache_manager_eviction() {
        let mut manager = TokenCacheManager::with_max_buffers(2);

        manager.get_or_create(1);
        manager.get_or_create(2);
        manager.get_or_create(3); // Should evict one

        // Only 2 buffers should remain
        let mut count = 0;
        if manager.get(1).is_some() {
            count += 1;
        }
        if manager.get(2).is_some() {
            count += 1;
        }
        if manager.get(3).is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_tokens_for_line_empty() {
        let manager = TokenCacheManager::new();
        assert!(manager.tokens_for_line(999, 0).next().is_none());
    }

    #[test]
    fn test_unicode_byte_to_position() {
        let mut cache = TokenCache::new();
        // "café" has 5 bytes but 4 characters
        let content = "café";
        cache.rebuild_line_offsets(content);

        // 'c' at byte 0 -> (0, 0)
        assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));

        // 'a' at byte 1 -> (0, 1)
        assert_eq!(cache.byte_to_position(1, content), Some((0, 1)));

        // 'f' at byte 2 -> (0, 2)
        assert_eq!(cache.byte_to_position(2, content), Some((0, 2)));

        // 'é' starts at byte 3, ends at byte 5 (2-byte UTF-8)
        // byte 3 -> (0, 3)
        assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));
    }

    #[test]
    fn test_multi_line_tokens() {
        let mut cache = TokenCache::new();
        let content = "fn main() {\n    println!(\"Hello\");\n}";

        let tokens = vec![
            TokenSpan {
                start_byte: 0,
                end_byte: 2,
                category: "keyword".to_string(),
            },
            TokenSpan {
                start_byte: 3,
                end_byte: 7,
                category: "function".to_string(),
            },
            TokenSpan {
                start_byte: 16,
                end_byte: 24,
                category: "function.builtin".to_string(),
            },
            TokenSpan {
                start_byte: 25,
                end_byte: 32,
                category: "string".to_string(),
            },
        ];

        cache.apply_update(&tokens, 0, 2, true, content);

        // Line 0: fn main()
        let line0: Vec<_> = cache.tokens_for_line(0).collect();
        assert_eq!(line0.len(), 2);
        assert_eq!(line0[0].category, "keyword");
        assert_eq!(line0[1].category, "function");

        // Line 1: println!("Hello");
        let line1: Vec<_> = cache.tokens_for_line(1).collect();
        assert_eq!(line1.len(), 2);
        assert_eq!(line1[0].category, "function.builtin");
        assert_eq!(line1[1].category, "string");

        // Line 2: } - no tokens
        assert!(cache.tokens_for_line(2).next().is_none());
    }

    #[test]
    fn test_token_category_hierarchical_lookup() {
        // This tests that token categories like "function.builtin" work with
        // ThemeManager's hierarchical fallback (function.builtin → function → default)
        let mut cache = TokenCache::new();
        let content = "fn main() {}";

        let tokens = vec![
            TokenSpan {
                start_byte: 0,
                end_byte: 2,
                category: "keyword.function".to_string(),
            },
            TokenSpan {
                start_byte: 3,
                end_byte: 7,
                category: "function.definition".to_string(),
            },
        ];

        cache.apply_update(&tokens, 0, 0, true, content);

        let line_tokens: Vec<_> = cache.tokens_for_line(0).collect();
        assert_eq!(line_tokens.len(), 2);

        // Categories preserve their hierarchy
        assert_eq!(line_tokens[0].category, "keyword.function");
        assert_eq!(line_tokens[1].category, "function.definition");

        // Verify column positions are correct
        assert_eq!(line_tokens[0].start_col, 0);
        assert_eq!(line_tokens[0].end_col, 2);
        assert_eq!(line_tokens[1].start_col, 3);
        assert_eq!(line_tokens[1].end_col, 7);
    }

    // =========================================================================
    // TokenCache extended tests
    // =========================================================================

    #[test]
    fn test_token_cache_clear() {
        let mut cache = TokenCache::new();
        let content = "fn main() {}";
        let tokens = vec![TokenSpan {
            start_byte: 0,
            end_byte: 2,
            category: "keyword".to_string(),
        }];
        cache.apply_update(&tokens, 0, 0, true, content);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_token_cache_tokens_accessor() {
        let mut cache = TokenCache::new();
        let content = "fn main() {}";
        let tokens = vec![
            TokenSpan {
                start_byte: 0,
                end_byte: 2,
                category: "keyword".to_string(),
            },
            TokenSpan {
                start_byte: 3,
                end_byte: 7,
                category: "function".to_string(),
            },
        ];
        cache.apply_update(&tokens, 0, 0, true, content);

        let all = cache.tokens();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].category, "keyword");
        assert_eq!(all[1].category, "function");
    }

    #[test]
    fn test_byte_to_position_past_end() {
        let mut cache = TokenCache::new();
        let content = "hello";
        cache.rebuild_line_offsets(content);

        // Past end of content
        assert!(cache.byte_to_position(100, content).is_none());
    }

    #[test]
    fn test_byte_to_position_at_boundary() {
        let mut cache = TokenCache::new();
        let content = "hello";
        cache.rebuild_line_offsets(content);

        // Exactly at end
        let pos = cache.byte_to_position(5, content);
        assert!(pos.is_some());
        let (line, col) = pos.unwrap();
        assert_eq!(line, 0);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_multi_line_token_span() {
        let mut cache = TokenCache::new();
        let content = "let x =\n    42;";

        // Token spanning lines 0-1
        let tokens = vec![TokenSpan {
            start_byte: 0,
            end_byte: 14,
            category: "expression".to_string(),
        }];

        cache.apply_update(&tokens, 0, 1, true, content);

        // Multi-line tokens get truncated to first line
        let line0: Vec<_> = cache.tokens_for_line(0).collect();
        assert_eq!(line0.len(), 1);
        assert_eq!(line0[0].line, 0);
    }

    #[test]
    fn test_apply_update_incremental_replaces_range() {
        let mut cache = TokenCache::new();
        let content = "line0\nline1\nline2";

        // Full refresh with tokens on all lines
        let tokens = vec![
            TokenSpan {
                start_byte: 0,
                end_byte: 5,
                category: "a".to_string(),
            },
            TokenSpan {
                start_byte: 6,
                end_byte: 11,
                category: "b".to_string(),
            },
            TokenSpan {
                start_byte: 12,
                end_byte: 17,
                category: "c".to_string(),
            },
        ];
        cache.apply_update(&tokens, 0, 2, true, content);
        assert_eq!(cache.len(), 3);

        // Incremental update: replace tokens on line 1 only
        let new_tokens = vec![TokenSpan {
            start_byte: 6,
            end_byte: 11,
            category: "replaced".to_string(),
        }];
        cache.apply_update(&new_tokens, 1, 1, false, content);

        // Line 0 should still have original
        let line0: Vec<_> = cache.tokens_for_line(0).collect();
        assert_eq!(line0.len(), 1);
        assert_eq!(line0[0].category, "a");

        // Line 1 should have replaced token
        let line1: Vec<_> = cache.tokens_for_line(1).collect();
        assert_eq!(line1.len(), 1);
        assert_eq!(line1[0].category, "replaced");

        // Line 2 should still have original
        let line2: Vec<_> = cache.tokens_for_line(2).collect();
        assert_eq!(line2.len(), 1);
        assert_eq!(line2[0].category, "c");
    }

    #[test]
    fn test_rebuild_line_offsets_empty() {
        let mut cache = TokenCache::new();
        cache.rebuild_line_offsets("");
        assert_eq!(cache.line_offsets, vec![0]);
    }

    #[test]
    fn test_rebuild_line_offsets_single_line() {
        let mut cache = TokenCache::new();
        cache.rebuild_line_offsets("hello world");
        assert_eq!(cache.line_offsets, vec![0]);
    }

    #[test]
    fn test_line_end_col() {
        let mut cache = TokenCache::new();
        let content = "hello\nworld\n";
        cache.rebuild_line_offsets(content);

        // Line 0: "hello\n" -> end col should be 5 (excluding newline)
        assert_eq!(cache.line_end_col(0, content), 5);

        // Line 1: "world\n" -> end col should be 5
        assert_eq!(cache.line_end_col(1, content), 5);
    }

    #[test]
    fn test_line_end_col_no_trailing_newline() {
        let mut cache = TokenCache::new();
        let content = "hello\nworld";
        cache.rebuild_line_offsets(content);

        // Line 1: "world" (no newline) -> end col should be 5
        assert_eq!(cache.line_end_col(1, content), 5);
    }

    // =========================================================================
    // TokenCacheManager extended tests
    // =========================================================================

    #[test]
    fn test_cache_manager_remove() {
        let mut manager = TokenCacheManager::new();
        manager.get_or_create(1);
        assert!(manager.get(1).is_some());

        manager.remove(1);
        assert!(manager.get(1).is_none());
    }

    #[test]
    fn test_cache_manager_clear() {
        let mut manager = TokenCacheManager::new();
        manager.get_or_create(1);
        manager.get_or_create(2);
        manager.get_or_create(3);

        manager.clear();
        assert!(manager.get(1).is_none());
        assert!(manager.get(2).is_none());
        assert!(manager.get(3).is_none());
    }

    #[test]
    fn test_cache_manager_apply_token_update() {
        let mut manager = TokenCacheManager::new();
        let content = "fn main() {}";
        let tokens = vec![TokenSpan {
            start_byte: 0,
            end_byte: 2,
            category: "keyword".to_string(),
        }];

        manager.apply_token_update(1, &tokens, 0, 0, true, content);

        let line_tokens: Vec<_> = manager.tokens_for_line(1, 0).collect();
        assert_eq!(line_tokens.len(), 1);
        assert_eq!(line_tokens[0].category, "keyword");
    }

    #[test]
    fn test_cache_manager_default() {
        let manager = TokenCacheManager::default();
        // Default should have max 10 buffers
        assert!(manager.get(999).is_none());
    }

    #[test]
    fn test_cache_manager_with_max_buffers() {
        let manager = TokenCacheManager::with_max_buffers(5);
        assert!(manager.get(0).is_none());
    }

}
