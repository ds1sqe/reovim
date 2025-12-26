//! Shared state for the completion plugin
//!
//! Provides thread-safe access to the completion manager.
//! Following the treesitter pattern for cross-plugin access.

use std::sync::{Arc, RwLock};

use crate::{
    CompletionCache, CompletionRequest, CompletionSaturatorHandle, SourceRegistry,
    cache::CompletionSnapshot, registry::SourceSupport, source::BufferWordsSource,
};

/// Shared completion manager state
///
/// Holds the source registry, cache, and saturator handle.
/// Registered in PluginStateRegistry for cross-plugin access.
pub struct SharedCompletionManager {
    /// Registry of completion sources
    pub registry: RwLock<SourceRegistry>,
    /// Lock-free completion cache
    pub cache: Arc<CompletionCache>,
    /// Saturator handle (set after boot)
    pub saturator: RwLock<Option<CompletionSaturatorHandle>>,
}

impl SharedCompletionManager {
    /// Create a new manager with default buffer source
    #[must_use]
    pub fn new() -> Self {
        let mut registry = SourceRegistry::new();
        // Register built-in buffer words source
        registry.register(Arc::new(BufferWordsSource::new()));

        Self {
            registry: RwLock::new(registry),
            cache: Arc::new(CompletionCache::new()),
            saturator: RwLock::new(None),
        }
    }

    /// Register a completion source
    pub fn register_source(&self, source: Arc<dyn SourceSupport>) {
        self.registry.write().unwrap().register(source);
    }

    /// Get all registered sources as Arc vec for saturator
    pub fn sources(&self) -> Arc<Vec<Arc<dyn SourceSupport>>> {
        Arc::new(self.registry.read().unwrap().sources().to_vec())
    }

    /// Set the saturator handle (called during boot)
    pub fn set_saturator(&self, handle: CompletionSaturatorHandle) {
        *self.saturator.write().unwrap() = Some(handle);
    }

    /// Request completion (delegates to saturator)
    pub fn request_completion(&self, request: CompletionRequest) {
        if let Some(handle) = self.saturator.read().unwrap().as_ref() {
            handle.request_completion(request);
        }
    }

    /// Dismiss completion
    pub fn dismiss(&self) {
        self.cache.dismiss();
    }

    /// Select next completion item
    pub fn select_next(&self) {
        self.cache.select_next();
    }

    /// Select previous completion item
    pub fn select_prev(&self) {
        self.cache.select_prev();
    }

    /// Check if completion is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.cache.is_active()
    }

    /// Get current snapshot
    #[must_use]
    pub fn snapshot(&self) -> Arc<CompletionSnapshot> {
        self.cache.load()
    }
}

impl Default for SharedCompletionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SharedCompletionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedCompletionManager")
            .field("cache", &self.cache)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_core::completion::{CompletionContext, CompletionItem},
        std::{future::Future, pin::Pin},
    };

    /// Test source for unit tests
    struct TestSource {
        id: &'static str,
        priority: u32,
    }

    impl TestSource {
        fn new(id: &'static str) -> Self {
            Self { id, priority: 100 }
        }

        fn with_priority(mut self, priority: u32) -> Self {
            self.priority = priority;
            self
        }
    }

    impl SourceSupport for TestSource {
        fn source_id(&self) -> &'static str {
            self.id
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        fn complete<'a>(
            &'a self,
            _ctx: &'a CompletionContext,
            _content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
            Box::pin(async move { vec![CompletionItem::new("test", self.id)] })
        }
    }

    #[test]
    fn test_shared_manager_new() {
        let manager = SharedCompletionManager::new();

        // Should have buffer source registered by default
        let sources = manager.sources();
        assert!(!sources.is_empty());

        // Should not be active initially
        assert!(!manager.is_active());
    }

    #[test]
    fn test_shared_manager_default() {
        let manager = SharedCompletionManager::default();
        assert!(!manager.is_active());
    }

    #[test]
    fn test_shared_manager_register_source() {
        let manager = SharedCompletionManager::new();
        let initial_count = manager.sources().len();

        manager.register_source(Arc::new(TestSource::new("test_source")));

        let sources = manager.sources();
        assert_eq!(sources.len(), initial_count + 1);
    }

    #[test]
    fn test_shared_manager_dismiss() {
        let manager = SharedCompletionManager::new();

        // Store some items in cache to make it active
        manager.cache.store(CompletionSnapshot::new(
            vec![CompletionItem::new("test", "test")],
            "t".to_string(),
            0,
            0,
            1,
            0,
        ));
        assert!(manager.is_active());

        // Dismiss
        manager.dismiss();
        assert!(!manager.is_active());
    }

    #[test]
    fn test_shared_manager_navigation() {
        let manager = SharedCompletionManager::new();

        // Store items
        manager.cache.store(CompletionSnapshot::new(
            vec![
                CompletionItem::new("a", "test"),
                CompletionItem::new("b", "test"),
                CompletionItem::new("c", "test"),
            ],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        // Initially at 0
        assert_eq!(manager.snapshot().selected_index, 0);

        // Select next
        manager.select_next();
        assert_eq!(manager.snapshot().selected_index, 1);

        // Select next again
        manager.select_next();
        assert_eq!(manager.snapshot().selected_index, 2);

        // Wrap around
        manager.select_next();
        assert_eq!(manager.snapshot().selected_index, 0);

        // Select prev wraps to end
        manager.select_prev();
        assert_eq!(manager.snapshot().selected_index, 2);
    }

    #[test]
    fn test_shared_manager_snapshot() {
        let manager = SharedCompletionManager::new();

        let items = vec![
            CompletionItem::new("foo", "test"),
            CompletionItem::new("bar", "test"),
        ];
        manager
            .cache
            .store(CompletionSnapshot::new(items, "prefix".to_string(), 42, 10, 15, 12));

        let snapshot = manager.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.items.len(), 2);
        assert_eq!(snapshot.prefix, "prefix");
        assert_eq!(snapshot.buffer_id, 42);
        assert_eq!(snapshot.cursor_row, 10);
        assert_eq!(snapshot.cursor_col, 15);
        assert_eq!(snapshot.word_start_col, 12);
    }

    #[test]
    fn test_shared_manager_sources_priority_order() {
        let manager = SharedCompletionManager::new();

        // Register sources with different priorities
        manager.register_source(Arc::new(TestSource::new("low").with_priority(200)));
        manager.register_source(Arc::new(TestSource::new("high").with_priority(10)));

        let sources = manager.sources();
        // High priority (10) should come before medium (100, buffer) before low (200)
        let ids: Vec<_> = sources.iter().map(|s| s.source_id()).collect();
        let high_pos = ids.iter().position(|id| *id == "high").unwrap();
        let low_pos = ids.iter().position(|id| *id == "low").unwrap();
        assert!(high_pos < low_pos);
    }

    #[test]
    fn test_shared_manager_debug_format() {
        let manager = SharedCompletionManager::new();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("SharedCompletionManager"));
        assert!(debug_str.contains("cache"));
    }

    #[test]
    fn test_shared_manager_request_without_saturator() {
        let manager = SharedCompletionManager::new();

        // Should not panic when saturator is not set
        let request = CompletionRequest {
            buffer_id: 1,
            file_path: None,
            content: "test".to_string(),
            cursor_row: 0,
            cursor_col: 0,
            line: String::new(),
            prefix: String::new(),
            word_start_col: 0,
            trigger_char: None,
        };
        manager.request_completion(request); // Should be no-op
    }

    #[test]
    fn test_shared_manager_concurrent_source_registration() {
        use std::thread;

        let manager = Arc::new(SharedCompletionManager::new());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    // Create a unique source for each thread
                    struct DynamicSource {
                        id: String,
                    }

                    impl SourceSupport for DynamicSource {
                        fn source_id(&self) -> &'static str {
                            // Leak the string to get a 'static reference
                            // This is acceptable in tests
                            Box::leak(self.id.clone().into_boxed_str())
                        }

                        fn priority(&self) -> u32 {
                            100
                        }

                        fn complete<'a>(
                            &'a self,
                            _ctx: &'a CompletionContext,
                            _content: &'a str,
                        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>>
                        {
                            Box::pin(async move { vec![] })
                        }
                    }

                    manager.register_source(Arc::new(DynamicSource {
                        id: format!("source_{}", i),
                    }));
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        // Should have 11 sources (1 default buffer + 10 registered)
        let sources = manager.sources();
        assert_eq!(sources.len(), 11);
    }

    #[test]
    fn test_shared_manager_concurrent_cache_access() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            thread,
        };

        let manager = Arc::new(SharedCompletionManager::new());
        let running = Arc::new(AtomicBool::new(true));

        // Store initial items
        manager.cache.store(CompletionSnapshot::new(
            vec![
                CompletionItem::new("a", "test"),
                CompletionItem::new("b", "test"),
                CompletionItem::new("c", "test"),
            ],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        // Spawn reader threads
        let reader_handles: Vec<_> = (0..5)
            .map(|_| {
                let manager = Arc::clone(&manager);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    while running.load(Ordering::SeqCst) {
                        let _snapshot = manager.snapshot();
                        let _active = manager.is_active();
                    }
                })
            })
            .collect();

        // Spawn navigation threads
        let nav_handles: Vec<_> = (0..3)
            .map(|_| {
                let manager = Arc::clone(&manager);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    while running.load(Ordering::SeqCst) {
                        manager.select_next();
                        manager.select_prev();
                    }
                })
            })
            .collect();

        // Let threads run for a bit
        thread::sleep(std::time::Duration::from_millis(50));

        // Stop threads
        running.store(false, Ordering::SeqCst);

        for handle in reader_handles {
            handle.join().expect("reader thread panicked");
        }
        for handle in nav_handles {
            handle.join().expect("nav thread panicked");
        }

        // Manager should still be in valid state
        let snapshot = manager.snapshot();
        assert!(snapshot.selected_index < 3);
    }

    #[test]
    fn test_shared_manager_concurrent_dismiss() {
        use std::thread;

        let manager = Arc::new(SharedCompletionManager::new());

        // Store initial items
        manager.cache.store(CompletionSnapshot::new(
            vec![CompletionItem::new("test", "test")],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    for _ in 0..100 {
                        if manager.is_active() {
                            manager.dismiss();
                        }
                        // Re-activate
                        manager.cache.store(CompletionSnapshot::new(
                            vec![CompletionItem::new("test", "test")],
                            "".to_string(),
                            0,
                            0,
                            0,
                            0,
                        ));
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        // Should complete without deadlock
    }

    #[test]
    fn test_shared_manager_sources_thread_safe() {
        use std::thread;

        let manager = Arc::new(SharedCompletionManager::new());

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let _sources = manager.sources();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_shared_manager_empty_after_dismiss() {
        let manager = SharedCompletionManager::new();

        // Store items
        manager.cache.store(CompletionSnapshot::new(
            vec![
                CompletionItem::new("a", "test"),
                CompletionItem::new("b", "test"),
            ],
            "prefix".to_string(),
            42,
            10,
            15,
            12,
        ));

        assert!(manager.is_active());
        assert_eq!(manager.snapshot().items.len(), 2);

        manager.dismiss();

        assert!(!manager.is_active());
        assert!(manager.snapshot().items.is_empty());
    }

    #[test]
    fn test_shared_manager_navigation_wraps() {
        let manager = SharedCompletionManager::new();

        manager.cache.store(CompletionSnapshot::new(
            vec![
                CompletionItem::new("first", "test"),
                CompletionItem::new("second", "test"),
            ],
            "".to_string(),
            0,
            0,
            0,
            0,
        ));

        // Start at 0
        assert_eq!(manager.snapshot().selected_index, 0);

        // Go to 1
        manager.select_next();
        assert_eq!(manager.snapshot().selected_index, 1);

        // Wrap to 0
        manager.select_next();
        assert_eq!(manager.snapshot().selected_index, 0);

        // Wrap to 1 (from 0 going backwards)
        manager.select_prev();
        assert_eq!(manager.snapshot().selected_index, 1);
    }
}
