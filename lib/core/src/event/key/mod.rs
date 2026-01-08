pub use reovim_sys::event::{KeyCode, KeyEvent};

use {
    crate::{constants::KEY_EVENT_CHANNEL_CAPACITY, event_bus::EventScope},
    tokio::sync::broadcast::{Sender, channel, error::SendError},
};

use super::Subscribe;

/// Key event with optional scope for lifecycle tracking.
///
/// When scope is present, the scope's counter should be decremented
/// after all effects of this key event have been processed.
#[derive(Debug, Clone)]
pub struct ScopedKeyEvent {
    pub key: KeyEvent,
    pub scope: Option<EventScope>,
}

impl ScopedKeyEvent {
    /// Create a new scoped key event without a scope.
    #[must_use]
    pub const fn new(key: KeyEvent) -> Self {
        Self { key, scope: None }
    }

    /// Create a new scoped key event with the given scope.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Some() not const-stable
    pub fn with_scope(key: KeyEvent, scope: EventScope) -> Self {
        Self {
            key,
            scope: Some(scope),
        }
    }

    /// Take the scope from this event, leaving None in its place.
    #[allow(clippy::missing_const_for_fn)] // Option::take() not const-stable
    pub fn take_scope(&mut self) -> Option<EventScope> {
        self.scope.take()
    }
}

pub struct KeyEventBroker {
    tx: Sender<ScopedKeyEvent>,
}

impl Default for KeyEventBroker {
    fn default() -> Self {
        let (tx, _) = channel(KEY_EVENT_CHANNEL_CAPACITY);
        Self { tx }
    }
}

impl KeyEventBroker {
    pub fn enlist(&self, handler: &mut impl Subscribe<ScopedKeyEvent>) {
        handler.subscribe(self.tx.subscribe());
    }

    /// Send a key event to all subscribers.
    ///
    /// # Errors
    ///
    /// Returns `SendError` if there are no active receivers.
    pub fn handle(&self, ev: ScopedKeyEvent) -> Result<usize, SendError<ScopedKeyEvent>> {
        self.tx.send(ev)
    }
}
