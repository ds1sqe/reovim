//! Key event handlers for `EventBus` dispatch.
//!
//! This module provides handler registration infrastructure for
//! future multi-handler scenarios (plugins, extensions).
//!
//! # Current Status
//!
//! As of #409, the event loop uses direct emit + process pattern.
//! This infrastructure is preserved for:
//! - Future plugin-based handlers
//! - Multi-session event routing
//! - Handler priority chains
//!
//! # Safety Features
//!
//! - **Panic isolation**: Handlers are wrapped in `catch_unwind` to prevent
//!   panics from crashing the server.
//! - **Recursion guard**: Thread-local depth counter prevents infinite loops
//!   when handlers emit key events.
//! - **Session filtering**: Handlers only process events for their session.
//!
//! # Future Usage
//!
//! ```ignore
//! // Future: Plugin registers handler
//! let _sub = register_key_handler(&bus, session_id, |event| {
//!     // Plugin-specific key handling
//!     EventResult::Handled
//! });
//! ```

use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use reovim_kernel::api::v1::{
    EventBus, EventResult, Subscription,
    events::{KeyPressEvent, SessionId, priority},
};

// =============================================================================
// Constants
// =============================================================================

/// Maximum recursion depth for event handlers.
///
/// Prevents infinite loops when handlers emit key events.
/// Set to 16 as a conservative limit (typical recursion is ~3 levels).
#[allow(dead_code)] // Preserved for future plugin support
pub const RECURSION_LIMIT: u32 = 16;

// =============================================================================
// Recursion Guard
// =============================================================================

thread_local! {
    /// Current recursion depth for this thread.
    static RECURSION_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Guard that increments recursion depth on creation and decrements on drop.
///
/// Uses RAII pattern to ensure depth is always decremented, even on panic.
#[allow(dead_code)] // Preserved for future plugin support
pub struct RecursionGuard;

#[allow(dead_code)] // Preserved for future plugin support
impl RecursionGuard {
    /// Try to enter a new recursion level.
    ///
    /// Returns `Some(RecursionGuard)` if under the limit, `None` if limit exceeded.
    /// The guard decrements the depth when dropped.
    #[must_use]
    pub fn try_enter() -> Option<Self> {
        RECURSION_DEPTH.with(|depth| {
            let current = depth.get();
            if current >= RECURSION_LIMIT {
                None
            } else {
                depth.set(current + 1);
                Some(Self)
            }
        })
    }

    /// Get the current recursion depth.
    #[must_use]
    pub fn current_depth() -> u32 {
        RECURSION_DEPTH.with(Cell::get)
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        RECURSION_DEPTH.with(|depth| {
            depth.set(depth.get().saturating_sub(1));
        });
    }
}

/// Reset recursion depth to 0 (for testing only).
#[cfg(test)]
pub fn reset_recursion_depth() {
    RECURSION_DEPTH.with(|depth| depth.set(0));
}

// =============================================================================
// Handler Registration
// =============================================================================

/// Register a key handler for a session.
///
/// The handler is called for all `KeyPressEvent`s but filters by `session_id`
/// to only process events for its session.
///
/// # Arguments
///
/// * `bus` - The event bus to register with
/// * `session_id` - The session ID to filter for
/// * `handler` - The handler function to call
///
/// # Returns
///
/// A `Subscription` that unregisters the handler when dropped.
///
/// # Panic Safety
///
/// The handler is wrapped in `catch_unwind` to prevent panics from
/// crashing the server. Panics are logged and the event is marked
/// as not handled.
///
/// # Recursion Safety
///
/// A recursion guard prevents infinite loops when handlers emit
/// key events. If the recursion limit (16) is exceeded, the event
/// is dropped and a warning is logged.
///
/// # Example
///
/// ```ignore
/// let _sub = register_key_handler(&bus, SessionId::new(0), |event| {
///     println!("Key: {:?}", event.key);
///     EventResult::Handled
/// });
/// ```
#[allow(dead_code)] // Preserved for future plugin support
pub fn register_key_handler<F>(bus: &EventBus, session_id: SessionId, handler: F) -> Subscription
where
    F: Fn(&KeyPressEvent) -> EventResult + Send + Sync + 'static,
{
    let handler = Arc::new(handler);

    bus.subscribe::<KeyPressEvent, _>(priority::CORE, move |event| {
        // 1. Filter by session ID
        if event.session_id != session_id {
            return EventResult::NotHandled;
        }

        // 2. Check recursion limit
        let Some(_guard) = RecursionGuard::try_enter() else {
            tracing::warn!(
                event_type = %event.event_type(),
                current_depth = RecursionGuard::current_depth(),
                max_depth = RECURSION_LIMIT,
                "Recursion limit exceeded, dropping event"
            );
            return EventResult::NotHandled;
        };

        // 3. Execute with panic isolation
        let handler = Arc::clone(&handler);
        let result = catch_unwind(AssertUnwindSafe(|| handler(event)));

        match result {
            Ok(event_result) => event_result,
            Err(panic_payload) => {
                // Extract panic message for cleaner logging
                let panic_info = panic_payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic_payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("unknown panic");

                tracing::error!(
                    handler_priority = priority::CORE,
                    event_type = %event.event_type(),
                    session_id = %event.session_id,
                    panic_info = panic_info,
                    "Handler panicked during event dispatch"
                );

                EventResult::NotHandled
            }
        }
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::events::{ClientId, KeyCode, KeyInput, Modifiers},
        std::sync::atomic::{AtomicU32, Ordering},
    };

    fn test_key_event(session_id: usize) -> KeyPressEvent {
        KeyPressEvent::new(
            KeyInput {
                key: KeyCode::Char('j'),
                modifiers: Modifiers::NONE,
            },
            SessionId::new(session_id),
            ClientId::new(1),
        )
    }

    #[test]
    fn test_register_key_handler_basic() {
        reset_recursion_depth();
        let bus = EventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let _sub = register_key_handler(&bus, SessionId::new(0), move |_event| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        bus.emit(test_key_event(0));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_register_key_handler_filters_session() {
        reset_recursion_depth();
        let bus = EventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        // Register for session 0
        let _sub = register_key_handler(&bus, SessionId::new(0), move |_event| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        // Emit for session 1 - should NOT trigger handler
        bus.emit(test_key_event(1));
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // Emit for session 0 - should trigger handler
        bus.emit(test_key_event(0));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_register_key_handler_panic_isolation() {
        reset_recursion_depth();
        let bus = EventBus::new();

        let _sub = register_key_handler(&bus, SessionId::new(0), |_event| {
            panic!("test panic");
        });

        // Should not panic - panic is caught
        let result = bus.emit(test_key_event(0));
        assert!(result.is_not_handled());
    }

    #[test]
    fn test_recursion_guard_basic() {
        reset_recursion_depth();

        let guard1 = RecursionGuard::try_enter();
        assert!(guard1.is_some());
        assert_eq!(RecursionGuard::current_depth(), 1);

        let guard2 = RecursionGuard::try_enter();
        assert!(guard2.is_some());
        assert_eq!(RecursionGuard::current_depth(), 2);

        drop(guard2);
        assert_eq!(RecursionGuard::current_depth(), 1);

        drop(guard1);
        assert_eq!(RecursionGuard::current_depth(), 0);
    }

    #[test]
    #[allow(clippy::collection_is_never_read)] // Guards held for RAII, not read
    fn test_recursion_guard_limit() {
        reset_recursion_depth();

        let mut guards = Vec::new();

        // Fill up to the limit
        for i in 0..RECURSION_LIMIT {
            let guard = RecursionGuard::try_enter();
            assert!(guard.is_some(), "Guard {i} should succeed");
            guards.push(guard);
        }

        assert_eq!(RecursionGuard::current_depth(), RECURSION_LIMIT);

        // Next one should fail
        let over_limit = RecursionGuard::try_enter();
        assert!(over_limit.is_none(), "Guard over limit should fail");

        // Cleanup
        guards.clear();
        assert_eq!(RecursionGuard::current_depth(), 0);
    }

    #[test]
    fn test_recursion_guard_boundary_15() {
        reset_recursion_depth();

        // Fill to 15 (one under limit)
        let mut guards: Vec<_> = (0..15)
            .filter_map(|_| RecursionGuard::try_enter())
            .collect();
        assert_eq!(guards.len(), 15);
        assert_eq!(RecursionGuard::current_depth(), 15);

        // 16th should succeed (at the limit)
        let guard_16 = RecursionGuard::try_enter();
        assert!(guard_16.is_some());
        assert_eq!(RecursionGuard::current_depth(), 16);

        // 17th should fail (over the limit)
        let guard_17 = RecursionGuard::try_enter();
        assert!(guard_17.is_none());

        drop(guard_16);
        guards.clear();
        assert_eq!(RecursionGuard::current_depth(), 0);
    }

    #[test]
    fn test_subscription_cleanup() {
        reset_recursion_depth();
        let bus = EventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);

        {
            let _sub = register_key_handler(&bus, SessionId::new(0), move |_event| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                EventResult::Handled
            });

            bus.emit(test_key_event(0));
            assert_eq!(call_count.load(Ordering::SeqCst), 1);
        } // Subscription dropped here

        // Handler should be unregistered - emit should not call handler
        bus.emit(test_key_event(0));
        // Count should still be 1 (not incremented)
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_handlers_same_session() {
        reset_recursion_depth();
        let bus = EventBus::new();
        let count1 = Arc::new(AtomicU32::new(0));
        let count2 = Arc::new(AtomicU32::new(0));
        let count1_clone = Arc::clone(&count1);
        let count2_clone = Arc::clone(&count2);

        let _sub1 = register_key_handler(&bus, SessionId::new(0), move |_event| {
            count1_clone.fetch_add(1, Ordering::SeqCst);
            EventResult::NotHandled // Allow propagation
        });

        let _sub2 = register_key_handler(&bus, SessionId::new(0), move |_event| {
            count2_clone.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

        bus.emit(test_key_event(0));

        // Both handlers should be called (same priority, both get the event)
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
