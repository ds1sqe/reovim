//! Event bus for type-erased event dispatch.
//!
//! The `EventBus` is the central hub for event-driven communication between
//! kernel subsystems, drivers, and modules. It provides:
//!
//! - **Type-erased dispatch**: Single bus handles all event types
//! - **Lock-free hot path**: Dispatch uses `ArcSwap` for zero-lock reads
//! - **Priority ordering**: Handlers execute in priority order (lower = first)
//! - **RAII subscriptions**: Handlers auto-unsubscribe when dropped
//! - **Scoped events**: Track event lifecycle for synchronization
//!
//! # Design Philosophy
//!
//! Following the proven patterns from `lib/core`:
//! - **Lock-free dispatch**: `ArcSwap::load()` for handler lookup
//! - **RCU for subscriptions**: Copy-on-write via `ArcSwap::rcu()`
//! - **Fire-and-forget**: Handlers return `EventResult`, not `Result<T, E>`
//!
//! # Performance
//!
//! - Dispatch latency: ~100ns (0 handlers) to ~1µs (10 handlers)
//! - Subscription: ~1µs (RCU clone + sort)
//! - Handler lookup: O(1) `HashMap` + O(n) handler iteration
//!
//! # Example
//!
//! ```
//! use reovim_kernel::ipc::{EventBus, Event, EventResult};
//!
//! #[derive(Debug)]
//! struct BufferChanged { buffer_id: u64 }
//! impl Event for BufferChanged {}
//!
//! let bus = EventBus::new();
//!
//! // Subscribe with priority 100 (default)
//! let _sub = bus.subscribe::<BufferChanged, _>(100, |event| {
//!     println!("Buffer {} changed", event.buffer_id);
//!     EventResult::Handled
//! });
//!
//! // Emit event - handler is called synchronously
//! bus.emit(BufferChanged { buffer_id: 1 });
//! ```

use std::{any::TypeId, collections::HashMap, sync::Arc};

use {arc_swap::ArcSwap, parking_lot::Mutex};

use super::{
    event::{DynEvent, Event, EventResult},
    scope::EventScope,
    subscription::{Subscription, SubscriptionId},
};

/// Handler function type for event dispatch.
///
/// Handlers receive a reference to the `DynEvent` and return an `EventResult`.
/// The handler must downcast to the specific event type internally.
type HandlerFn = Arc<dyn Fn(&DynEvent) -> EventResult + Send + Sync>;

/// Registered handler with metadata.
struct RegisteredHandler {
    /// Unique subscription ID.
    id: SubscriptionId,

    /// Handler priority (lower = earlier dispatch).
    priority: u32,

    /// The handler function.
    handler: HandlerFn,
}

/// Internal state for `EventBus`.
struct EventBusInner {
    /// Handler storage: `TypeId` -> sorted Vec of handlers.
    /// Uses `ArcSwap` for lock-free reads.
    handlers: ArcSwap<HashMap<TypeId, Vec<RegisteredHandler>>>,

    /// Async event queue for `emit_async()`.
    queue: Mutex<Vec<DynEvent>>,
}

/// Type-erased event bus for pub/sub communication.
///
/// The `EventBus` routes events to registered handlers based on event type.
/// It uses lock-free data structures for the hot dispatch path and
/// copy-on-write for subscription updates.
///
/// # Thread Safety
///
/// `EventBus` is `Clone`, `Send`, and `Sync`. Cloning creates a new handle
/// to the same underlying bus. Multiple threads can dispatch events
/// concurrently without blocking.
///
/// # Handler Ordering
///
/// Handlers are called in priority order (lower priority number = earlier).
/// For handlers with the same priority, registration order is preserved.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl EventBus {
    /// Create a new empty event bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                handlers: ArcSwap::from_pointee(HashMap::new()),
                queue: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Subscribe a handler for events of type `E`.
    ///
    /// Returns a `Subscription` handle that unsubscribes when dropped.
    ///
    /// # Arguments
    ///
    /// * `priority` - Handler priority (lower = called earlier). Convention:
    ///   - 0-50: Core/critical handlers
    ///   - 100: Default priority
    ///   - 200+: Low priority (cleanup, logging)
    /// * `handler` - Function called for each event of type `E`
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::ipc::{EventBus, Event, EventResult};
    ///
    /// #[derive(Debug)]
    /// struct MyEvent { value: i32 }
    /// impl Event for MyEvent {}
    ///
    /// let bus = EventBus::new();
    ///
    /// let sub = bus.subscribe::<MyEvent, _>(100, |event| {
    ///     println!("Received: {:?}", event.value);
    ///     EventResult::Handled
    /// });
    ///
    /// // Handler is active while `sub` is alive
    /// bus.emit(MyEvent { value: 42 });
    ///
    /// // Dropping `sub` removes the handler
    /// drop(sub);
    /// ```
    pub fn subscribe<E, F>(&self, priority: u32, handler: F) -> Subscription
    where
        E: Event,
        F: Fn(&E) -> EventResult + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<E>();
        let sub_id = SubscriptionId::new();

        // Wrap handler to downcast from DynEvent
        let wrapped_handler: HandlerFn = Arc::new(move |dyn_event: &DynEvent| {
            dyn_event
                .downcast_ref::<E>()
                .map_or(EventResult::NotHandled, &handler)
        });

        let registered = RegisteredHandler {
            id: sub_id,
            priority,
            handler: wrapped_handler,
        };

        // RCU update: clone, modify, swap
        // Note: rcu takes FnMut, so we need to clone registered
        self.inner.handlers.rcu(|current| {
            let mut new_map = (**current).clone();
            let handlers = new_map.entry(type_id).or_default();
            handlers.push(registered.clone());
            // Sort by priority (stable sort preserves registration order for same priority)
            handlers.sort_by_key(|h| h.priority);
            new_map
        });

        // Create unsubscribe closure that captures the inner Arc
        let inner = Arc::clone(&self.inner);
        let unsubscribe = move || {
            inner.handlers.rcu(|current| {
                let mut new_map = (**current).clone();
                if let Some(handlers) = new_map.get_mut(&type_id) {
                    handlers.retain(|h| h.id != sub_id);
                    if handlers.is_empty() {
                        new_map.remove(&type_id);
                    }
                }
                new_map
            });
        };

        Subscription::new::<E>(sub_id, unsubscribe)
    }

    /// Emit an event synchronously.
    ///
    /// Dispatches the event to all registered handlers in priority order.
    /// Returns the combined result of all handlers.
    ///
    /// # Handler Behavior
    ///
    /// - Handlers are called in priority order (lower = first)
    /// - If any handler returns `Consumed`, dispatch stops
    /// - Otherwise continues until all handlers have been called
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::ipc::{EventBus, Event, EventResult};
    ///
    /// #[derive(Debug)]
    /// struct MyEvent;
    /// impl Event for MyEvent {}
    ///
    /// let bus = EventBus::new();
    /// let result = bus.emit(MyEvent);
    /// assert!(result.is_not_handled()); // No handlers registered
    /// ```
    pub fn emit<E: Event>(&self, event: E) -> EventResult {
        let dyn_event = DynEvent::new(event);
        self.dispatch(&dyn_event)
    }

    /// Emit an event with scope tracking.
    ///
    /// The scope's counter is incremented before dispatch and decremented
    /// after all handlers complete. This allows callers to wait for all
    /// effects of the event to complete.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::ipc::{EventBus, Event, EventResult, EventScope};
    ///
    /// #[derive(Debug)]
    /// struct MyEvent;
    /// impl Event for MyEvent {}
    ///
    /// let bus = EventBus::new();
    /// let scope = EventScope::new();
    ///
    /// scope.increment(); // Manually increment before emit_scoped
    /// let result = bus.emit_scoped(MyEvent, &scope);
    /// // Scope counter is now decremented
    /// ```
    pub fn emit_scoped<E: Event>(&self, event: E, scope: &EventScope) -> EventResult {
        let dyn_event = DynEvent::new(event).with_scope(scope.clone());
        let result = self.dispatch(&dyn_event);

        // Decrement scope after dispatch completes
        scope.decrement();

        result
    }

    /// Queue an event for later processing.
    ///
    /// The event is stored in an internal queue and processed when
    /// `process_queue()` is called. This is useful for deferring
    /// event dispatch to avoid reentrancy issues.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::ipc::{EventBus, Event, EventResult};
    ///
    /// #[derive(Debug)]
    /// struct MyEvent;
    /// impl Event for MyEvent {}
    ///
    /// let bus = EventBus::new();
    ///
    /// bus.emit_async(MyEvent);
    /// bus.emit_async(MyEvent);
    ///
    /// // Events are queued but not dispatched yet
    /// let count = bus.process_queue();
    /// assert_eq!(count, 2);
    /// ```
    pub fn emit_async<E: Event>(&self, event: E) {
        let dyn_event = DynEvent::new(event);
        self.inner.queue.lock().push(dyn_event);
    }

    /// Queue an event with scope tracking for later processing.
    ///
    /// Like `emit_async`, but attaches a scope for lifecycle tracking.
    /// The scope is incremented when queued and decremented after dispatch.
    pub fn emit_async_scoped<E: Event>(&self, event: E, scope: &EventScope) {
        scope.increment();
        let dyn_event = DynEvent::new(event).with_scope(scope.clone());
        self.inner.queue.lock().push(dyn_event);
    }

    /// Process all queued events.
    ///
    /// Drains the queue and dispatches each event synchronously.
    /// Returns the number of events processed.
    ///
    /// # Scope Handling
    ///
    /// For events with attached scopes, the scope is decremented after
    /// each event is dispatched.
    #[must_use]
    pub fn process_queue(&self) -> usize {
        let events: Vec<_> = {
            let mut queue = self.inner.queue.lock();
            std::mem::take(&mut *queue)
        };

        let count = events.len();

        for mut event in events {
            let scope = event.take_scope();
            let result = self.dispatch(&event);
            tracing::trace!(event_type = event.type_name(), ?result, "processed queued event");
            if let Some(scope) = scope {
                scope.decrement();
            }
        }

        count
    }

    /// Dispatch a type-erased event to registered handlers.
    ///
    /// This is the core dispatch logic, used by both `emit` and `process_queue`.
    #[must_use]
    pub fn dispatch(&self, event: &DynEvent) -> EventResult {
        // Lock-free read of handlers
        let handlers = self.inner.handlers.load();
        let type_id = event.type_id();

        let Some(handlers) = handlers.get(&type_id) else {
            return EventResult::NotHandled;
        };

        let mut result = EventResult::NotHandled;

        for handler in handlers {
            let handler_result = (handler.handler)(event);

            match handler_result {
                EventResult::Consumed => {
                    return EventResult::Consumed;
                }
                EventResult::Handled => {
                    result = EventResult::Handled;
                }
                EventResult::NotHandled => {
                    // Continue to next handler
                }
            }
        }

        result
    }

    /// Get the number of handlers registered for a specific event type.
    ///
    /// Useful for debugging and testing.
    #[must_use]
    pub fn handler_count<E: Event>(&self) -> usize {
        let handlers = self.inner.handlers.load();
        handlers.get(&TypeId::of::<E>()).map_or(0, Vec::len)
    }

    /// Get the total number of handlers registered across all event types.
    #[must_use]
    pub fn total_handler_count(&self) -> usize {
        let handlers = self.inner.handlers.load();
        handlers.values().map(Vec::len).sum()
    }

    /// Get the number of events in the async queue.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.inner.queue.lock().len()
    }

    /// Check if the async queue is empty.
    #[must_use]
    pub fn queue_is_empty(&self) -> bool {
        self.inner.queue.lock().is_empty()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let handlers = self.inner.handlers.load();
        f.debug_struct("EventBus")
            .field("event_types", &handlers.len())
            .field("total_handlers", &self.total_handler_count())
            .field("queue_len", &self.queue_len())
            .finish()
    }
}

// Clone the handlers map for RegisteredHandler
impl Clone for RegisteredHandler {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            priority: self.priority,
            handler: self.handler.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            sync::atomic::{AtomicI32, AtomicU32, Ordering},
            thread,
        },
    };

    #[derive(Debug)]
    struct TestEvent {
        value: i32,
    }

    impl Event for TestEvent {
        fn priority(&self) -> u32 {
            50
        }
    }

    #[derive(Debug)]
    struct OtherEvent;
    impl Event for OtherEvent {}

    // ========== Basic tests ==========

    #[test]
    fn test_event_bus_new() {
        let bus = EventBus::new();
        assert_eq!(bus.total_handler_count(), 0);
        assert!(bus.queue_is_empty());
    }

    #[test]
    fn test_event_bus_subscribe() {
        let bus = EventBus::new();
        let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
        assert_eq!(bus.handler_count::<TestEvent>(), 1);
    }

    #[test]
    fn test_event_bus_unsubscribe_on_drop() {
        let bus = EventBus::new();
        {
            let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
            assert_eq!(bus.handler_count::<TestEvent>(), 1);
        }
        assert_eq!(bus.handler_count::<TestEvent>(), 0);
    }

    #[test]
    fn test_event_bus_emit() {
        let bus = EventBus::new();
        let received = Arc::new(AtomicI32::new(0));
        let received2 = received.clone();

        let _sub = bus.subscribe::<TestEvent, _>(100, move |event| {
            received2.store(event.value, Ordering::SeqCst);
            EventResult::Handled
        });

        let result = bus.emit(TestEvent { value: 42 });
        assert!(result.is_handled());
        assert_eq!(received.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_event_bus_emit_no_handlers() {
        let bus = EventBus::new();
        let result = bus.emit(TestEvent { value: 42 });
        assert!(result.is_not_handled());
    }

    #[test]
    fn test_event_bus_emit_wrong_type() {
        let bus = EventBus::new();
        let called = Arc::new(AtomicU32::new(0));
        let called2 = called.clone();

        let _sub = bus.subscribe::<TestEvent, _>(100, move |_| {
            called2.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        // Emit different event type
        bus.emit(OtherEvent);
        assert_eq!(called.load(Ordering::SeqCst), 0);
    }

    // ========== Priority ordering tests ==========

    #[test]
    fn test_event_bus_priority_ordering() {
        let bus = EventBus::new();
        let order = Arc::new(Mutex::new(Vec::new()));

        let order1 = order.clone();
        let _sub1 = bus.subscribe::<TestEvent, _>(200, move |_| {
            order1.lock().push(200);
            EventResult::Handled
        });

        let order2 = order.clone();
        let _sub2 = bus.subscribe::<TestEvent, _>(50, move |_| {
            order2.lock().push(50);
            EventResult::Handled
        });

        let order3 = order.clone();
        let _sub3 = bus.subscribe::<TestEvent, _>(100, move |_| {
            order3.lock().push(100);
            EventResult::Handled
        });

        bus.emit(TestEvent { value: 1 });

        let order_result = order.lock().clone();
        assert_eq!(order_result, vec![50, 100, 200]);
    }

    // ========== Consumed stops propagation ==========

    #[test]
    fn test_event_bus_consumed_stops_propagation() {
        let bus = EventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));

        let count1 = call_count.clone();
        let _sub1 = bus.subscribe::<TestEvent, _>(50, move |_| {
            count1.fetch_add(1, Ordering::SeqCst);
            EventResult::Consumed // Stop propagation
        });

        let count2 = call_count.clone();
        let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
            count2.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        let result = bus.emit(TestEvent { value: 1 });
        assert!(result.is_consumed());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    // ========== Async queue tests ==========

    #[test]
    fn test_event_bus_emit_async() {
        let bus = EventBus::new();

        bus.emit_async(TestEvent { value: 1 });
        bus.emit_async(TestEvent { value: 2 });

        assert_eq!(bus.queue_len(), 2);
    }

    #[test]
    fn test_event_bus_process_queue() {
        let bus = EventBus::new();
        let sum = Arc::new(AtomicI32::new(0));
        let sum2 = sum.clone();

        let _sub = bus.subscribe::<TestEvent, _>(100, move |event| {
            sum2.fetch_add(event.value, Ordering::SeqCst);
            EventResult::Handled
        });

        bus.emit_async(TestEvent { value: 10 });
        bus.emit_async(TestEvent { value: 20 });
        bus.emit_async(TestEvent { value: 30 });

        assert_eq!(sum.load(Ordering::SeqCst), 0); // Not processed yet

        let count = bus.process_queue();
        assert_eq!(count, 3);
        assert_eq!(sum.load(Ordering::SeqCst), 60);
        assert!(bus.queue_is_empty());
    }

    // ========== Scoped event tests ==========

    #[test]
    fn test_event_bus_emit_scoped() {
        let bus = EventBus::new();
        let scope = EventScope::new();

        scope.increment();
        assert_eq!(scope.in_flight(), 1);

        bus.emit_scoped(TestEvent { value: 1 }, &scope);
        assert_eq!(scope.in_flight(), 0);
    }

    #[test]
    fn test_event_bus_emit_async_scoped() {
        let bus = EventBus::new();
        let scope = EventScope::new();

        bus.emit_async_scoped(TestEvent { value: 1 }, &scope);
        assert_eq!(scope.in_flight(), 1);

        let count = bus.process_queue();
        assert_eq!(count, 1);
        assert_eq!(scope.in_flight(), 0);
    }

    // ========== Multiple subscriptions ==========

    #[test]
    fn test_event_bus_multiple_subscriptions() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicU32::new(0));

        let count1 = count.clone();
        let _sub1 = bus.subscribe::<TestEvent, _>(100, move |_| {
            count1.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        let count2 = count.clone();
        let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
            count2.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        bus.emit(TestEvent { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_bus_partial_unsubscribe() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicU32::new(0));

        let count1 = count.clone();
        let sub1 = bus.subscribe::<TestEvent, _>(100, move |_| {
            count1.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        let count2 = count.clone();
        let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
            count2.fetch_add(10, Ordering::SeqCst);
            EventResult::Handled
        });

        bus.emit(TestEvent { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 11);

        drop(sub1);

        count.store(0, Ordering::SeqCst);
        bus.emit(TestEvent { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 10);
    }

    // ========== Thread safety tests ==========

    #[test]
    fn test_event_bus_concurrent_emit() {
        let bus = Arc::new(EventBus::new());
        let count = Arc::new(AtomicU32::new(0));

        let count_handler = count.clone();
        let _sub = bus.subscribe::<TestEvent, _>(100, move |_| {
            count_handler.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        let mut handles = vec![];
        for i in 0..10 {
            let bus = bus.clone();
            handles.push(thread::spawn(move || {
                bus.emit(TestEvent { value: i });
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(count.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_event_bus_concurrent_subscribe_emit() {
        let bus = Arc::new(EventBus::new());

        let mut handles = vec![];

        // Subscribe from multiple threads
        for _ in 0..5 {
            let bus = bus.clone();
            handles.push(thread::spawn(move || {
                let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
                // Keep subscription alive briefly
                thread::sleep(std::time::Duration::from_millis(10));
            }));
        }

        // Emit from multiple threads
        for i in 0..5 {
            let bus = bus.clone();
            handles.push(thread::spawn(move || {
                bus.emit(TestEvent { value: i });
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    // ========== Debug and Default tests ==========

    #[test]
    fn test_event_bus_debug() {
        let bus = EventBus::new();
        let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
        let debug_str = format!("{bus:?}");
        assert!(debug_str.contains("EventBus"));
        assert!(debug_str.contains("total_handlers"));
    }

    #[test]
    fn test_event_bus_default() {
        let bus = EventBus::default();
        assert_eq!(bus.total_handler_count(), 0);
    }

    #[test]
    fn test_event_bus_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EventBus>();
    }
}
