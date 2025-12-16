pub use reovim_sys::event::{KeyCode, KeyEvent};

use crate::constants::KEY_EVENT_CHANNEL_CAPACITY;
use tokio::sync::broadcast::{channel, error::SendError, Sender};

use super::Subscribe;

pub struct KeyEventBroker {
    tx: Sender<KeyEvent>,
}

impl Default for KeyEventBroker {
    fn default() -> Self {
        let (tx, _) = channel(KEY_EVENT_CHANNEL_CAPACITY);
        Self { tx }
    }
}

impl KeyEventBroker {
    pub fn enlist(&self, handler: &mut impl Subscribe<KeyEvent>) {
        handler.subscribe(self.tx.subscribe());
    }

    pub fn handle(&self, ev: KeyEvent) -> Result<usize, SendError<KeyEvent>> {
        self.tx.send(ev)
    }
}
