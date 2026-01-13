//! Generic line-based cache with content hash validation.
//!
//! This module provides `LineCache<T>`, a lock-free cache that stores
//! values indexed by line number with hash-based invalidation. It uses
//! `ArcSwap` for the RCU (Read-Copy-Update) pattern, enabling lock-free
//! reads on the hot path.
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - Generic type parameter `T` allows any cache value type
//! - Hash validation is provided but interpretation is up to the driver
//! - No syntax-specific knowledge in the kernel
//!
//! # Performance
//!
//! - Read operations: O(1) hash lookup, lock-free via `ArcSwap::load()`
//! - Write operations: O(n) RCU clone, acceptable for background updates
//! - Memory: One `Arc<HashMap>` plus one per concurrent reader
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//! use std::collections::HashMap;
//!
//! // Create a cache for syntax highlighting spans
//! let cache: LineCache<Vec<(usize, usize)>> = LineCache::new();
//!
//! // Store some entries (line_idx -> (hash, value))
//! let mut entries = HashMap::new();
//! entries.insert(0, (12345u64, vec![(0, 5), (6, 10)]));
//! entries.insert(1, (67890u64, vec![(0, 3)]));
//! cache.store(entries);
//!
//! // Check if a line is cached with matching hash
//! assert!(cache.has(0, 12345));
//! assert!(!cache.has(0, 99999)); // Wrong hash
//!
//! // Get cached value if hash matches
//! let spans = cache.get_if_valid(0, 12345);
//! assert!(spans.is_some());
//! ```

use std::collections::HashMap;

use reovim_arch::sync::ArcSwap;

/// A generic line-based cache with hash-based validation.
///
/// `LineCache<T>` stores values indexed by line number, where each entry
/// includes a content hash for invalidation checking. This enables efficient
/// cache reuse when line content hasn't changed.
///
/// # Thread Safety
///
/// `LineCache` is `Send + Sync` when `T` is. The underlying `ArcSwap` provides
/// lock-free reads and atomic updates via the RCU pattern.
///
/// # Type Parameters
///
/// - `T`: The cached value type. Must be `Clone + Send + Sync + 'static`.
#[derive(Debug)]
pub struct LineCache<T> {
    /// Inner storage: `line_idx` -> (`content_hash`, `cached_value`)
    inner: ArcSwap<HashMap<usize, (u64, T)>>,
}

impl<T: Clone + Send + Sync + 'static> LineCache<T> {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ArcSwap::from_pointee(HashMap::new()),
        }
    }

    /// Check if a line is cached with a matching hash.
    ///
    /// Returns `true` if the line exists in the cache AND the stored hash
    /// matches the provided hash. This is a fast validity check.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - The line index to check
    /// * `hash` - The expected content hash
    #[must_use]
    pub fn has(&self, line_idx: usize, hash: u64) -> bool {
        self.inner
            .load()
            .get(&line_idx)
            .is_some_and(|(h, _)| *h == hash)
    }

    /// Get cached value only if the hash matches.
    ///
    /// Returns `Some(value)` if the line is cached AND the stored hash
    /// matches. Returns `None` if not cached or hash mismatch.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - The line index to retrieve
    /// * `hash` - The expected content hash
    #[must_use]
    pub fn get_if_valid(&self, line_idx: usize, hash: u64) -> Option<T> {
        self.inner
            .load()
            .get(&line_idx)
            .filter(|(h, _)| *h == hash)
            .map(|(_, v)| v.clone())
    }

    /// Get cached value without hash validation.
    ///
    /// Returns `Some((hash, value))` if the line is cached, regardless of
    /// whether the hash is current. Use this when you need to inspect
    /// the cached hash.
    #[must_use]
    pub fn get(&self, line_idx: usize) -> Option<(u64, T)> {
        self.inner.load().get(&line_idx).cloned()
    }

    /// Store entries in the cache, replacing all existing content.
    ///
    /// This uses the RCU pattern: atomically swaps the entire cache contents.
    /// Concurrent readers see either the old or new state, never a partial update.
    ///
    /// # Arguments
    ///
    /// * `entries` - Map of `line_idx` -> (hash, value) to store
    pub fn store(&self, entries: HashMap<usize, (u64, T)>) {
        // RCU update: follows event_bus.rs:173-180 pattern
        // The closure must return the new value
        self.inner.rcu(move |_current| entries.clone());
    }

    /// Update specific entries without replacing the entire cache.
    ///
    /// Merges the provided entries into the existing cache. Existing entries
    /// not in `updates` are preserved.
    ///
    /// # Arguments
    ///
    /// * `updates` - Map of `line_idx` -> (hash, value) to merge
    pub fn update(&self, updates: &HashMap<usize, (u64, T)>) {
        let updates = updates.clone();
        self.inner.rcu(move |current| {
            let mut new_map = (**current).clone();
            new_map.extend(updates.clone());
            new_map
        });
    }

    /// Clone current entries for external modification.
    ///
    /// Returns a clone of all cached entries. Use this when you need to
    /// inspect or transform the cache contents.
    #[must_use]
    pub fn clone_entries(&self) -> HashMap<usize, (u64, T)> {
        (**self.inner.load()).clone()
    }

    /// Invalidate (remove) entries from a specific line onward.
    ///
    /// Removes all entries where `line_idx >= from_line`. This is useful
    /// when text is inserted or deleted, invalidating subsequent lines.
    ///
    /// # Arguments
    ///
    /// * `from_line` - First line index to invalidate (inclusive)
    pub fn invalidate_from(&self, from_line: usize) {
        self.inner.rcu(|current| {
            let filtered: HashMap<usize, (u64, T)> = current
                .iter()
                .filter(|(idx, _)| **idx < from_line)
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            filtered
        });
    }

    /// Invalidate a specific line.
    ///
    /// Removes the entry for a single line if it exists.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - The line index to invalidate
    pub fn invalidate_line(&self, line_idx: usize) {
        self.inner.rcu(|current| {
            let mut new_map = (**current).clone();
            new_map.remove(&line_idx);
            new_map
        });
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        self.inner.rcu(|_| HashMap::new());
    }

    /// Get the number of cached lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.load().len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.load().is_empty()
    }

    /// Get all cached line indices.
    #[must_use]
    pub fn cached_lines(&self) -> Vec<usize> {
        self.inner.load().keys().copied().collect()
    }
}

impl<T: Clone + Send + Sync + 'static> Default for LineCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

// LineCache is automatically Send + Sync when T is, because:
// - ArcSwap<HashMap<K, V>> is Send + Sync when HashMap<K, V> is
// - HashMap<usize, (u64, T)> is Send + Sync when T is

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_cache_new() {
        let cache: LineCache<String> = LineCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_line_cache_has_with_valid_hash() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (12345u64, 100));
        cache.store(entries);

        assert!(cache.has(0, 12345));
    }

    #[test]
    fn test_line_cache_has_with_invalid_hash() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (12345u64, 100));
        cache.store(entries);

        assert!(!cache.has(0, 99999)); // Wrong hash
        assert!(!cache.has(1, 12345)); // Wrong line
    }

    #[test]
    fn test_line_cache_get_if_valid() {
        let cache: LineCache<String> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (100u64, "hello".to_string()));
        entries.insert(1, (200u64, "world".to_string()));
        cache.store(entries);

        assert_eq!(cache.get_if_valid(0, 100), Some("hello".to_string()));
        assert_eq!(cache.get_if_valid(1, 200), Some("world".to_string()));
        assert_eq!(cache.get_if_valid(0, 999), None); // Wrong hash
        assert_eq!(cache.get_if_valid(2, 100), None); // Wrong line
    }

    #[test]
    fn test_line_cache_store() {
        let cache: LineCache<i32> = LineCache::new();

        let mut entries1 = HashMap::new();
        entries1.insert(0, (1u64, 10));
        cache.store(entries1);
        assert_eq!(cache.len(), 1);

        // Store replaces all content
        let mut entries2 = HashMap::new();
        entries2.insert(1, (2u64, 20));
        entries2.insert(2, (3u64, 30));
        cache.store(entries2);
        assert_eq!(cache.len(), 2);
        assert!(!cache.has(0, 1)); // Line 0 was replaced
    }

    #[test]
    fn test_line_cache_update() {
        let cache: LineCache<i32> = LineCache::new();

        let mut entries1 = HashMap::new();
        entries1.insert(0, (1u64, 10));
        cache.store(entries1);

        // Update merges, doesn't replace
        let mut entries2 = HashMap::new();
        entries2.insert(1, (2u64, 20));
        cache.update(&entries2);

        assert_eq!(cache.len(), 2);
        assert!(cache.has(0, 1)); // Line 0 preserved
        assert!(cache.has(1, 2)); // Line 1 added
    }

    #[test]
    fn test_line_cache_clone_entries() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (1u64, 10));
        entries.insert(1, (2u64, 20));
        cache.store(entries.clone());

        let cloned = cache.clone_entries();
        assert_eq!(cloned, entries);
    }

    #[test]
    fn test_line_cache_invalidate_from() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (1u64, 10));
        entries.insert(1, (2u64, 20));
        entries.insert(2, (3u64, 30));
        entries.insert(3, (4u64, 40));
        cache.store(entries);

        cache.invalidate_from(2);

        assert!(cache.has(0, 1));
        assert!(cache.has(1, 2));
        assert!(!cache.has(2, 3)); // Invalidated
        assert!(!cache.has(3, 4)); // Invalidated
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_line_cache_invalidate_line() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (1u64, 10));
        entries.insert(1, (2u64, 20));
        cache.store(entries);

        cache.invalidate_line(0);

        assert!(!cache.has(0, 1));
        assert!(cache.has(1, 2));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_line_cache_clear() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(0, (1u64, 10));
        cache.store(entries);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_line_cache_concurrent_access() {
        use std::{sync::Arc, thread};

        let cache = Arc::new(LineCache::<i32>::new());

        // Spawn readers
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = cache.has(i, i as u64);
                        let _ = cache.get_if_valid(i, i as u64);
                    }
                })
            })
            .collect();

        // Concurrent writer
        let writer_cache = Arc::clone(&cache);
        let writer = thread::spawn(move || {
            for j in 0_usize..100 {
                let mut entries = HashMap::new();
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                entries.insert(j % 10, (j as u64, j as i32));
                writer_cache.update(&entries);
            }
        });

        for handle in handles {
            handle.join().unwrap();
        }
        writer.join().unwrap();
    }

    #[test]
    fn test_line_cache_get_without_validation() {
        let cache: LineCache<String> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(5, (999u64, "data".to_string()));
        cache.store(entries);

        let result = cache.get(5);
        assert!(result.is_some());
        let (hash, value) = result.unwrap();
        assert_eq!(hash, 999);
        assert_eq!(value, "data");

        assert!(cache.get(99).is_none());
    }

    #[test]
    fn test_line_cache_cached_lines() {
        let cache: LineCache<i32> = LineCache::new();
        let mut entries = HashMap::new();
        entries.insert(1, (1u64, 10));
        entries.insert(5, (5u64, 50));
        entries.insert(10, (10u64, 100));
        cache.store(entries);

        let mut lines = cache.cached_lines();
        lines.sort_unstable();
        assert_eq!(lines, vec![1, 5, 10]);
    }
}
