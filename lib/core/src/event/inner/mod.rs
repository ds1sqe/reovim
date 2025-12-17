use std::path::PathBuf;

use crate::bind::CommandRef;
use crate::command::traits::OperatorMotionAction;
use crate::command::CommandContext;
use crate::completion::CompletionItem;
use crate::highlight::{Highlight, HighlightGroup};
use crate::modd::ModeState;
use crate::telescope::{PreviewContent, TelescopeItem};

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent(WindowEvent),
    CommandEvent(CommandEvent),
    ModeChangeEvent(ModeState),
    PendingKeysEvent(String),
    HighlightEvent(HighlightEvent),
    CompletionEvent(CompletionEvent),
    ExplorerEvent(ExplorerEvent),
    /// Operator + motion action (d+motion, y+motion, c+motion)
    OperatorMotionEvent(OperatorMotionAction),
    TelescopeEvent(TelescopeEvent),
    RenderSignal,
    KillSignal,
    /// Show the which-key popup with available bindings
    WhichKeyShow {
        prefix: String,
        bindings: Vec<WhichKeyBinding>,
    },
    /// Hide the which-key popup
    WhichKeyHide,
}

/// A single binding entry for the which-key popup
#[derive(Debug, Clone)]
pub struct WhichKeyBinding {
    /// The key sequence (e.g., "g" or "gg")
    pub key: String,
    /// Description of what this key does
    pub description: String,
    /// Whether this is a prefix (has more bindings) or a terminal command
    pub is_prefix: bool,
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

/// Completion-related events
pub enum CompletionEvent {
    /// Request completion at current position
    Trigger { buffer_id: usize },
    /// Update completion items (from async completion fetch)
    Update {
        items: Vec<CompletionItem>,
        prefix: String,
        start_col: u16,
        start_row: u16,
    },
    /// Select next completion item
    SelectNext,
    /// Select previous completion item
    SelectPrev,
    /// Confirm the selected completion
    Confirm,
    /// Dismiss the completion popup
    Dismiss,
    /// Update filter as user continues typing
    UpdateFilter { new_prefix: String },
}

/// Telescope-related events
pub enum TelescopeEvent {
    /// Open telescope with a specific picker
    Open { picker: String },
    /// Update the search query
    UpdateQuery { query: String },
    /// Update items from async fetch
    UpdateItems { items: Vec<TelescopeItem> },
    /// Select next item
    SelectNext,
    /// Select previous item
    SelectPrev,
    /// Page down
    PageDown,
    /// Page up
    PageUp,
    /// Confirm selection
    Confirm,
    /// Close telescope
    Close,
    /// Update preview content
    UpdatePreview { content: PreviewContent },
}
