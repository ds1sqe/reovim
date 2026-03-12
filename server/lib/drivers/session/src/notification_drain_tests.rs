use {super::*, reovim_kernel::api::v1::ServiceRegistry};

struct MockDrain {
    called: std::sync::atomic::AtomicBool,
}

impl MockDrain {
    fn new() -> Self {
        Self {
            called: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl NotificationDrain for MockDrain {
    fn drain_pending(&self, _runtime: &mut SessionRuntime<'_>) {
        self.called
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl std::fmt::Debug for MockDrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockDrain").finish()
    }
}

#[test]
fn registry_default_is_empty() {
    let reg = NotificationDrainRegistry::default();
    assert!(reg.get().is_none());
}

#[test]
fn registry_new_is_empty() {
    let reg = NotificationDrainRegistry::new();
    assert!(reg.get().is_none());
}

#[test]
fn registry_register_and_get() {
    let reg = NotificationDrainRegistry::new();
    let mock = Arc::new(MockDrain::new());
    reg.register(mock);
    assert!(reg.get().is_some());
}

#[test]
fn registry_overwrite() {
    let reg = NotificationDrainRegistry::new();
    reg.register(Arc::new(MockDrain::new()));
    reg.register(Arc::new(MockDrain::new()));
    assert!(reg.get().is_some());
}

#[test]
fn registry_get_returns_cloned_arc() {
    let reg = NotificationDrainRegistry::new();
    let mock: Arc<dyn NotificationDrain> = Arc::new(MockDrain::new());
    reg.register(Arc::clone(&mock));
    let got = reg.get().unwrap();
    assert!(Arc::ptr_eq(&mock, &got));
}

#[test]
fn registry_service_impl() {
    let services = ServiceRegistry::new();
    let reg = Arc::new(NotificationDrainRegistry::new());
    services.register(reg);
    assert!(services.get::<NotificationDrainRegistry>().is_some());
}

#[test]
fn registry_debug() {
    let reg = NotificationDrainRegistry::new();
    let debug = format!("{reg:?}");
    assert!(debug.contains("NotificationDrainRegistry"));
    assert!(debug.contains("false"));

    reg.register(Arc::new(MockDrain::new()));
    let debug = format!("{reg:?}");
    assert!(debug.contains("true"));
}

#[test]
fn registry_concurrent_access() {
    let reg = Arc::new(NotificationDrainRegistry::new());

    let reg_clone = Arc::clone(&reg);
    let writer = std::thread::spawn(move || {
        reg_clone.register(Arc::new(MockDrain::new()));
    });
    writer.join().unwrap();

    let mut readers = Vec::new();
    for _ in 0..4 {
        let r = Arc::clone(&reg);
        readers.push(std::thread::spawn(move || r.get().is_some()));
    }
    for handle in readers {
        assert!(handle.join().unwrap());
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn trait_is_object_safe() {
    fn _accepts_ref(_: &dyn NotificationDrain) {}
    fn _accepts_box(_: Box<dyn NotificationDrain>) {}
}

#[test]
fn new_and_default_equivalent() {
    let new = NotificationDrainRegistry::new();
    let default = NotificationDrainRegistry::default();
    assert!(new.get().is_none());
    assert!(default.get().is_none());
}
