//! Statusline state management
//!
//! Provides thread-safe storage for registered sections.

use std::sync::{Arc, RwLock};

use reovim_core::plugin::{RenderedSection, StatuslineRenderContext, StatuslineSectionProvider};

use crate::section::{SectionRenderContext, StatuslineSection};

/// Inner state for the statusline manager
struct StatuslineManagerInner {
    sections: Vec<StatuslineSection>,
    enabled: bool,
}

impl Default for StatuslineManagerInner {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            enabled: true,
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
