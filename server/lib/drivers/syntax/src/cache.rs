//! Syntax cache trait.
//!
//! This module defines the [`SyntaxCache`] trait for caching highlight results.

use std::ops::Range;

use crate::highlight::Annotation;

/// Cache for annotation results.
///
/// Implementations can provide various caching strategies for
/// annotation data to improve performance.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// // Insert annotations for a range
/// cache.insert(0..100, annotations);
///
/// // Later, retrieve cached annotations
/// if let Some(cached) = cache.get(0..100) {
///     // Use cached annotations
/// }
///
/// // After an edit, invalidate affected ranges
/// cache.invalidate_range(50..75);
/// ```
pub trait SyntaxCache: Send + Sync {
    /// Get cached annotations for a byte range.
    ///
    /// Returns `None` if the range is not cached or cache is stale.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range to look up
    fn get(&self, byte_range: Range<usize>) -> Option<Vec<Annotation>>;

    /// Insert annotations into the cache.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range these annotations cover
    /// * `highlights` - The annotations to cache
    fn insert(&mut self, byte_range: Range<usize>, highlights: Vec<Annotation>);

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
    use {super::*, crate::HighlightCategory, std::collections::HashMap};

    /// Simple in-memory cache implementation for testing.
    struct SimpleCache {
        entries: HashMap<(usize, usize), Vec<Annotation>>,
    }

    impl SimpleCache {
        fn new() -> Self {
            Self {
                entries: HashMap::new(),
            }
        }
    }

    impl SyntaxCache for SimpleCache {
        fn get(&self, byte_range: Range<usize>) -> Option<Vec<Annotation>> {
            self.entries
                .get(&(byte_range.start, byte_range.end))
                .cloned()
        }

        fn insert(&mut self, byte_range: Range<usize>, highlights: Vec<Annotation>) {
            self.entries
                .insert((byte_range.start, byte_range.end), highlights);
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn invalidate_range(&mut self, byte_range: Range<usize>) {
            self.entries
                .retain(|(start, end), _| *end <= byte_range.start || *start >= byte_range.end);
        }

        fn clear(&mut self) {
            self.entries.clear();
        }

        fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }

    fn ann(start: usize, end: usize, cat: &str) -> Annotation {
        Annotation::highlight(start, end, HighlightCategory::new(cat))
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = SimpleCache::new();

        let highlights = vec![ann(0, 5, "keyword"), ann(6, 10, "function")];

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

        cache.insert(0..50, vec![ann(0, 50, "keyword")]);
        cache.insert(50..100, vec![ann(50, 100, "function")]);
        cache.insert(100..150, vec![ann(100, 150, "string")]);

        cache.invalidate_range(40..60);

        assert!(cache.get(0..50).is_none());
        assert!(cache.get(50..100).is_none());
        assert!(cache.get(100..150).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = SimpleCache::new();

        cache.insert(0..50, vec![ann(0, 50, "keyword")]);
        cache.insert(50..100, vec![ann(50, 100, "function")]);

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
