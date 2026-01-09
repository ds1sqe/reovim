//! Statusline section types
//!
//! Defines section content and configuration for plugins to register.

use std::sync::Arc;

use reovim_core::{highlight::Style, plugin::SectionAlignment};

// Re-export from core
pub use reovim_core::plugin::PluginStateRegistry;

/// Context passed to section render callbacks
pub struct SectionRenderContext<'a> {
    /// Plugin state registry for accessing plugin state
    pub plugin_state: &'a PluginStateRegistry,
    /// Active buffer ID (if any buffer is open)
    pub active_buffer_id: Option<usize>,
    /// Active buffer content (if any buffer is open)
    pub buffer_content: Option<&'a str>,
    /// Cursor row in active buffer (if any buffer is open)
    pub cursor_row: Option<u32>,
    /// Cursor column in active buffer (if any buffer is open)
    pub cursor_col: Option<u32>,
}

/// Content returned by a section render callback
#[derive(Debug, Clone)]
pub struct SectionContent {
    /// Text content to display
    pub text: String,
    /// Optional style (uses statusline background if None)
    pub style: Option<Style>,
    /// Whether to show this section
    pub visible: bool,
}

impl SectionContent {
    /// Create a new visible section content
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            visible: true,
        }
    }

    /// Create a hidden section
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            text: String::new(),
            style: None,
            visible: false,
        }
    }

    /// Set the style
    #[must_use]
    pub const fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }
}

/// A registered statusline section
pub struct StatuslineSection {
    /// Unique identifier for this section
    pub id: &'static str,
    /// Priority within alignment group (lower = first)
    pub priority: u32,
    /// Where to align this section
    pub alignment: SectionAlignment,
    /// Render callback
    pub render: Arc<dyn Fn(&SectionRenderContext) -> SectionContent + Send + Sync>,
}

impl std::fmt::Debug for StatuslineSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatuslineSection")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("alignment", &self.alignment)
            .finish_non_exhaustive()
    }
}

impl Clone for StatuslineSection {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            priority: self.priority,
            alignment: self.alignment,
            render: Arc::clone(&self.render),
        }
    }
}

impl StatuslineSection {
    /// Create a new section
    pub fn new<F>(id: &'static str, priority: u32, alignment: SectionAlignment, render: F) -> Self
    where
        F: Fn(&SectionRenderContext) -> SectionContent + Send + Sync + 'static,
    {
        Self {
            id,
            priority,
            alignment,
            render: Arc::new(render),
        }
    }
}
