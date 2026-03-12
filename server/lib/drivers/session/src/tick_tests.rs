
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