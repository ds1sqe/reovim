//! Window content source types
//!
//! Defines how windows get their content - from file buffers
//! or plugin-provided buffers.

mod provider;

pub use provider::{BufferContext, InputResult, PluginBufferProvider};

use std::sync::Arc;

use crate::screen::window::Anchor;

/// Source of content for a window
#[derive(Clone)]
pub enum WindowContentSource {
    /// File-backed buffer (traditional editor window)
    FileBuffer {
        buffer_id: usize,
        buffer_anchor: Anchor,
    },

    /// Plugin-owned buffer (virtual buffer for explorer, LSP, terminal)
    PluginBuffer {
        buffer_id: usize,
        buffer_anchor: Anchor,
        provider: Arc<dyn PluginBufferProvider>,
    },
}

impl std::fmt::Debug for WindowContentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileBuffer {
                buffer_id,
                buffer_anchor,
            } => f
                .debug_struct("FileBuffer")
                .field("buffer_id", buffer_id)
                .field("buffer_anchor", buffer_anchor)
                .finish(),
            Self::PluginBuffer {
                buffer_id,
                buffer_anchor,
                ..
            } => f
                .debug_struct("PluginBuffer")
                .field("buffer_id", buffer_id)
                .field("buffer_anchor", buffer_anchor)
                .finish(),
        }
    }
}
