//! Plugin state for sticky context headers

use std::sync::RwLock;

use reovim_core::context_provider::ContextHierarchy;

/// Plugin state for sticky context headers
pub struct StickyContextState {
    /// Feature enabled/disabled
    enabled: RwLock<bool>,
    /// Maximum context lines (1-5, default 3)
    max_lines: RwLock<usize>,
    /// Show separator line
    show_separator: RwLock<bool>,
    /// Cached viewport context (from `ViewportContextUpdated` events)
    cached_context: RwLock<Option<CachedViewportContext>>,
}

/// Cached viewport context from `ViewportContextUpdated` events
#[derive(Debug, Clone)]
pub struct CachedViewportContext {
    /// Buffer ID this context is for
    pub buffer_id: usize,
    /// The context hierarchy
    pub context: Option<ContextHierarchy>,
}

impl StickyContextState {
    /// Create new sticky context state with defaults
    pub const fn new() -> Self {
        Self {
            enabled: RwLock::new(true),
            max_lines: RwLock::new(3),
            show_separator: RwLock::new(true),
            cached_context: RwLock::new(None),
        }
    }

    /// Check if feature is enabled
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    /// Get maximum lines to display
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn max_lines(&self) -> usize {
        *self.max_lines.read().unwrap()
    }

    /// Check if separator should be shown
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn show_separator(&self) -> bool {
        *self.show_separator.read().unwrap()
    }

    /// Update cached viewport context
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_cached_context(&self, context: CachedViewportContext) {
        *self.cached_context.write().unwrap() = Some(context);
    }

    /// Get cached viewport context
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn get_cached_context(&self) -> Option<CachedViewportContext> {
        self.cached_context.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_core::context_provider::ContextItem};

    fn create_test_hierarchy() -> ContextHierarchy {
        ContextHierarchy::with_items(
            1,
            0,
            0,
            vec![ContextItem {
                text: "fn main()".to_string(),
                start_line: 0,
                end_line: 100,
                kind: "function".to_string(),
                level: 0,
            }],
        )
    }

    #[test]
    fn test_initial_state() {
        let state = StickyContextState::new();
        assert!(state.is_enabled());
        assert_eq!(state.max_lines(), 3);
        assert!(state.show_separator());
        assert!(state.get_cached_context().is_none());
    }

    #[test]
    fn test_cached_viewport_context() {
        let state = StickyContextState::new();

        // Set cached context
        state.set_cached_context(CachedViewportContext {
            buffer_id: 1,
            context: Some(create_test_hierarchy()),
        });

        // Get cached context
        let cached = state.get_cached_context();
        assert!(cached.is_some());

        let ctx = cached.unwrap();
        assert_eq!(ctx.buffer_id, 1);
        assert!(ctx.context.is_some());
    }

    #[test]
    fn test_cached_context_update() {
        let state = StickyContextState::new();

        // Set initial context
        state.set_cached_context(CachedViewportContext {
            buffer_id: 1,
            context: Some(create_test_hierarchy()),
        });

        // Update to new context
        state.set_cached_context(CachedViewportContext {
            buffer_id: 2,
            context: None,
        });

        // Get updated context
        let cached = state.get_cached_context().unwrap();
        assert_eq!(cached.buffer_id, 2);
        assert!(cached.context.is_none());
    }
}
