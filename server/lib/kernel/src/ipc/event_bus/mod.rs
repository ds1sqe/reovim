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
//! Following the proven lock-free patterns:
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
//! use reovim_kernel::api::v1::*;
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

mod handler;
mod sender;

pub use sender::EventSender;

use std::{any::TypeId, collections::HashMap, sync::Arc};

use reovim_arch::sync::{ArcSwap, Mutex};

use handler::{ContextHandlerFn, HandlerFn, HandlerType, RegisteredHandler};

use super::{
    channel::{BoundedReceiver, BoundedSender, bounded},
    context::{DispatchResult, HandlerContext},
    event::{DynEvent, Event, EventResult, TargetedEvent},
    scope::EventScope,
    subscription::{Subscription, SubscriptionId},
};

/// Optional channel for dedicated processor thread pattern.
///
/// Used when the `EventBus` is created with `new_with_channel()`.
struct ChannelInner {
    /// Sender side (cloneable)
    tx: BoundedSender<DynEvent>,

    /// Receiver side (only accessible once via `take_receiver()`)
    rx: Mutex<Option<BoundedReceiver<DynEvent>>>,
}

/// Internal state for `EventBus`.
struct EventBusInner {
    /// Handler storage: `TypeId` -> sorted Vec of handlers.
    /// Uses `ArcSwap` for lock-free reads.
    handlers: ArcSwap<HashMap<TypeId, Vec<RegisteredHandler>>>,

    /// Async event queue for `emit_async()`.
    queue: Mutex<Vec<DynEvent>>,

    /// Optional channel for dedicated processor thread.
    channel: Option<ChannelInner>,
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
///
/// # Two Usage Patterns
///
/// 1. **Simple/Tests**: Use `new()` with `emit_async()` and `process_queue()`
/// 2. **Runtime**: Use `new_with_channel()` with `sender()` and `take_receiver()`
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl EventBus {
    /// Create a new empty event bus.
    ///
    /// This creates an `EventBus` without a channel. Use `emit_async()` and
    /// `process_queue()` for deferred event processing.
    ///
    /// For runtime integration with a dedicated processor thread, use
    /// `new_with_channel()` instead.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                handlers: ArcSwap::from_pointee(HashMap::new()),
                queue: Mutex::new(Vec::new()),
                channel: None,
            }),
        }
    }

    /// Create an event bus with a channel for dedicated processor thread.
    ///
    /// This is the preferred constructor for runtime integration where events
    /// are processed by a dedicated OS thread using `blocking_recv()`.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Bounded channel capacity (typically 1024)
    ///
    /// # Usage Pattern
    ///
    /// ```ignore
    /// let bus = EventBus::new_with_channel(1024);
    ///
    /// // Get sender for emitting events
    /// let sender = bus.sender().unwrap();
    ///
    /// // Take receiver for processor thread
    /// let mut receiver = bus.take_receiver().unwrap();
    ///
    /// // Spawn processor thread
    /// std::thread::spawn(move || {
    ///     while let Some(event) = receiver.blocking_recv() {
    ///         // Process event
    ///     }
    /// });
    ///
    /// // Emit events from anywhere
    /// sender.send(MyEvent { ... });
    /// ```
    #[must_use]
    pub fn new_with_channel(capacity: usize) -> Self {
        let (tx, rx) = bounded(capacity);
        Self {
            inner: Arc::new(EventBusInner {
                handlers: ArcSwap::from_pointee(HashMap::new()),
                queue: Mutex::new(Vec::new()),
                channel: Some(ChannelInner {
                    tx,
                    rx: Mutex::new(Some(rx)),
                }),
            }),
        }
    }

    /// Get a cloneable sender for emitting events via channel.
    ///
    /// Returns `None` if the `EventBus` was created without a channel.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bus = EventBus::new_with_channel(1024);
    /// let sender = bus.sender().expect("bus has channel");
    ///
    /// sender.try_send(MyEvent { ... });
    /// ```
    #[must_use]
    pub fn sender(&self) -> Option<EventSender> {
        self.inner
            .channel
            .as_ref()
            .map(|c| EventSender { tx: c.tx.clone() })
    }

    /// Take the receiver for the dedicated processor thread.
    ///
    /// Can only be called once. Subsequent calls return `None`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bus = EventBus::new_with_channel(1024);
    /// let receiver = bus.take_receiver().expect("bus has channel");
    ///
    /// // Spawn processor thread
    /// std::thread::spawn(move || {
    ///     while let Some(event) = receiver.blocking_recv() {
    ///         // Process event
    ///     }
    /// });
    /// ```
    #[must_use]
    pub fn take_receiver(&self) -> Option<BoundedReceiver<DynEvent>> {
        self.inner.channel.as_ref()?.rx.lock().take()
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
    /// use reovim_kernel::api::v1::*;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
            handler: HandlerType::Simple(wrapped_handler),
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

    /// Subscribe a context-aware handler for events of type `E`.
    ///
    /// Similar to `subscribe`, but the handler receives a `HandlerContext`
    /// that allows emitting new events, requesting renders, etc.
    ///
    /// # Arguments
    ///
    /// * `priority` - Handler priority (lower = called earlier)
    /// * `handler` - Function called for each event, with access to context
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// #[derive(Debug)]
    /// struct MyEvent { value: i32 }
    /// impl Event for MyEvent {}
    ///
    /// #[derive(Debug)]
    /// struct FollowUpEvent;
    /// impl Event for FollowUpEvent {}
    ///
    /// let bus = EventBus::new();
    ///
    /// let sub = bus.subscribe_with_context::<MyEvent, _>(100, |event, ctx| {
    ///     // Emit a follow-up event
    ///     ctx.emit(FollowUpEvent);
    ///     // Request render
    ///     ctx.request_render();
    ///     EventResult::Handled
    /// });
    /// ```
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn subscribe_with_context<E, F>(&self, priority: u32, handler: F) -> Subscription
    where
        E: Event,
        F: Fn(&E, &mut HandlerContext) -> EventResult + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<E>();
        let sub_id = SubscriptionId::new();

        // Wrap handler to downcast from DynEvent
        let wrapped_handler: ContextHandlerFn = Arc::new(move |dyn_event: &DynEvent, ctx| {
            dyn_event
                .downcast_ref::<E>()
                .map_or(EventResult::NotHandled, |e| handler(e, ctx))
        });

        let registered = RegisteredHandler {
            id: sub_id,
            priority,
            handler: HandlerType::WithContext(wrapped_handler),
        };

        // RCU update: clone, modify, swap
        self.inner.handlers.rcu(|current| {
            let mut new_map = (**current).clone();
            let handlers = new_map.entry(type_id).or_default();
            handlers.push(registered.clone());
            handlers.sort_by_key(|h| h.priority);
            new_map
        });

        // Create unsubscribe closure
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

    /// Subscribe a handler for targeted events, filtering by target.
    ///
    /// The handler is only called when `event.target()` matches the
    /// specified target string.
    ///
    /// # Arguments
    ///
    /// * `target` - Target string to filter events
    /// * `priority` - Handler priority (lower = called earlier)
    /// * `handler` - Function called for matching events
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// #[derive(Debug)]
    /// struct PluginInput {
    ///     target: &'static str,
    ///     value: i32,
    /// }
    /// impl Event for PluginInput {}
    /// impl TargetedEvent for PluginInput {
    ///     fn target(&self) -> &str { self.target }
    /// }
    ///
    /// let bus = EventBus::new();
    ///
    /// // Only receives events where target == "my_plugin"
    /// let sub = bus.subscribe_targeted::<PluginInput, _>("my_plugin", 100, |event, ctx| {
    ///     println!("My plugin received: {}", event.value);
    ///     EventResult::Handled
    /// });
    /// ```
    pub fn subscribe_targeted<E, F>(&self, target: &str, priority: u32, handler: F) -> Subscription
    where
        E: TargetedEvent,
        F: Fn(&E, &mut HandlerContext) -> EventResult + Send + Sync + 'static,
    {
        let target = target.to_owned();
        self.subscribe_with_context::<E, _>(priority, move |event, ctx| {
            if event.target() == target {
                handler(event, ctx)
            } else {
                EventResult::NotHandled
            }
        })
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
    /// use reovim_kernel::api::v1::*;
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
    /// use reovim_kernel::api::v1::*;
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
    /// use reovim_kernel::api::v1::*;
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
            let _result = self.dispatch(&event);
            // TODO: Replace with pr_trace!() once printk is implemented
            // pr_trace!("processed queued event: {} -> {:?}", event.type_name(), result);
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
        // Create a temporary context for context-aware handlers
        let mut temp_ctx = HandlerContext::new();

        for handler in handlers {
            let handler_result = match &handler.handler {
                HandlerType::Simple(h) => h(event),
                HandlerType::WithContext(h) => h(event, &mut temp_ctx),
            };

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

    /// Dispatch an event with handler context.
    ///
    /// This allows handlers to emit new events, request renders, etc.
    /// Returns a `DispatchResult` containing the event result and any
    /// side effects (emitted events, render requests).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut ctx = HandlerContext::new().with_scope(Some(scope));
    /// let result = bus.dispatch_with_context(&event, &mut ctx);
    ///
    /// if result.render_requested {
    ///     // Handle render request
    /// }
    ///
    /// // Process emitted events
    /// for event in result.emitted_events {
    ///     bus.dispatch(&event);
    /// }
    /// ```
    pub fn dispatch_with_context(
        &self,
        event: &DynEvent,
        ctx: &mut HandlerContext,
    ) -> DispatchResult {
        // Lock-free read of handlers
        let handlers = self.inner.handlers.load();
        let type_id = event.type_id();

        let Some(handlers) = handlers.get(&type_id) else {
            return DispatchResult::not_handled();
        };

        let mut result = EventResult::NotHandled;

        for handler in handlers {
            let handler_result = match &handler.handler {
                HandlerType::Simple(h) => h(event),
                HandlerType::WithContext(h) => h(event, ctx),
            };

            match handler_result {
                EventResult::Consumed => {
                    return DispatchResult::new(EventResult::Consumed, ctx);
                }
                EventResult::Handled => {
                    result = EventResult::Handled;
                }
                EventResult::NotHandled => {
                    // Continue to next handler
                }
            }
        }

        DispatchResult::new(result, ctx)
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

