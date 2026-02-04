//! Context update events
//!
//! Events emitted by the `ContextPlugin` when context changes.
//! Consumers subscribe to these events instead of polling `get_context()`.

use reovim_core::{context_provider::ContextHierarchy, event_bus::Event};

/// Emitted when cursor context changes
///
/// Statusline and other cursor-position-dependent UI should subscribe to this.
#[derive(Debug, Clone)]
pub struct CursorContextUpdated {
    /// Buffer where cursor moved
    pub buffer_id: usize,
    /// Current cursor line (0-indexed)
    pub line: u32,
    /// Current cursor column (0-indexed)
    pub col: u32,
    /// New context hierarchy (None if no context at this position)
    pub context: Option<ContextHierarchy>,
}

impl Event for CursorContextUpdated {
    fn priority(&self) -> u32 {
        100
    }
}

/// Emitted when viewport context changes
///
/// Sticky context headers and other viewport-dependent UI should subscribe to this.
#[derive(Debug, Clone)]
pub struct ViewportContextUpdated {
    /// Window that scrolled
    pub window_id: usize,
    /// Buffer being viewed
    pub buffer_id: usize,
    /// Top visible line (0-indexed)
    pub top_line: u32,
    /// New context hierarchy for the viewport (None if no context)
    pub context: Option<ContextHierarchy>,
}

impl Event for ViewportContextUpdated {
    fn priority(&self) -> u32 {
        100
    }
}
