//! Lock-free decoration cache for O(1) render reads
//!
//! Architecture (same as `HighlightCache`):
//! - `ArcSwap` for lock-free atomic swap of the cache
//! - Saturator: prepares new buffer, swaps atomically when ready
//! - Render: reads from current buffer (lock-free load)
//!
//! Thread-safe without locks:
//! - Render reads via `ArcSwap::load()` (lock-free)
//! - Saturator writes to new buffer, then `ArcSwap::store()` (lock-free)

use std::{collections::HashMap, sync::Arc};

use arc_swap::ArcSwap;

use super::Decoration;

/// Cache buffer storing line decorations
#[derive(Debug, Clone, Default)]
struct CacheBuffer {
    /// `line_index` -> cache entry
    entries: HashMap<usize, CacheEntry>,
}

/// Cache entry for a single line
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Hash of line content for invalidation detection
    content_hash: u64,
    /// Computed decorations for this line
    decorations: Vec<Decoration>,
}

/// Lock-free decoration cache using `ArcSwap`
///
/// Thread-safe for concurrent access:
/// - `load()`: Lock-free read of current cache
/// - `store()`: Lock-free atomic replacement
///
/// Saturator builds new buffer and swaps atomically.
/// Render reads current buffer without blocking.
#[derive(Debug)]
pub struct DecorationCache {
    /// Current cache buffer (atomically swappable)
    current: ArcSwap<CacheBuffer>,
}

impl Default for DecorationCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DecorationCache {
    fn clone(&self) -> Self {
        Self {
            current: ArcSwap::new(self.current.load_full()),
        }
    }
}

impl DecorationCache {
    /// Create new empty cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: ArcSwap::new(Arc::new(CacheBuffer::default())),
        }
    }

    // ========================================
    // SATURATOR API (cache logic owner)
    // ========================================

    /// Check if line is cached with matching content hash
    #[must_use]
    pub fn has(&self, line_idx: usize, content_hash: u64) -> bool {
        self.current
            .load()
            .entries
            .get(&line_idx)
            .is_some_and(|e| e.content_hash == content_hash)
    }

    /// Get cached decorations if content hash matches
    #[must_use]
    pub fn get(&self, line_idx: usize, content_hash: u64) -> Option<Vec<Decoration>> {
        self.current
            .load()
            .entries
            .get(&line_idx)
            .filter(|e| e.content_hash == content_hash)
            .map(|e| e.decorations.clone())
    }

    /// Get a mutable copy of current entries (for saturator to build upon)
    #[must_use]
    pub fn clone_entries(&self) -> HashMap<usize, (u64, Vec<Decoration>)> {
        self.current
            .load()
            .entries
            .iter()
            .map(|(&idx, e)| (idx, (e.content_hash, e.decorations.clone())))
            .collect()
    }

    /// Store a complete new buffer (atomic swap)
    ///
    /// Saturator calls this after computing all decorations.
    pub fn store(&self, entries: HashMap<usize, (u64, Vec<Decoration>)>) {
        let buffer = CacheBuffer {
            entries: entries
                .into_iter()
                .map(|(idx, (hash, decorations))| {
                    (
                        idx,
                        CacheEntry {
                            content_hash: hash,
                            decorations,
                        },
                    )
                })
                .collect(),
        };
        self.current.store(Arc::new(buffer));
    }

    // ========================================
    // RENDER API (zero cache logic)
    // ========================================

    /// Get all ready decorations for render
    ///
    /// Render calls this - no cache logic, just "give me decorations".
    /// Returns empty vec for lines without decorations.
    #[must_use]
    pub fn get_ready_decorations(&self, line_count: usize) -> Vec<Vec<Decoration>> {
        let buffer = self.current.load();
        let mut result = vec![Vec::new(); line_count];
        for (&line_idx, entry) in &buffer.entries {
            if line_idx < line_count {
                result[line_idx].clone_from(&entry.decorations);
            }
        }
        result
    }

    // ========================================
    // INVALIDATION API (main thread only)
    // ========================================

    /// Invalidate a single line (for single-line edits)
    pub fn invalidate_line(&self, line: usize) {
        let current = self.current.load_full();
        let mut new_entries = current.entries.clone();
        new_entries.remove(&line);
        self.current.store(Arc::new(CacheBuffer {
            entries: new_entries,
        }));
    }

    /// Invalidate from line to end (for structure changes like newline/delete)
    pub fn invalidate_from(&self, start_line: usize) {
        let current = self.current.load_full();
        let new_entries: HashMap<_, _> = current
            .entries
            .iter()
            .filter(|(k, _)| **k < start_line)
            .map(|(&k, v)| (k, v.clone()))
            .collect();
        self.current.store(Arc::new(CacheBuffer {
            entries: new_entries,
        }));
    }

    /// Clear all cache (for full content replacement)
    pub fn clear(&self) {
        self.current.store(Arc::new(CacheBuffer::default()));
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::render::DecorationKind};

    fn make_decoration(start: usize, end: usize) -> Decoration {
        Decoration {
            start_col: start,
            end_col: end,
            kind: DecorationKind::Conceal {
                replacement: Some("...".to_string()),
                col_mapping: None,
            },
        }
    }

    #[test]
    fn test_basic_cache_operations() {
        let cache = DecorationCache::new();

        // Initially empty
        assert!(!cache.has(0, 123));

        // Store entries
        let mut entries = HashMap::new();
        entries.insert(0, (123, vec![make_decoration(0, 5)]));
        cache.store(entries);

        // Now has the entry
        assert!(cache.has(0, 123));
        assert!(cache.get(0, 123).is_some());

        // Wrong hash
        assert!(!cache.has(0, 999));
    }

    #[test]
    fn test_content_hash_validation() {
        let cache = DecorationCache::new();

        let mut entries = HashMap::new();
        entries.insert(0, (100, vec![make_decoration(0, 5)]));
        cache.store(entries);

        // Correct hash
        assert!(cache.has(0, 100));

        // Wrong hash (content changed)
        assert!(!cache.has(0, 999));
    }

    #[test]
    fn test_get_ready_decorations() {
        let cache = DecorationCache::new();

        let mut entries = HashMap::new();
        entries.insert(1, (100, vec![make_decoration(0, 5)]));
        entries.insert(3, (200, vec![make_decoration(2, 8)]));
        cache.store(entries);

        let ready = cache.get_ready_decorations(5);
        assert_eq!(ready.len(), 5);
        assert!(ready[0].is_empty()); // No decorations
        assert_eq!(ready[1].len(), 1); // Has decoration
        assert!(ready[2].is_empty()); // No decorations
        assert_eq!(ready[3].len(), 1); // Has decoration
        assert!(ready[4].is_empty()); // No decorations
    }

    #[test]
    fn test_invalidation() {
        let cache = DecorationCache::new();

        // Populate cache
        let mut entries = HashMap::new();
        for i in 0..10 {
            entries.insert(i, (i as u64, vec![make_decoration(0, 1)]));
        }
        cache.store(entries);

        // Single line invalidation
        cache.invalidate_line(5);
        assert!(cache.has(4, 4));
        assert!(!cache.has(5, 5));
        assert!(cache.has(6, 6));

        // From-line invalidation
        cache.invalidate_from(7);
        assert!(cache.has(6, 6));
        assert!(!cache.has(7, 7));
        assert!(!cache.has(8, 8));

        // Full clear
        cache.clear();
        assert!(!cache.has(0, 0));
        assert!(!cache.has(6, 6));
    }

    #[test]
    fn test_clone_entries() {
        let cache = DecorationCache::new();

        let mut entries = HashMap::new();
        entries.insert(0, (100, vec![make_decoration(0, 5)]));
        entries.insert(1, (200, vec![make_decoration(0, 3)]));
        cache.store(entries);

        // Clone entries
        let cloned = cache.clone_entries();
        assert_eq!(cloned.len(), 2);
        assert_eq!(cloned.get(&0).unwrap().0, 100);
        assert_eq!(cloned.get(&1).unwrap().0, 200);
    }
}
