use tokio::sync::broadcast;

mod handler;
pub mod key;

mod inner;
mod input;

pub use {
    handler::{CommandHandler, PrintEventHandler, TerminateHandler},
    inner::{
        BufferEvent, CommandEvent, EditingEvent, FileEvent, HighlightEvent, InputEvent, ModeEvent,
        MouseButton, MouseEvent, MouseEventKind, PluginEventData, RenderEvent, RpcEvent,
        RuntimeEvent, RuntimeEventPayload, SettingsEvent, SyntaxEvent, TextInputEvent,
        VisualTextObjectAction, WindowEvent,
    },
    input::*,
};

pub use key::{KeyEvent, ScopedKeyEvent};

pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
