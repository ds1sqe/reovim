//! Window content source types
//!
//! Defines how windows get their content - from file buffers,
//! plugin-provided buffers, or direct overlay rendering.

mod provider;

pub use provider::{BufferContext, InputResult, PluginBufferProvider};

use std::sync::{Arc, RwLock};

use crate::{overlay::OverlayRenderer, screen::window::Anchor};

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

    /// Direct overlay rendering (lightweight UI like completion popup)
    Overlay {
        overlay_id: &'static str,
        renderer: Arc<RwLock<dyn OverlayRenderer>>,
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
            Self::Overlay { overlay_id, .. } => f
                .debug_struct("Overlay")
                .field("overlay_id", overlay_id)
                .finish(),
        }
    }
}
