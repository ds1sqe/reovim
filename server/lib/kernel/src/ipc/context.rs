//! Handler context for event dispatch.
//!
//! `HandlerContext` provides event handlers with the ability to:
//! - Emit new events (with automatic scope inheritance)
//! - Request render updates
//! - Request application quit
//!
//! # Design Philosophy
//!
//! Following "mechanism, not policy":
//! - Context provides emission and signaling mechanisms
//! - Policy-specific methods (mode changes, etc.) stay in modules as extension traits
//!
//! # Example
//!
//! ```ignore
//! bus.subscribe_with_context::<MyEvent, _>(100, |event, ctx| {
//!     // Emit a follow-up event (scope is inherited automatically)
//!     ctx.emit(FollowUpEvent { id: event.id });
//!
//!     // Request a render after processing
//!     ctx.request_render();
//!
//!     EventResult::Handled
//! });
//! ```

use super::{
    event::{DynEvent, Event, EventResult},
    event_bus::EventSender,
    scope::EventScope,
};

/// Context passed to event handlers during dispatch.
///
/// `HandlerContext` enables handlers to interact with the event system:
/// - Emit new events with automatic scope tracking
/// - Signal render requests
/// - Signal quit requests
///
/// # Emission Modes
///
/// `HandlerContext` supports two emission modes:
///
/// 1. **Collect mode** (default): Events are collected in a Vec and returned
///    via `take_emitted_events()` after the handler completes.
///
/// 2. **Direct mode** (with sender): Events are sent directly to a channel
///    via the attached `EventSender`. Use `with_sender()` to enable this.
///
/// # Scope Inheritance
///
/// When `emit()` is called and the context has an attached scope,
/// the new event inherits the scope. This ensures proper lifecycle
/// tracking for chains of related events.
///
/// # Thread Safety
///
/// `HandlerContext` is not `Send` or `Sync` - it's designed to be used
/// within a single handler invocation.
pub struct HandlerContext<'a> {
    /// Flag: render was requested by handler
    render_requested: bool,

    /// Flag: quit was requested by handler
    quit_requested: bool,

    /// Current scope for lifecycle tracking (inherited by emitted events)
    scope: Option<EventScope>,

    /// Events emitted during handler execution (collect mode)
    /// These are collected and dispatched after the handler returns
    emitted_events: Vec<DynEvent>,

    /// Optional sender for direct emission mode
    /// When present, `emit()` sends directly instead of collecting
    sender: Option<&'a EventSender>,
}

impl<'a> HandlerContext<'a> {
    /// Create a new handler context in collect mode.
    ///
    /// Events emitted via `emit()` are collected and can be retrieved
    /// with `take_emitted_events()` after the handler completes.
    ///
    /// Typically called by the event processor before invoking handlers.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            render_requested: false,
            quit_requested: false,
            scope: None,
            emitted_events: Vec::new(),
            sender: None,
        }
    }

    /// Attach an event sender for direct emission mode.
    ///
    /// When a sender is attached, `emit()` sends events directly to the
    /// channel instead of collecting them. This matches the runtime's
    /// expected behavior where emitted events re-enter the event queue.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let sender = bus.sender().unwrap();
    /// let ctx = HandlerContext::new().with_sender(&sender);
    /// // Now ctx.emit() sends directly via channel
    /// ```
    #[must_use]
    pub const fn with_sender(mut self, sender: &'a EventSender) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Attach a scope for lifecycle tracking.
    ///
    /// Events emitted via `emit()` will inherit this scope.
    #[must_use]
    pub fn with_scope(mut self, scope: Option<EventScope>) -> Self {
        self.scope = scope;
        self
    }

    /// Get a reference to the current scope, if any.
    #[must_use]
    pub const fn scope(&self) -> Option<&EventScope> {
        self.scope.as_ref()
    }

    /// Emit a new event from within a handler.
    ///
    /// Behavior depends on mode:
    /// - **Collect mode** (no sender): Event is collected in internal Vec
    /// - **Direct mode** (with sender): Event is sent directly to channel
    ///
    /// If this context has a scope, the event inherits it for proper
    /// lifecycle tracking.
    ///
    /// # Scope Tracking
    ///
    /// When a scope is present:
    /// 1. `scope.increment()` is called
    /// 2. Event is wrapped with the scope
    /// 3. After dispatch, `scope.decrement()` is called by the processor
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.emit(BufferModified { buffer_id: 1 });
    /// ```
    pub fn emit<E: Event>(&mut self, event: E) {
        let dyn_event = if let Some(ref scope) = self.scope {
            scope.increment();
            DynEvent::new(event).with_scope(scope.clone())
        } else {
            DynEvent::new(event)
        };

        // Direct mode: send via channel
        if let Some(sender) = self.sender {
            sender.send_dyn(dyn_event);
        } else {
            // Collect mode: store for later retrieval
            self.emitted_events.push(dyn_event);
        }
    }

    /// Emit a pre-boxed dynamic event.
    ///
    /// Useful when you already have a `DynEvent` and want to emit it.
    /// The event's existing scope (if any) is preserved.
    pub fn emit_dyn(&mut self, event: DynEvent) {
        if let Some(sender) = self.sender {
            sender.send_dyn(event);
        } else {
            self.emitted_events.push(event);
        }
    }

    /// Request a render update after event processing completes.
    ///
    /// This is a hint to the runtime that the UI should be refreshed.
    /// The actual render is performed by the runtime, not the kernel.
    pub const fn request_render(&mut self) {
        self.render_requested = true;
    }

    /// Request application quit.
    ///
    /// This signals that the handler wants the application to terminate.
    /// The runtime decides how to handle this request.
    pub const fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Check if render was requested.
    #[must_use]
    pub const fn render_requested(&self) -> bool {
        self.render_requested
    }

    /// Check if quit was requested.
    #[must_use]
    pub const fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Take collected emitted events.
    ///
    /// Called by the event processor after handler execution to get
    /// the events that need to be dispatched.
    ///
    /// Note: In direct mode (with sender), this will always return
    /// an empty Vec since events are sent immediately.
    #[must_use]
    pub fn take_emitted_events(&mut self) -> Vec<DynEvent> {
        std::mem::take(&mut self.emitted_events)
    }

    /// Check if any events were emitted (collect mode only).
    #[must_use]
    pub const fn has_emitted_events(&self) -> bool {
        !self.emitted_events.is_empty()
    }

    /// Get the number of emitted events (collect mode only).
    #[must_use]
    pub const fn emitted_event_count(&self) -> usize {
        self.emitted_events.len()
    }

    /// Check if this context is in direct emission mode.
    #[must_use]
    pub const fn has_sender(&self) -> bool {
        self.sender.is_some()
    }
}

impl Default for HandlerContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HandlerContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerContext")
            .field("render_requested", &self.render_requested)
            .field("quit_requested", &self.quit_requested)
            .field("has_scope", &self.scope.is_some())
            .field("has_sender", &self.sender.is_some())
            .field("emitted_events", &self.emitted_events.len())
            .finish()
    }
}

/// Result of dispatching with context.
///
/// Contains both the event result and any side effects from handlers.
#[derive(Debug)]
pub struct DispatchResult {
    /// The event handling result
    pub result: EventResult,

    /// Whether any handler requested a render
    pub render_requested: bool,

    /// Whether any handler requested quit
    pub quit_requested: bool,

    /// Events emitted by handlers (need to be dispatched)
    pub emitted_events: Vec<DynEvent>,
}

impl DispatchResult {
    /// Create a new dispatch result.
    #[must_use]
    pub fn new(result: EventResult, ctx: &mut HandlerContext<'_>) -> Self {
        Self {
            result,
            render_requested: ctx.render_requested(),
            quit_requested: ctx.quit_requested(),
            emitted_events: ctx.take_emitted_events(),
        }
    }

    /// Create a result indicating no handlers matched.
    #[must_use]
    pub const fn not_handled() -> Self {
        Self {
            result: EventResult::NotHandled,
            render_requested: false,
            quit_requested: false,
            emitted_events: Vec::new(),
        }
    }
}

