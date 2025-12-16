use tokio::sync::broadcast;

mod handler;
mod key;

mod inner;
mod input;

pub use {
    handler::{CommandHandler, CompletionHandler, PrintEventHandler, TerminateHandler},
    inner::{
        BufferEvent, CommandEvent, CompletionEvent, ExplorerEvent, HighlightEvent, InnerEvent,
        WhichKeyBinding, WindowEvent,
    },
    input::*,
};

pub use key::KeyEvent;

pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
