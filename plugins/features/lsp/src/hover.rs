//! LSP hover cache for non-blocking popup rendering.
//!
//! Uses `ArcSwap` for lock-free reads from the render thread.

use std::sync::Arc;

use arc_swap::ArcSwap;

/// Snapshot of hover content for rendering.
#[derive(Debug, Clone)]
pub struct HoverSnapshot {
    /// The hover content text (may contain markdown).
    pub content: String,
    /// Anchor row (0-indexed) where hover was triggered.
    pub anchor_row: usize,
    /// Anchor column (0-indexed) where hover was triggered.
    pub anchor_col: usize,
    /// Buffer ID where hover was triggered.
    pub buffer_id: usize,
}

impl HoverSnapshot {
    /// Create a new hover snapshot.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String is not const
    pub fn new(content: String, anchor_row: usize, anchor_col: usize, buffer_id: usize) -> Self {
        Self {
            content,
            anchor_row,
            anchor_col,
            buffer_id,
        }
    }

    /// Get the lines of content.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.content.lines()
    }

    /// Get the number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    /// Get the maximum line width.
    #[must_use]
    pub fn max_line_width(&self) -> usize {
        self.content
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    }
}

/// Lock-free hover cache using `ArcSwap`.
///
/// Allows the render thread to read hover content without blocking.
pub struct HoverCache {
    current: ArcSwap<Option<HoverSnapshot>>,
}

impl HoverCache {
    /// Create a new empty hover cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: ArcSwap::new(Arc::new(None)),
        }
    }

    /// Store hover content (called from async hover handler).
    pub fn store(&self, snapshot: HoverSnapshot) {
        self.current.store(Arc::new(Some(snapshot)));
    }

    /// Clear the hover content (dismiss popup).
    pub fn clear(&self) {
        self.current.store(Arc::new(None));
    }

    /// Load the current hover snapshot (lock-free read).
    #[must_use]
    pub fn load(&self) -> Option<HoverSnapshot> {
        self.current.load().as_ref().clone()
    }

    /// Check if hover is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.current.load().is_some()
    }
}

impl Default for HoverCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_cache_store_and_load() {
        let cache = HoverCache::new();
        assert!(!cache.is_active());
        assert!(cache.load().is_none());

        let snapshot = HoverSnapshot::new("Hello, world!".to_string(), 10, 5, 1);
        cache.store(snapshot);

        assert!(cache.is_active());
        let loaded = cache.load().unwrap();
        assert_eq!(loaded.content, "Hello, world!");
        assert_eq!(loaded.anchor_row, 10);
        assert_eq!(loaded.anchor_col, 5);
        assert_eq!(loaded.buffer_id, 1);
    }

    #[test]
    fn test_hover_cache_clear() {
        let cache = HoverCache::new();
        cache.store(HoverSnapshot::new("Test".to_string(), 0, 0, 0));
        assert!(cache.is_active());

        cache.clear();
        assert!(!cache.is_active());
        assert!(cache.load().is_none());
    }

    #[test]
    fn test_hover_snapshot_lines() {
        let snapshot = HoverSnapshot::new("Line 1\nLine 2\nLine 3".to_string(), 0, 0, 0);
        assert_eq!(snapshot.line_count(), 3);
        assert_eq!(snapshot.max_line_width(), 6);

        let lines: Vec<&str> = snapshot.lines().collect();
        assert_eq!(lines, vec!["Line 1", "Line 2", "Line 3"]);
    }
}
