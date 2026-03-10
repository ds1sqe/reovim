//! Tick scheduler trait for periodic extension state advancement (#546).
//!
//! Defined in the driver layer (mechanism). Implemented in the server layer
//! (e.g., `TokioTickScheduler`). Modules call start/stop from command handlers.
//!
//! # Architecture
//!
//! ```text
//! TickScheduler trait (driver)  →  WHAT a scheduler does
//! TokioTickScheduler (server)   →  HOW (tokio::spawn + interval)
//! Command handlers (module)     →  WHEN to start/stop/pause/resume
//! ```

use std::{sync::Arc, time::Duration};

use {parking_lot::Mutex, reovim_kernel::api::v1::Service};

use crate::ClientId;

/// Trait for scheduling periodic tick callbacks.
///
/// The scheduler runs [`ExtensionStateBridge::tick()`](crate::bridges::ExtensionStateBridge::tick)
/// at regular intervals for a given client. This enables server-driven state
/// advancement independent of key input (e.g., gravity in a game).
///
/// # Usage
///
/// ```ignore
/// // Start ticking for a client (e.g., game start)
/// scheduler.start(client_id, "tetromino", Duration::from_millis(50));
///
/// // Pause/resume (e.g., game pause)
/// scheduler.pause(client_id, "tetromino");
/// scheduler.resume(client_id, "tetromino");
///
/// // Stop ticking (e.g., game quit)
/// scheduler.stop(client_id, "tetromino");
/// ```
pub trait TickScheduler: Send + Sync + 'static {
    /// Start ticking for a client.
    ///
    /// Spawns a periodic task that calls `ExtensionStateBridge::tick()` every
    /// `interval`. The `kind` identifies which bridge to tick (e.g., `"tetromino"`).
    ///
    /// If a tick is already running for this `(client_id, kind)`, it is
    /// stopped and restarted with the new interval.
    fn start(&self, client_id: ClientId, kind: &'static str, interval: Duration);

    /// Stop ticking for a client.
    ///
    /// Cancels the periodic task. No-op if not running.
    fn stop(&self, client_id: ClientId, kind: &'static str);

    /// Pause ticking for a client.
    ///
    /// The task remains alive but skips tick calls. No-op if not running.
    fn pause(&self, client_id: ClientId, kind: &'static str);

    /// Resume ticking for a client.
    ///
    /// Resumes tick calls after a pause. No-op if not paused or not running.
    fn resume(&self, client_id: ClientId, kind: &'static str);
}

/// Service handle for accessing the tick scheduler from modules.
///
/// Registered as a [`Service`] in the kernel's `ServiceRegistry` during module
/// `init()`. The server layer populates the inner scheduler at startup via
/// [`set()`](Self::set).
///
/// This follows the [`BridgeProvider`](crate::bridges::BridgeProvider) pattern:
/// driver-layer type, populated by server, consumed by modules. Modules never
/// see the concrete scheduler implementation.
pub struct TickSchedulerHandle {
    inner: Mutex<Option<Arc<dyn TickScheduler>>>,
}

impl Default for TickSchedulerHandle {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

impl Service for TickSchedulerHandle {}

impl TickSchedulerHandle {
    /// Set the scheduler implementation. Called by server layer at startup.
    pub fn set(&self, scheduler: Arc<dyn TickScheduler>) {
        *self.inner.lock() = Some(scheduler);
    }

    /// Start ticking. No-op if scheduler not set.
    pub fn start(&self, client_id: ClientId, kind: &'static str, interval: Duration) {
        if let Some(ref s) = *self.inner.lock() {
            s.start(client_id, kind, interval);
        }
    }

    /// Stop ticking. No-op if scheduler not set.
    pub fn stop(&self, client_id: ClientId, kind: &'static str) {
        if let Some(ref s) = *self.inner.lock() {
            s.stop(client_id, kind);
        }
    }

    /// Pause ticking. No-op if scheduler not set.
    pub fn pause(&self, client_id: ClientId, kind: &'static str) {
        if let Some(ref s) = *self.inner.lock() {
            s.pause(client_id, kind);
        }
    }

    /// Resume ticking. No-op if scheduler not set.
    pub fn resume(&self, client_id: ClientId, kind: &'static str) {
        if let Some(ref s) = *self.inner.lock() {
            s.resume(client_id, kind);
        }
    }
}

impl std::fmt::Debug for TickSchedulerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_scheduler = self.inner.lock().is_some();
        f.debug_struct("TickSchedulerHandle")
            .field("has_scheduler", &has_scheduler)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TickSchedulerHandle tests
    // =========================================================================

    #[test]
    fn handle_default_has_no_scheduler() {
        let handle = TickSchedulerHandle::default();
        assert!(handle.inner.lock().is_none());
    }

    #[test]
    fn handle_start_noop_when_empty() {
        let handle = TickSchedulerHandle::default();
        // Should not panic
        handle.start(ClientId::new(1), "test", Duration::from_millis(50));
    }

    #[test]
    fn handle_stop_noop_when_empty() {
        let handle = TickSchedulerHandle::default();
        handle.stop(ClientId::new(1), "test");
    }

    #[test]
    fn handle_pause_noop_when_empty() {
        let handle = TickSchedulerHandle::default();
        handle.pause(ClientId::new(1), "test");
    }

    #[test]
    fn handle_resume_noop_when_empty() {
        let handle = TickSchedulerHandle::default();
        handle.resume(ClientId::new(1), "test");
    }

    #[test]
    fn handle_set_and_delegates() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[allow(clippy::struct_field_names)]
        struct CountingScheduler {
            start_count: AtomicUsize,
            stop_count: AtomicUsize,
            pause_count: AtomicUsize,
            resume_count: AtomicUsize,
        }

        impl TickScheduler for CountingScheduler {
            fn start(&self, _: ClientId, _: &'static str, _: Duration) {
                self.start_count.fetch_add(1, Ordering::Relaxed);
            }
            fn stop(&self, _: ClientId, _: &'static str) {
                self.stop_count.fetch_add(1, Ordering::Relaxed);
            }
            fn pause(&self, _: ClientId, _: &'static str) {
                self.pause_count.fetch_add(1, Ordering::Relaxed);
            }
            fn resume(&self, _: ClientId, _: &'static str) {
                self.resume_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        let scheduler = Arc::new(CountingScheduler {
            start_count: AtomicUsize::new(0),
            stop_count: AtomicUsize::new(0),
            pause_count: AtomicUsize::new(0),
            resume_count: AtomicUsize::new(0),
        });

        let handle = TickSchedulerHandle::default();
        handle.set(Arc::clone(&scheduler) as Arc<dyn TickScheduler>);

        handle.start(ClientId::new(1), "test", Duration::from_millis(50));
        handle.stop(ClientId::new(1), "test");
        handle.pause(ClientId::new(1), "test");
        handle.resume(ClientId::new(1), "test");

        assert_eq!(scheduler.start_count.load(Ordering::Relaxed), 1);
        assert_eq!(scheduler.stop_count.load(Ordering::Relaxed), 1);
        assert_eq!(scheduler.pause_count.load(Ordering::Relaxed), 1);
        assert_eq!(scheduler.resume_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn handle_debug() {
        let handle = TickSchedulerHandle::default();
        let debug = format!("{handle:?}");
        assert!(debug.contains("TickSchedulerHandle"));
        assert!(debug.contains("false"));
    }

    #[test]
    fn handle_service_trait() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let registry = ServiceRegistry::new();
        let handle = registry.get_or_create::<TickSchedulerHandle>();
        assert!(handle.inner.lock().is_none());

        // Same instance via get_or_create
        let handle2 = registry.get_or_create::<TickSchedulerHandle>();
        assert!(Arc::ptr_eq(&handle, &handle2));
    }

    // =========================================================================
    // TickScheduler trait object safety
    // =========================================================================

    #[test]
    fn trait_is_object_safe() {
        struct NoopScheduler;
        impl TickScheduler for NoopScheduler {
            fn start(&self, _: ClientId, _: &'static str, _: Duration) {}
            fn stop(&self, _: ClientId, _: &'static str) {}
            fn pause(&self, _: ClientId, _: &'static str) {}
            fn resume(&self, _: ClientId, _: &'static str) {}
        }

        let _boxed: Box<dyn TickScheduler> = Box::new(NoopScheduler);
        let _arc: Arc<dyn TickScheduler> = Arc::new(NoopScheduler);
    }
}
