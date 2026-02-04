//! Handler types for event dispatch.

use std::sync::Arc;

use super::super::{
    context::HandlerContext,
    event::{DynEvent, EventResult},
    subscription::SubscriptionId,
};

/// Handler function type for simple event dispatch (no context).
///
/// Handlers receive a reference to the `DynEvent` and return an `EventResult`.
/// The handler must downcast to the specific event type internally.
pub(super) type HandlerFn = Arc<dyn Fn(&DynEvent) -> EventResult + Send + Sync>;

/// Handler function type for context-aware event dispatch.
///
/// Handlers receive both the event and a mutable `HandlerContext` that allows
/// emitting new events, requesting renders, etc.
///
/// Uses Higher-Ranked Trait Bounds (HRTB) to work with any `HandlerContext` lifetime.
pub(super) type ContextHandlerFn =
    Arc<dyn for<'a> Fn(&DynEvent, &mut HandlerContext<'a>) -> EventResult + Send + Sync>;

/// Type of handler function (simple or context-aware).
pub(super) enum HandlerType {
    /// Simple handler: `Fn(&E) -> EventResult`
    Simple(HandlerFn),
    /// Context-aware handler: `Fn(&E, &mut HandlerContext) -> EventResult`
    WithContext(ContextHandlerFn),
}

impl Clone for HandlerType {
    fn clone(&self) -> Self {
        match self {
            Self::Simple(h) => Self::Simple(h.clone()),
            Self::WithContext(h) => Self::WithContext(h.clone()),
        }
    }
}

/// Registered handler with metadata.
pub(super) struct RegisteredHandler {
    /// Unique subscription ID.
    pub id: SubscriptionId,

    /// Handler priority (lower = earlier dispatch).
    pub priority: u32,

    /// The handler function (simple or context-aware).
    pub handler: HandlerType,
}

impl Clone for RegisteredHandler {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            priority: self.priority,
            handler: self.handler.clone(),
        }
    }
}
