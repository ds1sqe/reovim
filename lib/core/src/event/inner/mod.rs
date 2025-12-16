use crate::command::{Command, CommandContext};
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
    pub command: Command,
    pub context: CommandContext,
}
