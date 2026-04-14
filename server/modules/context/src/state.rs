//! Context session state.
//!
//! Per-client state holding the current scope hierarchy.

use {reovim_driver_text_session::SessionExtension, reovim_driver_text_syntax::ContextHierarchy};

/// Options for context display.
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Show breadcrumb in statusline.
    pub breadcrumb: bool,
    /// Breadcrumb separator string.
    pub separator: String,
    /// Maximum breadcrumb depth.
    pub max_items: usize,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            breadcrumb: true,
            separator: " > ".to_string(),
            max_items: 4,
        }
    }
}

/// Per-client context session state.
///
/// Stores the last computed scope hierarchy for a client's cursor position.
#[derive(Debug, Default)]
pub struct ContextSessionState {
    /// Last computed hierarchy.
    hierarchy: Option<ContextHierarchy>,
    /// Display options.
    pub options: ContextOptions,
}

impl SessionExtension for ContextSessionState {
    fn create() -> Self {
        Self::default()
    }
}

impl ContextSessionState {
    /// Set the current hierarchy.
    pub fn set_hierarchy(&mut self, hierarchy: ContextHierarchy) {
        self.hierarchy = Some(hierarchy);
    }

    /// Get the current hierarchy.
    #[must_use]
    pub const fn hierarchy(&self) -> Option<&ContextHierarchy> {
        self.hierarchy.as_ref()
    }

    /// Clear the current hierarchy.
    pub fn clear(&mut self) {
        self.hierarchy = None;
    }

    /// Format the current breadcrumb string.
    #[must_use]
    pub fn breadcrumb_text(&self) -> Option<String> {
        if !self.options.breadcrumb {
            return None;
        }
        let hierarchy = self.hierarchy.as_ref()?;
        if hierarchy.is_empty() {
            return None;
        }
        Some(hierarchy.to_breadcrumb_max(&self.options.separator, self.options.max_items))
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
