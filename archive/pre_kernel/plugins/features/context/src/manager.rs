//! Context manager for caching and computing context
//!
//! Manages context computation and caching to avoid redundant calculations.

use std::sync::Arc;

use {arc_swap::ArcSwap, reovim_core::context_provider::ContextHierarchy};

/// Cached context state
#[derive(Debug, Clone)]
pub struct CachedContext {
    /// Buffer ID this context is for
    pub buffer_id: usize,
    /// Line where context was computed
    pub line: u32,
    /// Column where context was computed
    pub col: u32,
    /// The context hierarchy
    pub context: Option<ContextHierarchy>,
}

/// Last known cursor position for change detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastCursorPosition {
    pub buffer_id: usize,
    pub line: u32,
    pub col: u32,
}

/// Last known viewport position for change detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastViewportPosition {
    pub buffer_id: usize,
    pub top_line: u32,
}

/// Context manager that caches computed contexts
///
/// Uses `ArcSwap` for lock-free reads during rendering.
pub struct ContextManager {
    /// Cached cursor context (per active buffer)
    cursor_context: ArcSwap<Option<CachedContext>>,
    /// Cached viewport context (per window)
    viewport_context: ArcSwap<Option<CachedContext>>,
    /// Last known cursor position to detect changes (None = never processed)
    last_cursor: std::sync::RwLock<Option<LastCursorPosition>>,
    /// Last known viewport top line to detect changes (None = never processed)
    last_viewport: std::sync::RwLock<Option<LastViewportPosition>>,
    /// Buffer modification version to invalidate cache
    buffer_versions: std::sync::RwLock<std::collections::HashMap<usize, u64>>,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    /// Create a new context manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor_context: ArcSwap::new(Arc::new(None)),
            viewport_context: ArcSwap::new(Arc::new(None)),
            last_cursor: std::sync::RwLock::new(None),
            last_viewport: std::sync::RwLock::new(None),
            buffer_versions: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Check if cursor position changed
    ///
    /// Returns true if position differs from last known position, or if never processed.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn cursor_changed(&self, buffer_id: usize, line: u32, col: u32) -> bool {
        let last = self.last_cursor.read().unwrap();
        last.is_none_or(|pos| {
            pos != LastCursorPosition {
                buffer_id,
                line,
                col,
            }
        })
    }

    /// Update last cursor position
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn update_cursor(&self, buffer_id: usize, line: u32, col: u32) {
        let mut last = self.last_cursor.write().unwrap();
        *last = Some(LastCursorPosition {
            buffer_id,
            line,
            col,
        });
    }

    /// Check if viewport top line changed
    ///
    /// Returns true if position differs from last known position, or if never processed.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn viewport_changed(&self, buffer_id: usize, top_line: u32) -> bool {
        let last = self.last_viewport.read().unwrap();
        last.is_none_or(|pos| {
            pos != LastViewportPosition {
                buffer_id,
                top_line,
            }
        })
    }

    /// Update last viewport position
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn update_viewport(&self, buffer_id: usize, top_line: u32) {
        let mut last = self.last_viewport.write().unwrap();
        *last = Some(LastViewportPosition {
            buffer_id,
            top_line,
        });
    }

    /// Store cursor context
    pub fn set_cursor_context(&self, context: CachedContext) {
        self.cursor_context.store(Arc::new(Some(context)));
    }

    /// Get cursor context (lock-free read)
    #[must_use]
    pub fn get_cursor_context(&self) -> Arc<Option<CachedContext>> {
        self.cursor_context.load_full()
    }

    /// Store viewport context
    pub fn set_viewport_context(&self, context: CachedContext) {
        self.viewport_context.store(Arc::new(Some(context)));
    }

    /// Get viewport context (lock-free read)
    #[must_use]
    pub fn get_viewport_context(&self) -> Arc<Option<CachedContext>> {
        self.viewport_context.load_full()
    }

    /// Invalidate cache for a buffer (called on `BufferModified`)
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn invalidate_buffer(&self, buffer_id: usize) {
        // Increment buffer version
        *self
            .buffer_versions
            .write()
            .unwrap()
            .entry(buffer_id)
            .or_insert(0) += 1;

        // Clear cached contexts if they're for this buffer
        if let Some(ref ctx) = **self.cursor_context.load()
            && ctx.buffer_id == buffer_id
        {
            self.cursor_context.store(Arc::new(None));
        }
        if let Some(ref ctx) = **self.viewport_context.load()
            && ctx.buffer_id == buffer_id
        {
            self.viewport_context.store(Arc::new(None));
        }
    }

    /// Get current buffer version (for cache validation)
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn buffer_version(&self, buffer_id: usize) -> u64 {
        self.buffer_versions
            .read()
            .unwrap()
            .get(&buffer_id)
            .copied()
            .unwrap_or(0)
    }
}

/// Shared context manager for cross-plugin access
pub type SharedContextManager = Arc<ContextManager>;

#[cfg(test)]
mod tests {
    use {super::*, reovim_core::context_provider::ContextItem};

    fn create_test_hierarchy(buffer_id: usize, line: u32, col: u32) -> ContextHierarchy {
        ContextHierarchy::with_items(
            buffer_id,
            line,
            col,
            vec![ContextItem {
                text: "test_function".to_string(),
                start_line: 0,
                end_line: 100,
                kind: "function".to_string(),
                level: 0,
            }],
        )
    }

    #[test]
    fn test_cursor_change_detection() {
        let manager = ContextManager::new();

        // Initial state is None, so any position should be detected as changed
        assert!(manager.cursor_changed(0, 0, 0));
        assert!(manager.cursor_changed(1, 0, 0));

        // After update, same position should not be changed
        manager.update_cursor(0, 0, 0);
        assert!(!manager.cursor_changed(0, 0, 0));
        assert!(manager.cursor_changed(1, 0, 0)); // Different buffer
        assert!(manager.cursor_changed(0, 1, 0)); // Different line
        assert!(manager.cursor_changed(0, 0, 1)); // Different column
    }

    #[test]
    fn test_cursor_update() {
        let manager = ContextManager::new();

        // Update cursor position
        manager.update_cursor(5, 10, 20);

        // Now the new position should not be detected as changed
        assert!(!manager.cursor_changed(5, 10, 20));
        assert!(manager.cursor_changed(5, 11, 20)); // Different line
    }

    #[test]
    fn test_viewport_change_detection() {
        let manager = ContextManager::new();

        // Initial state is None, so any position should be detected as changed
        assert!(manager.viewport_changed(0, 0));
        assert!(manager.viewport_changed(1, 0));

        // After update, same position should not be changed
        manager.update_viewport(0, 0);
        assert!(!manager.viewport_changed(0, 0));
        assert!(manager.viewport_changed(1, 0)); // Different buffer
        assert!(manager.viewport_changed(0, 1)); // Different top line
    }

    #[test]
    fn test_viewport_update() {
        let manager = ContextManager::new();

        // Update viewport position
        manager.update_viewport(3, 50);

        // Now the new position should not be detected as changed
        assert!(!manager.viewport_changed(3, 50));
        assert!(manager.viewport_changed(3, 51)); // Different top line
    }

    #[test]
    fn test_cursor_context_caching() {
        let manager = ContextManager::new();

        // Initially no context
        assert!((*manager.get_cursor_context()).is_none());

        // Set cursor context
        let hierarchy = create_test_hierarchy(1, 5, 0);
        manager.set_cursor_context(CachedContext {
            buffer_id: 1,
            line: 5,
            col: 0,
            context: Some(hierarchy),
        });

        // Now context should be available
        let cached = manager.get_cursor_context();
        assert!((*cached).is_some());
        let ctx = cached.as_ref().as_ref().unwrap();
        assert_eq!(ctx.buffer_id, 1);
        assert_eq!(ctx.line, 5);
    }

    #[test]
    fn test_viewport_context_caching() {
        let manager = ContextManager::new();

        // Initially no context
        assert!((*manager.get_viewport_context()).is_none());

        // Set viewport context
        let hierarchy = create_test_hierarchy(2, 100, 0);
        manager.set_viewport_context(CachedContext {
            buffer_id: 2,
            line: 100,
            col: 0,
            context: Some(hierarchy),
        });

        // Now context should be available
        let cached = manager.get_viewport_context();
        assert!((*cached).is_some());
        let ctx = cached.as_ref().as_ref().unwrap();
        assert_eq!(ctx.buffer_id, 2);
        assert_eq!(ctx.line, 100);
    }

    #[test]
    fn test_buffer_invalidation() {
        let manager = ContextManager::new();

        // Set contexts for buffer 1
        manager.set_cursor_context(CachedContext {
            buffer_id: 1,
            line: 5,
            col: 0,
            context: Some(create_test_hierarchy(1, 5, 0)),
        });
        manager.set_viewport_context(CachedContext {
            buffer_id: 1,
            line: 0,
            col: 0,
            context: Some(create_test_hierarchy(1, 0, 0)),
        });

        // Both should be cached
        assert!((*manager.get_cursor_context()).is_some());
        assert!((*manager.get_viewport_context()).is_some());

        // Invalidate buffer 1
        manager.invalidate_buffer(1);

        // Both should be cleared
        assert!((*manager.get_cursor_context()).is_none());
        assert!((*manager.get_viewport_context()).is_none());
    }

    #[test]
    fn test_buffer_invalidation_different_buffer() {
        let manager = ContextManager::new();

        // Set contexts for buffer 1
        manager.set_cursor_context(CachedContext {
            buffer_id: 1,
            line: 5,
            col: 0,
            context: Some(create_test_hierarchy(1, 5, 0)),
        });

        // Invalidate buffer 2 (different buffer)
        manager.invalidate_buffer(2);

        // Buffer 1's context should still be cached
        assert!((*manager.get_cursor_context()).is_some());
    }

    #[test]
    fn test_buffer_version() {
        let manager = ContextManager::new();

        // Initial version is 0
        assert_eq!(manager.buffer_version(1), 0);

        // Invalidate increments version
        manager.invalidate_buffer(1);
        assert_eq!(manager.buffer_version(1), 1);

        manager.invalidate_buffer(1);
        assert_eq!(manager.buffer_version(1), 2);

        // Different buffer still at 0
        assert_eq!(manager.buffer_version(2), 0);
    }
}
