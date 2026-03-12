//! Subscription handle for event bus unsubscription.
//!
//! `Subscription` is an RAII handle that automatically unsubscribes from the
//! event bus when dropped. This ensures handlers are properly cleaned up even
//! in the presence of panics or early returns.
//!
//! # Design Philosophy
//!
//! Following Rust's RAII pattern:
//! - Subscription creation is tied to handler registration
//! - Dropping the subscription removes the handler
//! - Explicit `cancel()` available for early unsubscription
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::ipc::{EventBus, Event, Subscription};
//!
//! #[derive(Debug)]
//! struct MyEvent;
//! impl Event for MyEvent {}
//!
//! let bus = EventBus::new();
//!
//! // Subscription returned from subscribe
//! let sub = bus.subscribe::<MyEvent, _>(100, |_event| {
//!     println!("Event received!");
//!     EventResult::Handled
//! });
//!
//! // Handler is active while `sub` is alive
//! bus.emit(MyEvent);
//!
//! // Drop `sub` to unsubscribe
//! drop(sub);
//! ```

use std::{
    any::TypeId,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use reovim_arch::sync::Mutex;

/// Unique identifier for a subscription.
///
/// Each subscription gets a unique ID for identification and debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Create a new unique `SubscriptionId`.
    pub(crate) fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    #[inline]
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sub-{}", self.0)
    }
}

/// Callback type for unsubscription.
type UnsubscribeFn = Box<dyn FnOnce() + Send + Sync>;

/// RAII handle for event subscriptions.
///
/// When a `Subscription` is dropped, the associated handler is automatically
/// removed from the event bus. This ensures proper cleanup even when code
/// panics or returns early.
///
/// # Thread Safety
///
/// `Subscription` is `Send + Sync`, allowing it to be moved between threads
/// and shared (though sharing is unusual since drop triggers unsubscription).
///
/// # Example
///
/// ```ignore
/// // Subscription automatically unsubscribes when dropped
/// {
///     let sub = bus.subscribe::<MyEvent, _>(100, handler);
///     // Handler is active here
/// } // Handler is removed here
/// ```
pub struct Subscription {
    /// Unique identifier for this subscription.
    id: SubscriptionId,

    /// `TypeId` of the event type (for debugging).
    type_id: TypeId,

    /// Type name of the event (for debugging).
    type_name: &'static str,

    /// Unsubscribe callback, called on drop.
    /// Wrapped in `Arc<Mutex>` to allow the callback to capture &self references
    /// from the `EventBus`.
    unsubscribe_fn: Arc<Mutex<Option<UnsubscribeFn>>>,
}

impl Subscription {
    /// Create a new subscription.
    ///
    /// This is called internally by `EventBus::subscribe()`.
    pub(crate) fn new<E: 'static>(
        id: SubscriptionId,
        unsubscribe_fn: impl FnOnce() + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            type_id: TypeId::of::<E>(),
            type_name: std::any::type_name::<E>(),
            unsubscribe_fn: Arc::new(Mutex::new(Some(Box::new(unsubscribe_fn)))),
        }
    }

    /// Get the subscription ID.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> SubscriptionId {
        self.id
    }

    /// Get the `TypeId` of the subscribed event type.
    #[inline]
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Get the type name of the subscribed event type.
    #[inline]
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Check if the subscription is still active.
    ///
    /// Returns `false` after `cancel()` has been called.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.unsubscribe_fn.lock().is_some()
    }

    /// Explicitly cancel the subscription.
    ///
    /// This is equivalent to dropping the subscription, but allows for
    /// explicit control over when unsubscription occurs.
    ///
    /// Calling `cancel()` multiple times is safe and idempotent.
    pub fn cancel(&self) {
        let unsub = self.unsubscribe_fn.lock().take();
        if let Some(callback) = unsub {
            callback();
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Subscription")
            .field("id", &self.id)
            .field("type_name", &self.type_name)
            .field("active", &self.is_active())
            .finish_non_exhaustive()
    }
}

// Subscription is automatically Send + Sync because:
// - SubscriptionId is Copy
// - TypeId and &'static str are Send + Sync
// - UnsubscribeFn is Box<dyn FnOnce() + Send + Sync>
// - Arc<Mutex<...>> is Send + Sync
//
// No manual unsafe impl needed - Rust derives these automatically.

