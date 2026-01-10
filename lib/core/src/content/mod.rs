//! Window content source types
//!
//! Defines how windows get their content - from file buffers
//! or plugin-provided buffers.
//!
//! Note: Scroll position (`buffer_anchor`) was moved to `Window::viewport.scroll`
//! as part of the Window refactoring to group viewport-related state.

mod provider;

pub use provider::{BufferContext, InputResult, PluginBufferProvider};

use std::sync::Arc;

/// Source of content for a window
#[derive(Clone)]
pub enum WindowContentSource {
    /// File-backed buffer (traditional editor window)
    FileBuffer { buffer_id: usize },

    /// Plugin-owned buffer (virtual buffer for explorer, LSP, terminal)
    PluginBuffer {
        buffer_id: usize,
        provider: Arc<dyn PluginBufferProvider>,
    },
}

impl std::fmt::Debug for WindowContentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileBuffer { buffer_id } => f
                .debug_struct("FileBuffer")
                .field("buffer_id", buffer_id)
                .finish(),
            Self::PluginBuffer { buffer_id, .. } => f
                .debug_struct("PluginBuffer")
                .field("buffer_id", buffer_id)
                .finish(),
        }
    }
}
