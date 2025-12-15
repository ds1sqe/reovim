use crate::command::{Command, CommandContext};
use crate::modd::Mod;

pub enum InnerEvent {
    BufferEvent(BufferEvent),
    WindowEvent,
    CommandEvent(CommandEvent),
    ModeChangeEvent(Mod),
    RenderSignal,
    KillSignal,
}

pub enum BufferEvent {
    SetContent { buffer_id: usize, content: String },
}

/// Command event to be processed by runtime
pub struct CommandEvent {
    pub command: Command,
    pub context: CommandContext,
}
