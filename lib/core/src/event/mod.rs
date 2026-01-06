use tokio::sync::broadcast;

mod handler;
mod key;

mod inner;
mod input;

pub use {
    handler::{CommandHandler, PrintEventHandler, TerminateHandler},
    inner::{
        BufferEvent, CommandEvent, HighlightEvent, InnerEvent, MouseButton, MouseEvent,
        MouseEventKind, SyntaxEvent, TextInputEvent, VisualTextObjectAction, WindowEvent,
    },
    input::*,
};

pub use key::KeyEvent;

pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
