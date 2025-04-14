use tokio::sync::broadcast;

mod handler;
mod key;

mod inner;
mod input;

pub use {
    handler::{PrintEventHandler, TerminateHandler},
    inner::{BufferEvent, InnerEvent},
    input::*,
};

pub use key::KeyEvent;

pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
