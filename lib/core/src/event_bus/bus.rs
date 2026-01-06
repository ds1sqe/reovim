//! `EventBus` implementation for type-erased event dispatch

#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::option_if_let_else)]

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tokio::sync::mpsc;

use super::{core_events::RequestModeChange, DynEvent, Event, EventResult};
use crate::modd::{ComponentId, EditMode, ModeState, SubMode};

/// Handler function type - receives event and context, returns result
type EventHandlerFn = Box<dyn Fn(&DynEvent, &mut HandlerContext) -> EventResult + Send + Sync>;

/// A registered event handler with its priority
struct RegisteredHandler {
    priority: u32,
    handler: EventHandlerFn,
}

/// Context passed to event handlers
///
/// Provides handlers with access to emit new events and request renders.
pub struct HandlerContext<'a> {
    /// Event sender for emitting new events
    event_tx: &'a EventSender,
    /// Flag to track if render was requested
    render_requested: bool,
    /// Flag to track if quit was requested
    quit_requested: bool,
}

impl<'a> HandlerContext<'a> {
    /// Create a new handler context
    #[allow(dead_code)] // Will be used by runtime integration
    pub(crate) fn new(event_tx: &'a EventSender) -> Self {
        Self {
            event_tx,
            render_requested: false,
            quit_requested: false,
        }
    }

    /// Emit a new event from within a handler
    pub fn emit<E: Event>(&self, event: E) {
        self.event_tx.try_send(event);
    }

    /// Request a render after event processing
    pub fn request_render(&mut self) {
        self.render_requested = true;
    }

    /// Request the editor to quit
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Check if render was requested
    #[must_use]
    pub fn render_requested(&self) -> bool {
        self.render_requested
    }

    /// Check if quit was requested
    #[must_use]
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    // === Mode change helpers ===

    /// Enter interactor mode: sets both focus and SubMode to component_id
    ///
    /// This is the common pattern for plugins that want to receive text input.
    /// It sets the interactor_id to the component and also sets SubMode::Interactor
    /// to route input events to the plugin.
    pub fn enter_interactor_mode(&self, component_id: ComponentId) {
        let mode = ModeState::with_interactor_id_sub_mode(
            component_id,
            EditMode::Normal,
            SubMode::Interactor(component_id),
        );
        self.emit(RequestModeChange { mode });
    }

    /// Return to normal editor mode
    ///
    /// Resets to standard editor Normal mode with no sub-mode.
    pub fn exit_to_normal(&self) {
        self.emit(RequestModeChange {
            mode: ModeState::normal(),
        });
    }

    /// Set arbitrary mode state
    ///
    /// For edge cases that don't fit the standard patterns.
    pub fn set_mode(&self, mode: ModeState) {
        self.emit(RequestModeChange { mode });
    }
}

/// Cloneable sender for emitting events to the bus
#[derive(Clone)]
pub struct EventSender {
    tx: mpsc::Sender<DynEvent>,
}

impl EventSender {
    /// Send an event asynchronously
    pub async fn send<E: Event>(&self, event: E) {
        let _ = self.tx.send(DynEvent::new(event)).await;
    }

    /// Try to send an event without blocking
    pub fn try_send<E: Event>(&self, event: E) {
        let _ = self.tx.try_send(DynEvent::new(event));
    }

    /// Send a pre-boxed dynamic event
    pub fn send_dyn(&self, event: DynEvent) {
        let _ = self.tx.try_send(event);
    }
}

/// The event bus that coordinates event dispatch
///
/// Handlers are registered by event type and called in priority order
/// when matching events are dispatched.
pub struct EventBus {
    /// Handlers indexed by event TypeId, sorted by priority
    handlers: RwLock<HashMap<TypeId, Vec<RegisteredHandler>>>,
    /// Channel for sending events
    tx: mpsc::Sender<DynEvent>,
    /// Channel for receiving events
    rx: RwLock<Option<mpsc::Receiver<DynEvent>>>,
}

impl EventBus {
    /// Create a new event bus with the specified channel capacity
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self {
            handlers: RwLock::new(HashMap::new()),
            tx,
            rx: RwLock::new(Some(rx)),
        }
    }

    /// Take the receiver for use in the event loop
    ///
    /// This can only be called once; subsequent calls return None.
    pub fn take_receiver(&self) -> Option<mpsc::Receiver<DynEvent>> {
        self.rx.write().unwrap().take()
    }

    /// Get an event sender for emitting events
    #[must_use]
    pub fn sender(&self) -> EventSender {
        EventSender {
            tx: self.tx.clone(),
        }
    }

    /// Register a handler for a specific event type
    ///
    /// # Arguments
    ///
    /// * `priority` - Lower values are called first (0-50 for core, 100+ for plugins)
    /// * `handler` - Function to call when event is received
    ///
    /// # Example
    ///
    /// ```ignore
    /// bus.subscribe::<MyEvent, _>(100, |event, ctx| {
    ///     // Handle the event
    ///     EventResult::Handled
    /// });
    /// ```
    pub fn subscribe<E: Event, F>(&self, priority: u32, handler: F)
    where
        F: Fn(&E, &mut HandlerContext) -> EventResult + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<E>();

        // Wrap the typed handler in a type-erased handler
        let wrapped: EventHandlerFn = Box::new(move |event, ctx| {
            if let Some(e) = event.downcast_ref::<E>() {
                handler(e, ctx)
            } else {
                EventResult::NotHandled
            }
        });

        let mut handlers = self.handlers.write().unwrap();
        let entry = handlers.entry(type_id).or_default();
        entry.push(RegisteredHandler {
            priority,
            handler: wrapped,
        });
        // Sort by priority (lower = earlier)
        entry.sort_by_key(|h| h.priority);
    }

    /// Emit an event to the bus (non-blocking)
    pub fn emit<E: Event>(&self, event: E) {
        let _ = self.tx.try_send(DynEvent::new(event));
    }

    /// Emit a pre-boxed dynamic event to the bus (non-blocking)
    pub fn emit_dyn(&self, event: DynEvent) {
        let _ = self.tx.try_send(event);
    }

    /// Emit an event to the bus (async)
    pub async fn emit_async<E: Event>(&self, event: E) {
        let _ = self.tx.send(DynEvent::new(event)).await;
    }

    /// Dispatch an event to all registered handlers
    ///
    /// Handlers are called in priority order. Processing stops if a handler
    /// returns `EventResult::Consumed` or `EventResult::Quit`.
    ///
    /// Returns the final result after all handlers have processed.
    pub fn dispatch(&self, event: &DynEvent, ctx: &mut HandlerContext) -> EventResult {
        let handlers = self.handlers.read().unwrap();

        if let Some(type_handlers) = handlers.get(&event.type_id()) {
            let mut final_result = EventResult::NotHandled;

            for registered in type_handlers {
                let result = (registered.handler)(event, ctx);

                match result {
                    EventResult::Consumed | EventResult::Quit => {
                        return result;
                    }
                    EventResult::NeedsRender => {
                        ctx.request_render();
                        final_result = EventResult::Handled;
                    }
                    EventResult::Handled => {
                        final_result = EventResult::Handled;
                    }
                    EventResult::NotHandled => {}
                }
            }

            final_result
        } else {
            EventResult::NotHandled
        }
    }

    /// Check if any handlers are registered for an event type
    #[must_use]
    pub fn has_handlers<E: Event>(&self) -> bool {
        let handlers = self.handlers.read().unwrap();
        handlers
            .get(&TypeId::of::<E>())
            .is_some_and(|h| !h.is_empty())
    }

    /// Get the number of handlers registered for an event type
    #[must_use]
    pub fn handler_count<E: Event>(&self) -> usize {
        let handlers = self.handlers.read().unwrap();
        handlers
            .get(&TypeId::of::<E>())
            .map_or(0, std::vec::Vec::len)
    }

    /// Clear all handlers (mainly for testing)
    pub fn clear(&self) {
        self.handlers.write().unwrap().clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Thread-safe wrapper for EventBus
#[allow(dead_code)]
pub type SharedEventBus = Arc<EventBus>;

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicI32, Ordering},
    };

    #[derive(Debug)]
    struct TestEvent {
        value: i32,
    }
    impl Event for TestEvent {}

    #[test]
    fn test_subscribe_and_dispatch() {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        bus.subscribe::<TestEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::SeqCst);
            EventResult::Handled
        });

        let event = DynEvent::new(TestEvent { value: 5 });
        let sender = bus.sender();
        let mut ctx = HandlerContext::new(&sender);
        let result = bus.dispatch(&event, &mut ctx);

        assert_eq!(result, EventResult::Handled);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_priority_ordering() {
        let bus = EventBus::new(16);
        let order = Arc::new(RwLock::new(Vec::new()));

        let order1 = order.clone();
        bus.subscribe::<TestEvent, _>(200, move |_, _| {
            order1.write().unwrap().push(200);
            EventResult::Handled
        });

        let order2 = order.clone();
        bus.subscribe::<TestEvent, _>(50, move |_, _| {
            order2.write().unwrap().push(50);
            EventResult::Handled
        });

        let order3 = order.clone();
        bus.subscribe::<TestEvent, _>(100, move |_, _| {
            order3.write().unwrap().push(100);
            EventResult::Handled
        });

        let event = DynEvent::new(TestEvent { value: 1 });
        let sender = bus.sender();
        let mut ctx = HandlerContext::new(&sender);
        bus.dispatch(&event, &mut ctx);

        let order_vec = order.read().unwrap();
        assert_eq!(*order_vec, vec![50, 100, 200]);
    }

    #[test]
    fn test_consumed_stops_propagation() {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicI32::new(0));

        let counter1 = counter.clone();
        bus.subscribe::<TestEvent, _>(50, move |_, _| {
            counter1.fetch_add(1, Ordering::SeqCst);
            EventResult::Consumed
        });

        let counter2 = counter.clone();
        bus.subscribe::<TestEvent, _>(100, move |_, _| {
            counter2.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        let event = DynEvent::new(TestEvent { value: 1 });
        let sender = bus.sender();
        let mut ctx = HandlerContext::new(&sender);
        let result = bus.dispatch(&event, &mut ctx);

        assert_eq!(result, EventResult::Consumed);
        // Only first handler should have run
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_no_handlers() {
        let bus = EventBus::new(16);

        let event = DynEvent::new(TestEvent { value: 1 });
        let sender = bus.sender();
        let mut ctx = HandlerContext::new(&sender);
        let result = bus.dispatch(&event, &mut ctx);

        assert_eq!(result, EventResult::NotHandled);
    }
}
