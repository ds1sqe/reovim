//! Event trait and type-erased event wrapper.
//!
//! This module provides the foundational `Event` trait that all IPC events must implement,
//! along with `DynEvent` for type-erased event dispatch.
//!
//! # Design Philosophy
//!
//! Following the Linux kernel "mechanism, not policy" principle:
//! - Event trait defines minimal requirements
//! - `DynEvent` provides type-erased storage and dispatch
//! - `EventResult` signals handler outcomes without error semantics
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! #[derive(Debug)]
//! struct BufferChanged {
//!     buffer_id: u64,
//! }
//!
//! impl Event for BufferChanged {
//!     fn priority(&self) -> u32 { 50 } // Core priority
//! }
//!
//! let event = BufferChanged { buffer_id: 1 };
//! let dyn_event = DynEvent::new(event);
//!
//! // Type-safe downcasting
//! if let Some(bc) = dyn_event.downcast_ref::<BufferChanged>() {
//!     assert_eq!(bc.buffer_id, 1);
//! }
//! ```

use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

use super::scope::EventScope;

/// Trait for all IPC events.
///
/// Events are the primary communication mechanism between kernel subsystems,
/// drivers, and modules. All events must be:
/// - `Send + Sync` for cross-thread dispatch
/// - `Debug` for logging and diagnostics
/// - `'static` for type-erased storage
///
/// # Priority System
///
/// Events have a priority that affects handler dispatch order:
/// - **0-50**: Core/critical handlers (run first)
/// - **100**: Default priority
/// - **200+**: Low priority (cleanup, logging)
///
/// Lower priority numbers mean earlier dispatch.
///
/// # Batching
///
/// Events can opt into batching for future optimization. When `batchable()`
/// returns `true`, the event bus may combine multiple events of the same type
/// into a single dispatch when under load.
pub trait Event: Send + Sync + Debug + 'static {
    /// Priority for handler dispatch ordering.
    ///
    /// Lower values = earlier dispatch. Default is 100 (normal priority).
    ///
    /// Convention:
    /// - 0-50: Core handlers
    /// - 100: Default/plugin handlers
    /// - 200+: Low priority (logging, cleanup)
    fn priority(&self) -> u32 {
        100
    }

    /// Whether this event can be batched with similar events.
    ///
    /// Default is `false`. When `true`, the event bus may combine multiple
    /// events of the same type into a single dispatch for efficiency.
    fn batchable(&self) -> bool {
        false
    }
}

/// Result of handling an event.
///
/// Unlike `Result<T, E>`, this enum has no error variant. Event handling follows
/// a fire-and-forget philosophy where handlers must not fail - they either
/// handle the event or pass it on.
///
/// # Handler Behavior
///
/// - `Handled`: Continue to next handler (most common)
/// - `Consumed`: Stop propagation to remaining handlers
/// - `NotHandled`: Handler didn't process this event (continue dispatch)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventResult {
    /// Event was handled, continue to next handler.
    Handled,

    /// Event was consumed, stop propagation.
    Consumed,

    /// Handler did not process this event.
    #[default]
    NotHandled,
}

impl EventResult {
    /// Returns `true` if the event was consumed (propagation should stop).
    #[inline]
    #[must_use]
    pub const fn is_consumed(&self) -> bool {
        matches!(self, Self::Consumed)
    }

    /// Returns `true` if the event was handled (but not consumed).
    #[inline]
    #[must_use]
    pub const fn is_handled(&self) -> bool {
        matches!(self, Self::Handled)
    }

    /// Returns `true` if the handler did not process the event.
    #[inline]
    #[must_use]
    pub const fn is_not_handled(&self) -> bool {
        matches!(self, Self::NotHandled)
    }
}

/// Type-erased event wrapper for dynamic dispatch.
///
/// `DynEvent` wraps any type implementing `Event` and provides:
/// - Type-safe downcasting via `TypeId`
/// - Priority and metadata access
/// - Optional scope attachment for lifecycle tracking
///
/// # Type Safety
///
/// Despite being type-erased, `DynEvent` maintains full type safety through
/// `TypeId`-based downcasting. Incorrect type casts return `None` rather than
/// undefined behavior.
///
/// # Memory Layout
///
/// ```text
/// DynEvent
/// ├── type_id: TypeId (16 bytes)
/// ├── type_name: &'static str (16 bytes)
/// ├── priority: u32 (4 bytes)
/// ├── payload: Box<dyn Any + Send + Sync> (heap-allocated)
/// └── scope: Option<EventScope> (optional lifecycle tracking)
/// ```
pub struct DynEvent {
    /// `TypeId` for runtime type checking.
    type_id: TypeId,

    /// Type name for debugging/logging.
    type_name: &'static str,

    /// Event priority (cached from `Event::priority()`).
    priority: u32,

    /// Type-erased event payload
    payload: Box<dyn Any + Send + Sync>,

    /// Optional scope for lifecycle tracking
    scope: Option<EventScope>,
}

impl DynEvent {
    /// Create a new `DynEvent` from any type implementing `Event`.
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
    /// let dyn_event = DynEvent::new(MyEvent);
    /// assert!(dyn_event.type_name().contains("MyEvent"));
    /// ```
    pub fn new<E: Event>(event: E) -> Self {
        Self {
            type_id: TypeId::of::<E>(),
            type_name: std::any::type_name::<E>(),
            priority: event.priority(),
            payload: Box::new(event),
            scope: None,
        }
    }

    /// Attach an `EventScope` for lifecycle tracking.
    ///
    /// Returns `self` for method chaining.
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
    /// let scope = EventScope::new();
    /// let event = DynEvent::new(MyEvent).with_scope(scope);
    /// assert!(event.scope().is_some());
    /// ```
    #[must_use]
    pub fn with_scope(mut self, scope: EventScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Get the `TypeId` of the wrapped event.
    #[inline]
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Get the type name of the wrapped event (for debugging).
    #[inline]
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Get the event's priority.
    #[inline]
    #[must_use]
    pub const fn priority(&self) -> u32 {
        self.priority
    }

    /// Get a reference to the attached scope, if any.
    #[inline]
    #[must_use]
    pub const fn scope(&self) -> Option<&EventScope> {
        self.scope.as_ref()
    }

    /// Take the attached scope, leaving `None` in its place.
    #[inline]
    #[allow(clippy::missing_const_for_fn)] // Option::take() is not const
    pub fn take_scope(&mut self) -> Option<EventScope> {
        self.scope.take()
    }

    /// Check if this event is of type `E`.
    #[inline]
    #[must_use]
    pub fn is<E: Event>(&self) -> bool {
        self.type_id == TypeId::of::<E>()
    }

    /// Attempt to downcast to a reference of type `E`.
    ///
    /// Returns `None` if the event is not of type `E`.
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
    /// let event = DynEvent::new(MyEvent { value: 42 });
    ///
    /// if let Some(my_event) = event.downcast_ref::<MyEvent>() {
    ///     assert_eq!(my_event.value, 42);
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn downcast_ref<E: Event>(&self) -> Option<&E> {
        if self.type_id == TypeId::of::<E>() {
            self.payload.downcast_ref()
        } else {
            None
        }
    }

    /// Attempt to downcast to a mutable reference of type `E`.
    ///
    /// Returns `None` if the event is not of type `E`.
    #[inline]
    #[must_use]
    pub fn downcast_mut<E: Event>(&mut self) -> Option<&mut E> {
        if self.type_id == TypeId::of::<E>() {
            self.payload.downcast_mut()
        } else {
            None
        }
    }

    /// Consume the `DynEvent` and attempt to extract the inner event.
    ///
    /// # Errors
    ///
    /// Returns `Err(self)` if the event is not of type `E`.
    ///
    /// # Panics
    ///
    /// This function will not panic under normal operation. The internal
    /// `unwrap()` is guarded by a `TypeId` check that ensures type safety.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct MyEvent { value: i32 }
    /// impl Event for MyEvent {}
    ///
    /// let event = DynEvent::new(MyEvent { value: 42 });
    ///
    /// match event.into_inner::<MyEvent>() {
    ///     Ok(my_event) => assert_eq!(my_event.value, 42),
    ///     Err(_) => panic!("unexpected type"),
    /// }
    /// ```
    pub fn into_inner<E: Event>(self) -> Result<E, Self> {
        if self.type_id == TypeId::of::<E>() {
            // SAFETY: We verified the type matches via TypeId
            Ok(*self.payload.downcast::<E>().unwrap())
        } else {
            Err(self)
        }
    }
}

impl Debug for DynEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynEvent")
            .field("type_name", &self.type_name)
            .field("priority", &self.priority)
            .field("has_scope", &self.scope.is_some())
            .finish_non_exhaustive()
    }
}

// DynEvent is automatically Send + Sync because:
// - payload is Box<dyn Any + Send + Sync>
// - EventScope is Arc-based and thread-safe
// - All other fields are Copy or 'static references
//
// No manual unsafe impl needed - Rust derives these automatically.

// ============================================================================
// Built-in Events
// ============================================================================

use crate::mm::BufferId;

/// Cache update notification event.
///
/// Emitted when a cache (e.g., syntax highlights) has been updated for a buffer.
/// Drivers can subscribe to this event to trigger re-renders or other updates.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// let bus = EventBus::new();
///
/// bus.subscribe::<CacheUpdated, _>(100, |event| {
///     println!("Cache updated for buffer {:?}: {:?}", event.buffer_id, event.kind);
///     EventResult::Handled
/// });
/// ```
#[derive(Debug, Clone)]
pub struct CacheUpdated {
    /// The buffer whose cache was updated.
    pub buffer_id: BufferId,
    /// What kind of cache was updated.
    pub kind: CacheKind,
}

impl Event for CacheUpdated {
    fn priority(&self) -> u32 {
        50 // Core priority
    }
}

/// The kind of cache that was updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheKind {
    /// Syntax highlighting cache.
    Highlights,
    /// Decorations cache (diagnostics, git markers, etc.).
    Decorations,
    /// Both highlights and decorations.
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestEvent {
        value: i32,
    }

    impl Event for TestEvent {
        fn priority(&self) -> u32 {
            50
        }
    }

    #[derive(Debug)]
    struct HighPriorityEvent;

    impl Event for HighPriorityEvent {
        fn priority(&self) -> u32 {
            0
        }

        fn batchable(&self) -> bool {
            true
        }
    }

    #[derive(Debug)]
    struct DefaultEvent;
    impl Event for DefaultEvent {}

    #[derive(Debug)]
    struct OtherEvent;
    impl Event for OtherEvent {}

    // ========== Event trait tests ==========

    #[test]
    fn test_event_default_priority() {
        let event = DefaultEvent;
        assert_eq!(event.priority(), 100);
    }

    #[test]
    fn test_event_custom_priority() {
        let event = TestEvent { value: 42 };
        assert_eq!(event.priority(), 50);
    }

    #[test]
    fn test_event_high_priority() {
        let event = HighPriorityEvent;
        assert_eq!(event.priority(), 0);
    }

    #[test]
    fn test_event_default_batchable() {
        let event = DefaultEvent;
        assert!(!event.batchable());
    }

    #[test]
    fn test_event_custom_batchable() {
        let event = HighPriorityEvent;
        assert!(event.batchable());
    }

    // ========== EventResult tests ==========

    #[test]
    fn test_event_result_is_consumed() {
        assert!(EventResult::Consumed.is_consumed());
        assert!(!EventResult::Handled.is_consumed());
        assert!(!EventResult::NotHandled.is_consumed());
    }

    #[test]
    fn test_event_result_is_handled() {
        assert!(!EventResult::Consumed.is_handled());
        assert!(EventResult::Handled.is_handled());
        assert!(!EventResult::NotHandled.is_handled());
    }

    #[test]
    fn test_event_result_is_not_handled() {
        assert!(!EventResult::Consumed.is_not_handled());
        assert!(!EventResult::Handled.is_not_handled());
        assert!(EventResult::NotHandled.is_not_handled());
    }

    #[test]
    fn test_event_result_default() {
        assert_eq!(EventResult::default(), EventResult::NotHandled);
    }

    // ========== DynEvent tests ==========

    #[test]
    fn test_dyn_event_new() {
        let event = DynEvent::new(TestEvent { value: 42 });
        assert!(event.type_name().contains("TestEvent"));
        assert_eq!(event.priority(), 50);
        assert!(event.scope().is_none());
    }

    #[test]
    fn test_dyn_event_type_id() {
        let event = DynEvent::new(TestEvent { value: 42 });
        assert_eq!(event.type_id(), TypeId::of::<TestEvent>());
    }

    #[test]
    fn test_dyn_event_is() {
        let event = DynEvent::new(TestEvent { value: 42 });
        assert!(event.is::<TestEvent>());
        assert!(!event.is::<OtherEvent>());
    }

    #[test]
    fn test_dyn_event_downcast_ref() {
        let event = DynEvent::new(TestEvent { value: 42 });
        let downcasted = event.downcast_ref::<TestEvent>();
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().value, 42);
    }

    #[test]
    fn test_dyn_event_downcast_ref_wrong_type() {
        let event = DynEvent::new(TestEvent { value: 42 });
        let downcasted = event.downcast_ref::<OtherEvent>();
        assert!(downcasted.is_none());
    }

    #[test]
    fn test_dyn_event_downcast_mut() {
        let mut event = DynEvent::new(TestEvent { value: 42 });
        if let Some(inner) = event.downcast_mut::<TestEvent>() {
            inner.value = 100;
        }
        assert_eq!(event.downcast_ref::<TestEvent>().unwrap().value, 100);
    }

    #[test]
    fn test_dyn_event_downcast_mut_wrong_type() {
        let mut event = DynEvent::new(TestEvent { value: 42 });
        let downcasted = event.downcast_mut::<OtherEvent>();
        assert!(downcasted.is_none());
    }

    #[test]
    fn test_dyn_event_into_inner() {
        let event = DynEvent::new(TestEvent { value: 42 });
        let inner = event.into_inner::<TestEvent>();
        assert!(inner.is_ok());
        assert_eq!(inner.unwrap(), TestEvent { value: 42 });
    }

    #[test]
    fn test_dyn_event_into_inner_wrong_type() {
        let event = DynEvent::new(TestEvent { value: 42 });
        let result = event.into_inner::<OtherEvent>();
        assert!(result.is_err());
    }

    #[test]
    fn test_dyn_event_with_scope() {
        let scope = EventScope::new();
        let event = DynEvent::new(TestEvent { value: 42 }).with_scope(scope.clone());
        assert!(event.scope().is_some());
        assert_eq!(event.scope().unwrap().id(), scope.id());
    }

    #[test]
    fn test_dyn_event_take_scope() {
        let scope = EventScope::new();
        let mut event = DynEvent::new(TestEvent { value: 42 }).with_scope(scope);

        let taken = event.take_scope();
        assert!(taken.is_some());
        assert!(event.scope().is_none());
    }

    #[test]
    fn test_dyn_event_debug() {
        let event = DynEvent::new(TestEvent { value: 42 });
        let debug_str = format!("{event:?}");
        assert!(debug_str.contains("DynEvent"));
        assert!(debug_str.contains("TestEvent"));
        assert!(debug_str.contains("50"));
    }

    #[test]
    fn test_dyn_event_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DynEvent>();
    }
}
