pub use reovim_sys::event::{KeyCode, KeyEvent, KeyModifiers, ModifierKeyCode};

use tokio::sync::broadcast::{channel, error::SendError, Sender};

use super::Subscribe;

pub struct KeyEventBroker {
    tx: Sender<KeyEvent>,
}

const BUFFER_SIZE: usize = 255;

impl Default for KeyEventBroker {
    fn default() -> Self {
        let (tx, _) = channel(BUFFER_SIZE);
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
