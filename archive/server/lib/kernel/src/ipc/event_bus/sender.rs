//! Cloneable sender handle for emitting events.

use super::super::{
    channel::BoundedSender,
    event::{DynEvent, Event},
    scope::EventScope,
};

/// Cloneable sender handle for emitting events via channel.
///
/// Obtained from `EventBus::sender()` when the bus is created with
/// `new_with_channel()`.
///
/// # Thread Safety
///
/// `EventSender` is `Clone`, `Send`, and `Sync`. Multiple threads can
/// send events concurrently.
#[derive(Clone)]
pub struct EventSender {
    pub(super) tx: BoundedSender<DynEvent>,
}

impl EventSender {
    /// Send an event (blocking if channel is full).
    ///
    /// This will block if the channel is full until space is available.
    pub fn send<E: Event>(&self, event: E) {
        let _ = self.tx.send(DynEvent::new(event));
    }

    /// Try to send an event without blocking.
    ///
    /// Returns immediately even if the channel is full.
    pub fn try_send<E: Event>(&self, event: E) {
        let _ = self.tx.try_send(DynEvent::new(event));
    }

    /// Send a pre-boxed dynamic event.
    pub fn send_dyn(&self, event: DynEvent) {
        let _ = self.tx.try_send(event);
    }

    /// Send an event with scope tracking.
    ///
    /// The scope is incremented before sending.
    pub fn send_scoped<E: Event>(&self, event: E, scope: &EventScope) {
        scope.increment();
        let dyn_event = DynEvent::new(event).with_scope(scope.clone());
        let _ = self.tx.try_send(dyn_event);
    }
}
