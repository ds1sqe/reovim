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
///
/// # Targeted Events
///
/// For events that target specific components, implement [`TargetedEvent`]
/// in addition to `Event`. This allows using `EventBus::subscribe_targeted()`
/// for automatic filtering by target.
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

/// Marker trait for events that target a specific component.
///
/// Events implementing this trait have a `target` field that specifies
/// which component should handle the event. This enables `EventBus::subscribe_targeted()`
/// to automatically filter events by target.
///
/// # Design Note
///
/// The kernel uses `&str` for target identifiers (mechanism), not `ComponentId`
/// (which is a policy-level type). Modules convert their identifiers to `&str`
/// when interacting with the kernel API.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// #[derive(Debug)]
/// struct PluginTextInput {
///     target: &'static str,
///     c: char,
/// }
///
/// impl Event for PluginTextInput {}
///
/// impl TargetedEvent for PluginTextInput {
///     fn target(&self) -> &str {
///         self.target
///     }
/// }
/// ```
pub trait TargetedEvent: Event {
    /// Get the target component identifier for this event.
    ///
    /// Used by `EventBus::subscribe_targeted()` to filter events.
    fn target(&self) -> &str;
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

