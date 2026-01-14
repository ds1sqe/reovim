//! Statusline state management
//!
//! Provides thread-safe storage for registered sections and cached context.

use std::sync::{Arc, RwLock};

use reovim_core::{
    context_provider::ContextHierarchy,
    plugin::{RenderedSection, StatuslineRenderContext, StatuslineSectionProvider},
};

use crate::section::{SectionRenderContext, StatuslineSection};

/// Cached cursor context from `CursorContextUpdated` events
#[derive(Debug, Clone)]
pub struct CachedCursorContext {
    /// Buffer ID this context is for
    pub buffer_id: usize,
    /// Line where context was computed
    pub line: u32,
    /// Column where context was computed
    pub col: u32,
    /// The context hierarchy
    pub context: Option<ContextHierarchy>,
}

/// Inner state for the statusline manager
struct StatuslineManagerInner {
    sections: Vec<StatuslineSection>,
    enabled: bool,
    /// Cached cursor context from event subscription
    cached_cursor_context: Option<CachedCursorContext>,
}

impl Default for StatuslineManagerInner {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            enabled: true,
            cached_cursor_context: None,
        }
    }
}

/// Thread-safe statusline manager
///
/// Stores registered sections and implements the `StatuslineSectionProvider` trait.
pub struct SharedStatuslineManager {
    inner: RwLock<StatuslineManagerInner>,
}

impl Default for SharedStatuslineManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedStatuslineManager {
    /// Create a new statusline manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(StatuslineManagerInner::default()),
        }
    }

    /// Register a section
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register_section(&self, section: StatuslineSection) {
        let section_id = section.id;
        {
            let mut inner = self.inner.write().unwrap();
            // Replace if section with same ID exists
            inner.sections.retain(|s| s.id != section_id);
            inner.sections.push(section);
        }
        tracing::debug!(section_id = section_id, "Registered statusline section");
    }

    /// Unregister a section by ID
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn unregister_section(&self, id: &str) {
        let mut inner = self.inner.write().unwrap();
        let len_before = inner.sections.len();
        inner.sections.retain(|s| s.id != id);
        if inner.sections.len() < len_before {
            tracing::debug!(section_id = id, "Unregistered statusline section");
        }
    }

    /// Get the number of registered sections
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.inner.read().unwrap().sections.len()
    }

    /// Check if enabled
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.inner.read().unwrap().enabled
    }

    /// Set enabled state
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.write().unwrap().enabled = enabled;
    }

    /// Set cached cursor context
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_cached_cursor_context(&self, context: CachedCursorContext) {
        self.inner.write().unwrap().cached_cursor_context = Some(context);
    }

    /// Get cached cursor context
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_cached_cursor_context(&self) -> Option<CachedCursorContext> {
        self.inner.read().unwrap().cached_cursor_context.clone()
    }
}

impl StatuslineSectionProvider for SharedStatuslineManager {
    fn render_sections(&self, ctx: &StatuslineRenderContext) -> Vec<RenderedSection> {
        let inner = self.inner.read().unwrap();

        if !inner.enabled {
            return Vec::new();
        }

        let section_ctx = SectionRenderContext {
            plugin_state: ctx.plugin_state,
            active_buffer_id: ctx.active_buffer_id,
            buffer_content: ctx.buffer_content.as_deref(),
            cursor_row: ctx.cursor_row,
            cursor_col: ctx.cursor_col,
        };

        inner
            .sections
            .iter()
            .filter_map(|section| {
                let content = (section.render)(&section_ctx);
                if content.visible {
                    Some(RenderedSection {
                        text: content.text,
                        style: content.style,
                        alignment: section.alignment,
                        priority: section.priority,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Wrapper type for `Arc<SharedStatuslineManager>` to allow registration in `PluginStateRegistry`
pub type StatuslineManagerHandle = Arc<SharedStatuslineManager>;

#[cfg(test)]
mod tests {
    use {super::*, reovim_core::context_provider::ContextItem};

    fn create_test_hierarchy() -> ContextHierarchy {
        ContextHierarchy::with_items(
            1,
            5,
            0,
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
    fn test_cached_cursor_context_initially_none() {
        let manager = SharedStatuslineManager::new();
        assert!(manager.get_cached_cursor_context().is_none());
    }

    #[test]
    fn test_set_and_get_cached_cursor_context() {
        let manager = SharedStatuslineManager::new();

        // Set cached context
        manager.set_cached_cursor_context(CachedCursorContext {
            buffer_id: 1,
            line: 5,
            col: 10,
            context: Some(create_test_hierarchy()),
        });

        // Get cached context
        let cached = manager.get_cached_cursor_context();
        assert!(cached.is_some());

        let ctx = cached.unwrap();
        assert_eq!(ctx.buffer_id, 1);
        assert_eq!(ctx.line, 5);
        assert_eq!(ctx.col, 10);
        assert!(ctx.context.is_some());
    }

    #[test]
    fn test_cached_cursor_context_update() {
        let manager = SharedStatuslineManager::new();

        // Set initial context
        manager.set_cached_cursor_context(CachedCursorContext {
            buffer_id: 1,
            line: 5,
            col: 0,
            context: Some(create_test_hierarchy()),
        });

        // Update to new context
        manager.set_cached_cursor_context(CachedCursorContext {
            buffer_id: 2,
            line: 100,
            col: 50,
            context: None,
        });

        // Get updated context
        let cached = manager.get_cached_cursor_context().unwrap();
        assert_eq!(cached.buffer_id, 2);
        assert_eq!(cached.line, 100);
        assert_eq!(cached.col, 50);
        assert!(cached.context.is_none());
    }
}
