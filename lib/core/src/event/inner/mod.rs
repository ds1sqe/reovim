use std::path::PathBuf;

use crate::bind::CommandRef;
use crate::command::CommandContext;
use crate::highlight::{Highlight, HighlightGroup};
use crate::modd::Mod;

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent(WindowEvent),
    CommandEvent(CommandEvent),
    ModeChangeEvent(Mod),
    PendingKeysEvent(String),
    HighlightEvent(HighlightEvent),
    ExplorerEvent(ExplorerEvent),
    RenderSignal,
    KillSignal,
}

/// Buffer-related events
pub enum BufferEvent {
    /// Set buffer content directly
    SetContent { buffer_id: usize, content: String },
    /// Load a file into a buffer
    LoadFile { buffer_id: usize, path: PathBuf },
    /// Create a new empty buffer
    Create { buffer_id: usize },
    /// Close a buffer
    Close { buffer_id: usize },
    /// Switch to a different buffer
    Switch { buffer_id: usize },
}

/// Window-related events
pub enum WindowEvent {
    /// Toggle the explorer sidebar
    ToggleExplorer,
    /// Focus the explorer window
    FocusExplorer,
    /// Focus the editor window
    FocusEditor,
}

/// Explorer-related events
pub enum ExplorerEvent {
    /// Toggle explorer visibility
    Toggle,
    /// Open a file from the explorer
    OpenFile { path: PathBuf },
    /// Refresh the explorer tree
    Refresh,
    /// Set the explorer root directory
    SetRoot { path: PathBuf },
}

/// Highlight update events
pub enum HighlightEvent {
    /// Add highlights to a buffer
    Add {
        buffer_id: usize,
        highlights: Vec<Highlight>,
    },
    /// Clear a highlight group from a buffer
    ClearGroup {
        buffer_id: usize,
        group: HighlightGroup,
    },
    /// Clear all highlights from a buffer
    ClearAll { buffer_id: usize },
}

/// Command event to be processed by runtime
pub struct CommandEvent {
    pub command: CommandRef,
    pub context: CommandContext,
}
