//! Syntax cache trait.
//!
//! This module defines the [`SyntaxCache`] trait for caching highlight results.

use std::ops::Range;

use crate::highlight::HighlightSpan;

/// Cache for highlight results.
///
/// Implementations can provide various caching strategies for
/// highlight data to improve performance.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// // Insert highlights for a range
/// cache.insert(0..100, highlights);
///
/// // Later, retrieve cached highlights
/// if let Some(cached) = cache.get(0..100) {
///     // Use cached highlights
/// }
///
/// // After an edit, invalidate affected ranges
/// cache.invalidate_range(50..75);
/// ```
pub trait SyntaxCache: Send + Sync {
    /// Get cached highlights for a byte range.
    ///
    /// Returns `None` if the range is not cached or cache is stale.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range to look up
    fn get(&self, byte_range: Range<usize>) -> Option<Vec<HighlightSpan>>;

    /// Insert highlights into the cache.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range these highlights cover
    /// * `highlights` - The highlights to cache
    fn insert(&mut self, byte_range: Range<usize>, highlights: Vec<HighlightSpan>);

    /// Invalidate cache entries that overlap with a range.
    ///
    /// Called after edits to mark affected regions as stale.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range that was modified
    fn invalidate_range(&mut self, byte_range: Range<usize>);

    /// Clear all cached data.
    fn clear(&mut self);

    /// Check if the cache is empty.
    fn is_empty(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use {super::*, crate::highlight::HighlightGroup, std::collections::HashMap};

    /// Simple in-memory cache implementation for testing.
    struct SimpleCache {
        entries: HashMap<(usize, usize), Vec<HighlightSpan>>,
    }

    impl SimpleCache {
        fn new() -> Self {
            Self {
                entries: HashMap::new(),
            }
        }
    }

    impl SyntaxCache for SimpleCache {
        fn get(&self, byte_range: Range<usize>) -> Option<Vec<HighlightSpan>> {
            self.entries
                .get(&(byte_range.start, byte_range.end))
                .cloned()
        }

        fn insert(&mut self, byte_range: Range<usize>, highlights: Vec<HighlightSpan>) {
            self.entries
                .insert((byte_range.start, byte_range.end), highlights);
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn invalidate_range(&mut self, byte_range: Range<usize>) {
            self.entries.retain(|(start, end), _| {
                // Keep entries that don't overlap with the invalidated range
                *end <= byte_range.start || *start >= byte_range.end
            });
        }

        fn clear(&mut self) {
            self.entries.clear();
        }

        fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = SimpleCache::new();

        let highlights = vec![
            HighlightSpan::new(0, 5, HighlightGroup::Keyword),
            HighlightSpan::new(6, 10, HighlightGroup::Function),
        ];

        cache.insert(0..100, highlights.clone());

        let cached = cache.get(0..100);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), highlights);
    }

    #[test]
    fn test_cache_get_missing() {
        let cache = SimpleCache::new();

        let cached = cache.get(0..100);
        assert!(cached.is_none());
    }

    #[test]
    fn test_cache_invalidate_range() {
        let mut cache = SimpleCache::new();

        // Insert multiple entries
        cache.insert(0..50, vec![HighlightSpan::new(0, 50, HighlightGroup::Keyword)]);
        cache.insert(50..100, vec![HighlightSpan::new(50, 100, HighlightGroup::Function)]);
        cache.insert(100..150, vec![HighlightSpan::new(100, 150, HighlightGroup::String)]);

        // Invalidate middle range
        cache.invalidate_range(40..60);

        // First entry overlaps with invalidated range, should be removed
        assert!(cache.get(0..50).is_none());
        // Second entry overlaps with invalidated range, should be removed
        assert!(cache.get(50..100).is_none());
        // Third entry doesn't overlap, should remain
        assert!(cache.get(100..150).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = SimpleCache::new();

        cache.insert(0..50, vec![HighlightSpan::new(0, 50, HighlightGroup::Keyword)]);
        cache.insert(50..100, vec![HighlightSpan::new(50, 100, HighlightGroup::Function)]);

        assert!(!cache.is_empty());

        cache.clear();

        assert!(cache.is_empty());
        assert!(cache.get(0..50).is_none());
        assert!(cache.get(50..100).is_none());
    }

    #[test]
    fn test_cache_is_empty() {
        let mut cache = SimpleCache::new();

        assert!(cache.is_empty());

        cache.insert(0..50, vec![]);
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }
}
