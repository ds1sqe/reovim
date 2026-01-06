//! Type-erased event bus for plugin-based event handling
//!
//! This module provides a dynamic event system that allows plugins to define
//! and handle their own event types without modifying core code.
//!
//! # Architecture
//!
//! The event bus uses type erasure via `DynEvent` to allow any type implementing
//! the `Event` trait to be sent through the same channel. Handlers are registered
//! by event type and dispatched based on `TypeId`.
//!
//! # Example
//!
//! ```ignore
//! // Define a custom event
//! #[derive(Debug)]
//! struct MyEvent { data: String }
//! impl Event for MyEvent {}
//!
//! // Subscribe to the event
//! bus.subscribe::<MyEvent, _>(100, |event, ctx| {
//!     println!("Received: {}", event.data);
//!     EventResult::Handled
//! });
//!
//! // Emit the event
//! bus.emit(MyEvent { data: "hello".into() });
//! ```

#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::use_self)]
#![allow(clippy::missing_fields_in_debug)]

mod bus;
pub mod core_events;

use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

pub use {
    bus::{EventBus, EventSender, HandlerContext},
    core_events::*,
};

/// Marker trait for events that can be sent through the event bus
///
/// All events must be Send + Sync + 'static for thread-safety.
/// Events can optionally specify a priority for processing order.
pub trait Event: Send + Sync + Debug + 'static {
    /// Priority for processing order (lower = earlier)
    ///
    /// Default is 100. Core events use 0-50, plugins use 100+.
    fn priority(&self) -> u32 {
        100
    }
}

/// Marker trait for events that target a specific component
///
/// Events implementing this trait have a `target` field that specifies
/// which component should handle the event. This enables `subscribe_targeted()`
/// to automatically filter events by target.
pub trait TargetedEvent: Event {
    /// Get the target component ID for this event
    fn target(&self) -> crate::modd::ComponentId;
}

/// A type-erased event that can carry any payload
///
/// This allows different event types to be sent through the same channel
/// while preserving the ability to downcast to the original type.
pub struct DynEvent {
    /// Type ID for downcasting
    type_id: TypeId,
    /// Type name for debugging
    type_name: &'static str,
    /// Priority for ordering
    priority: u32,
    /// Boxed payload
    payload: Box<dyn Any + Send + Sync>,
}

impl DynEvent {
    /// Create a new dynamic event from any type implementing Event
    pub fn new<E: Event>(event: E) -> Self {
        let priority = event.priority();
        Self {
            type_id: TypeId::of::<E>(),
            type_name: std::any::type_name::<E>(),
            priority,
            payload: Box::new(event),
        }
    }

    /// Get the `TypeId` of the wrapped event
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Get the type name for debugging
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Get the event priority
    #[must_use]
    pub fn priority(&self) -> u32 {
        self.priority
    }

    /// Try to downcast to a concrete event type (immutable reference)
    pub fn downcast_ref<E: Event>(&self) -> Option<&E> {
        if self.type_id == TypeId::of::<E>() {
            self.payload.downcast_ref()
        } else {
            None
        }
    }

    /// Try to downcast to a concrete event type (mutable reference)
    pub fn downcast_mut<E: Event>(&mut self) -> Option<&mut E> {
        if self.type_id == TypeId::of::<E>() {
            self.payload.downcast_mut()
        } else {
            None
        }
    }

    /// Try to take ownership by downcasting
    pub fn into_inner<E: Event>(self) -> Result<E, Self> {
        if self.type_id == TypeId::of::<E>() {
            // SAFETY: We verified the type matches
            Ok(*self.payload.downcast::<E>().unwrap())
        } else {
            Err(self)
        }
    }
}

impl Debug for DynEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynEvent")
            .field("type", &self.type_name)
            .field("priority", &self.priority)
            .finish()
    }
}

/// Result of event handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Event was handled, continue processing other handlers
    Handled,
    /// Event was consumed, stop propagation to other handlers
    Consumed,
    /// Event was not handled by this handler
    NotHandled,
    /// Request a render after all handlers complete
    NeedsRender,
    /// Editor should quit
    Quit,
}

impl EventResult {
    /// Check if this result should stop propagation
    #[must_use]
    pub fn should_stop_propagation(&self) -> bool {
        matches!(self, EventResult::Consumed | EventResult::Quit)
    }

    /// Check if this result needs a render
    #[must_use]
    pub fn needs_render(&self) -> bool {
        matches!(self, EventResult::NeedsRender | EventResult::Handled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestEvent {
        value: i32,
    }

    impl Event for TestEvent {}

    #[test]
    fn test_dyn_event_creation_and_downcast() {
        let event = TestEvent { value: 42 };
        let dyn_event = DynEvent::new(event);

        assert_eq!(dyn_event.type_id(), TypeId::of::<TestEvent>());

        let downcasted = dyn_event.downcast_ref::<TestEvent>().unwrap();
        assert_eq!(downcasted.value, 42);
    }

    #[test]
    fn test_dyn_event_wrong_type_downcast() {
        #[derive(Debug)]
        struct OtherEvent;
        impl Event for OtherEvent {}

        let event = TestEvent { value: 42 };
        let dyn_event = DynEvent::new(event);

        assert!(dyn_event.downcast_ref::<OtherEvent>().is_none());
    }

    #[test]
    fn test_dyn_event_into_inner() {
        let event = TestEvent { value: 42 };
        let dyn_event = DynEvent::new(event);

        let inner = dyn_event.into_inner::<TestEvent>().unwrap();
        assert_eq!(inner.value, 42);
    }
}
