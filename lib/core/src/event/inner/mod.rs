use crate::bind::CommandRef;
use crate::command::CommandContext;
use crate::highlight::{Highlight, HighlightGroup};
use crate::modd::Mod;

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent,
    CommandEvent(CommandEvent),
    ModeChangeEvent(Mod),
    PendingKeysEvent(String),
    HighlightEvent(HighlightEvent),
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

pub enum BufferEvent {
    SetContent { buffer_id: usize, content: String },
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
