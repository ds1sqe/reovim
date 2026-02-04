//! Lock-free completion cache using ArcSwap
//!
//! Follows the highlight_cache.rs pattern for non-blocking reads during render.

use std::{sync::Arc, time::Instant};

use arc_swap::ArcSwap;

use reovim_core::completion::CompletionItem;

/// A snapshot of completion state at a point in time
#[derive(Debug, Clone)]
pub struct CompletionSnapshot {
    /// Completion items (already filtered and sorted)
    pub items: Vec<CompletionItem>,
    /// The prefix used for filtering
    pub prefix: String,
    /// Buffer ID this completion is for
    pub buffer_id: usize,
    /// Cursor position when completion was triggered
    pub cursor_row: u32,
    pub cursor_col: u32,
    /// Column where the word being completed starts
    pub word_start_col: u32,
    /// When this snapshot was created
    pub timestamp: Instant,
    /// Whether completion is active
    pub active: bool,
    /// Currently selected index
    pub selected_index: usize,
}

impl Default for CompletionSnapshot {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            prefix: String::new(),
            buffer_id: 0,
            cursor_row: 0,
            cursor_col: 0,
            word_start_col: 0,
            timestamp: Instant::now(),
            active: false,
            selected_index: 0,
        }
    }
}

impl CompletionSnapshot {
    /// Create a new active snapshot with items
    #[must_use]
    pub fn new(
        items: Vec<CompletionItem>,
        prefix: String,
        buffer_id: usize,
        cursor_row: u32,
        cursor_col: u32,
        word_start_col: u32,
    ) -> Self {
        Self {
            items,
            prefix,
            buffer_id,
            cursor_row,
            cursor_col,
            word_start_col,
            timestamp: Instant::now(),
            active: true,
            selected_index: 0,
        }
    }

    /// Create an inactive/dismissed snapshot
    #[must_use]
    pub fn dismissed() -> Self {
        Self {
            active: false,
            ..Self::default()
        }
    }

    /// Get the currently selected item
    #[must_use]
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        if self.items.is_empty() {
            None
        } else {
            self.items.get(self.selected_index)
        }
    }

    /// Check if there are any items
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Get the number of items
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

/// Lock-free completion cache
///
/// Uses ArcSwap for atomic snapshot replacement, allowing the render
/// thread to read without blocking the saturator thread.
pub struct CompletionCache {
    current: ArcSwap<CompletionSnapshot>,
}

impl CompletionCache {
    /// Create a new empty cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: ArcSwap::from_pointee(CompletionSnapshot::default()),
        }
    }

    /// Store a new snapshot (called by saturator)
    ///
    /// This is lock-free and will immediately be visible to readers.
    pub fn store(&self, snapshot: CompletionSnapshot) {
        self.current.store(Arc::new(snapshot));
    }

    /// Load the current snapshot (called by render)
    ///
    /// This is lock-free and will never block.
    #[must_use]
    pub fn load(&self) -> Arc<CompletionSnapshot> {
        self.current.load_full()
    }

    /// Update selected index without replacing the entire snapshot
    ///
    /// Creates a new snapshot with updated selection.
    pub fn update_selection(&self, new_index: usize) {
        let current = self.load();
        if current.active && new_index < current.items.len() {
            let mut new_snapshot = (*current).clone();
            new_snapshot.selected_index = new_index;
            self.store(new_snapshot);
        }
    }

    /// Select next item (wraps around)
    pub fn select_next(&self) {
        let current = self.load();
        if current.active && !current.items.is_empty() {
            let new_index = (current.selected_index + 1) % current.items.len();
            self.update_selection(new_index);
        }
    }

    /// Select previous item (wraps around)
    pub fn select_prev(&self) {
        let current = self.load();
        if current.active && !current.items.is_empty() {
            let new_index = if current.selected_index == 0 {
                current.items.len() - 1
            } else {
                current.selected_index - 1
            };
            self.update_selection(new_index);
        }
    }

    /// Dismiss completion
    pub fn dismiss(&self) {
        self.store(CompletionSnapshot::dismissed());
    }

    /// Check if completion is currently active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.load().active
    }
}

impl Default for CompletionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CompletionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snapshot = self.load();
        f.debug_struct("CompletionCache")
            .field("active", &snapshot.active)
            .field("item_count", &snapshot.items.len())
            .field("selected_index", &snapshot.selected_index)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_store_load() {
        let cache = CompletionCache::new();

        // Initially inactive
        assert!(!cache.is_active());

        // Store a snapshot
        let items = vec![
            CompletionItem::new("foo", "test"),
            CompletionItem::new("bar", "test"),
        ];
        let snapshot = CompletionSnapshot::new(items, "f".to_string(), 1, 0, 1, 0);
        cache.store(snapshot);

        // Now active
        assert!(cache.is_active());
        let loaded = cache.load();
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.prefix, "f");
    }

    #[test]
    fn test_selection_navigation() {
        let cache = CompletionCache::new();

        let items = vec![
            CompletionItem::new("a", "test"),
            CompletionItem::new("b", "test"),
            CompletionItem::new("c", "test"),
        ];
        let snapshot = CompletionSnapshot::new(items, "".to_string(), 1, 0, 0, 0);
        cache.store(snapshot);

        // Initially at 0
        assert_eq!(cache.load().selected_index, 0);

        // Next wraps around
        cache.select_next();
        assert_eq!(cache.load().selected_index, 1);
        cache.select_next();
        assert_eq!(cache.load().selected_index, 2);
        cache.select_next();
        assert_eq!(cache.load().selected_index, 0); // Wrapped

        // Prev wraps around
        cache.select_prev();
        assert_eq!(cache.load().selected_index, 2); // Wrapped
    }

    #[test]
    fn test_dismiss() {
        let cache = CompletionCache::new();

        let items = vec![CompletionItem::new("foo", "test")];
        let snapshot = CompletionSnapshot::new(items, "f".to_string(), 1, 0, 1, 0);
        cache.store(snapshot);

        assert!(cache.is_active());

        cache.dismiss();

        assert!(!cache.is_active());
        assert!(cache.load().items.is_empty());
    }

    #[test]
    fn test_snapshot_default() {
        let snapshot = CompletionSnapshot::default();

        assert!(!snapshot.active);
        assert!(snapshot.items.is_empty());
        assert_eq!(snapshot.selected_index, 0);
        assert_eq!(snapshot.buffer_id, 0);
        assert!(snapshot.prefix.is_empty());
    }

    #[test]
    fn test_snapshot_new_is_active() {
        let items = vec![CompletionItem::new("test", "source")];
        let snapshot = CompletionSnapshot::new(items, "te".to_string(), 1, 5, 7, 5);

        assert!(snapshot.active);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.prefix, "te");
        assert_eq!(snapshot.buffer_id, 1);
        assert_eq!(snapshot.cursor_row, 5);
        assert_eq!(snapshot.cursor_col, 7);
        assert_eq!(snapshot.word_start_col, 5);
        assert_eq!(snapshot.selected_index, 0);
    }

    #[test]
    fn test_snapshot_dismissed() {
        let dismissed = CompletionSnapshot::dismissed();

        assert!(!dismissed.active);
        assert!(dismissed.items.is_empty());
    }

    #[test]
    fn test_snapshot_selected_item() {
        let items = vec![
            CompletionItem::new("first", "test"),
            CompletionItem::new("second", "test"),
            CompletionItem::new("third", "test"),
        ];
        let mut snapshot = CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0);

        // Initially selects first item
        let selected = snapshot.selected_item();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().label, "first");

        // Change selection
        snapshot.selected_index = 2;
        let selected = snapshot.selected_item();
        assert_eq!(selected.unwrap().label, "third");
    }

    #[test]
    fn test_snapshot_selected_item_empty() {
        let snapshot = CompletionSnapshot::default();
        assert!(snapshot.selected_item().is_none());
    }

    #[test]
    fn test_snapshot_has_items() {
        let empty = CompletionSnapshot::default();
        assert!(!empty.has_items());

        let with_items = CompletionSnapshot::new(
            vec![CompletionItem::new("a", "test")],
            "".to_string(),
            0,
            0,
            0,
            0,
        );
        assert!(with_items.has_items());
    }

    #[test]
    fn test_snapshot_item_count() {
        let snapshot = CompletionSnapshot::new(
            vec![
                CompletionItem::new("a", "test"),
                CompletionItem::new("b", "test"),
            ],
            "".to_string(),
            0,
            0,
            0,
            0,
        );
        assert_eq!(snapshot.item_count(), 2);
    }

    #[test]
    fn test_update_selection() {
        let cache = CompletionCache::new();
        let items = vec![
            CompletionItem::new("a", "test"),
            CompletionItem::new("b", "test"),
            CompletionItem::new("c", "test"),
        ];
        cache.store(CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0));

        cache.update_selection(2);
        assert_eq!(cache.load().selected_index, 2);

        // Out of bounds should not update
        cache.update_selection(10);
        assert_eq!(cache.load().selected_index, 2); // Still 2
    }

    #[test]
    fn test_select_on_empty_cache() {
        let cache = CompletionCache::new();

        // Should not panic on empty cache
        cache.select_next();
        cache.select_prev();

        assert_eq!(cache.load().selected_index, 0);
    }

    #[test]
    fn test_select_on_single_item() {
        let cache = CompletionCache::new();
        cache.store(CompletionSnapshot::new(
            vec![CompletionItem::new("only", "test")],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        // Should wrap around to same item
        cache.select_next();
        assert_eq!(cache.load().selected_index, 0);

        cache.select_prev();
        assert_eq!(cache.load().selected_index, 0);
    }

    #[test]
    fn test_cache_debug_format() {
        let cache = CompletionCache::new();
        cache.store(CompletionSnapshot::new(
            vec![
                CompletionItem::new("a", "test"),
                CompletionItem::new("b", "test"),
            ],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        let debug_str = format!("{:?}", cache);
        assert!(debug_str.contains("CompletionCache"));
        assert!(debug_str.contains("active"));
        assert!(debug_str.contains("item_count"));
    }

    #[test]
    fn test_cache_default() {
        let cache = CompletionCache::default();
        assert!(!cache.is_active());
    }

    #[test]
    fn test_concurrent_reads() {
        use std::{sync::Arc, thread};

        let cache = Arc::new(CompletionCache::new());
        let items = vec![
            CompletionItem::new("a", "test"),
            CompletionItem::new("b", "test"),
            CompletionItem::new("c", "test"),
        ];
        cache.store(CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let snapshot = cache.load();
                        assert!(snapshot.active);
                        assert_eq!(snapshot.items.len(), 3);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_concurrent_read_write() {
        use std::{
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            thread,
        };

        let cache = Arc::new(CompletionCache::new());
        let running = Arc::new(AtomicBool::new(true));

        // Spawn reader threads
        let reader_handles: Vec<_> = (0..5)
            .map(|_| {
                let cache = Arc::clone(&cache);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    while running.load(Ordering::SeqCst) {
                        let snapshot = cache.load();
                        // Just read - shouldn't panic
                        let _ = snapshot.items.len();
                        let _ = snapshot.active;
                    }
                })
            })
            .collect();

        // Spawn writer threads
        let writer_handles: Vec<_> = (0..3)
            .map(|i| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for j in 0..50 {
                        let items = vec![CompletionItem::new(format!("item_{i}_{j}"), "test")];
                        cache.store(CompletionSnapshot::new(
                            items,
                            format!("prefix_{j}"),
                            i,
                            0,
                            0,
                            0,
                        ));
                    }
                })
            })
            .collect();

        // Wait for writers to finish
        for handle in writer_handles {
            handle.join().expect("writer thread panicked");
        }

        // Stop readers
        running.store(false, Ordering::SeqCst);
        for handle in reader_handles {
            handle.join().expect("reader thread panicked");
        }

        // Cache should have some value
        let snapshot = cache.load();
        assert!(!snapshot.items.is_empty() || !snapshot.active);
    }

    #[test]
    fn test_concurrent_selection_update() {
        use std::{sync::Arc, thread};

        let cache = Arc::new(CompletionCache::new());
        let items: Vec<_> = (0..100)
            .map(|i| CompletionItem::new(format!("item_{i}"), "test"))
            .collect();
        cache.store(CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for _ in 0..100 {
                        cache.select_next();
                        cache.select_prev();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        // Should still be valid (selection index should be within bounds)
        let snapshot = cache.load();
        assert!(snapshot.selected_index < 100);
    }

    #[test]
    fn test_rapid_store_load_cycle() {
        let cache = CompletionCache::new();

        for i in 0..1000 {
            let items = vec![CompletionItem::new(format!("item_{i}"), "test")];
            cache.store(CompletionSnapshot::new(items, format!("{i}"), i, 0, 0, 0));

            let snapshot = cache.load();
            assert!(snapshot.active);
            assert_eq!(snapshot.buffer_id, i);
        }
    }

    #[test]
    fn test_selection_bounds_after_new_store() {
        let cache = CompletionCache::new();

        // Store 10 items and select index 5
        let items: Vec<_> = (0..10)
            .map(|i| CompletionItem::new(format!("item_{i}"), "test"))
            .collect();
        cache.store(CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0));
        cache.update_selection(5);
        assert_eq!(cache.load().selected_index, 5);

        // Store new items (only 3) - selection should be reset to 0
        let new_items: Vec<_> = (0..3)
            .map(|i| CompletionItem::new(format!("new_{i}"), "test"))
            .collect();
        cache.store(CompletionSnapshot::new(new_items, "".to_string(), 0, 0, 0, 0));

        // New snapshot starts at 0
        assert_eq!(cache.load().selected_index, 0);
    }

    #[test]
    fn test_dismiss_clears_selection() {
        let cache = CompletionCache::new();
        let items = vec![
            CompletionItem::new("a", "test"),
            CompletionItem::new("b", "test"),
        ];
        cache.store(CompletionSnapshot::new(items, "".to_string(), 0, 0, 0, 0));
        cache.update_selection(1);

        cache.dismiss();

        let snapshot = cache.load();
        assert!(!snapshot.active);
        assert_eq!(snapshot.selected_index, 0);
    }

    #[test]
    fn test_multiple_store_overwrites() {
        let cache = CompletionCache::new();

        // Store first snapshot
        cache.store(CompletionSnapshot::new(
            vec![CompletionItem::new("first", "test")],
            "f".to_string(),
            0,
            0,
            0,
            0,
        ));
        assert_eq!(cache.load().items[0].label, "first");

        // Store second snapshot - should overwrite
        cache.store(CompletionSnapshot::new(
            vec![CompletionItem::new("second", "test")],
            "s".to_string(),
            0,
            0,
            0,
            0,
        ));
        assert_eq!(cache.load().items[0].label, "second");
        assert_eq!(cache.load().prefix, "s");
    }
}
