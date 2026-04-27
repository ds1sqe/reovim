use {super::*, reovim_kernel::api::v1::ServiceRegistry};

struct MockExpander {
    called: std::sync::atomic::AtomicBool,
}

impl MockExpander {
    fn new() -> Self {
        Self {
            called: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl SnippetExpander for MockExpander {
    fn expand(
        &self,
        _runtime: &mut SessionRuntime<'_>,
        _buffer_id: BufferId,
        _insert_pos: Position,
        _snippet_body: &str,
    ) {
        self.called
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl std::fmt::Debug for MockExpander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockExpander").finish()
    }
}

#[test]
fn registry_default_is_empty() {
    let reg = SnippetExpanderRegistry::default();
    assert!(reg.get().is_none());
}

#[test]
fn registry_new_is_empty() {
    let reg = SnippetExpanderRegistry::new();
    assert!(reg.get().is_none());
}

#[test]
fn registry_register_and_get() {
    let reg = SnippetExpanderRegistry::new();
    let mock = Arc::new(MockExpander::new());
    reg.register(mock);
    assert!(reg.get().is_some());
}

#[test]
fn registry_overwrite() {
    let reg = SnippetExpanderRegistry::new();
    reg.register(Arc::new(MockExpander::new()));
    reg.register(Arc::new(MockExpander::new()));
    assert!(reg.get().is_some());
}

#[test]
fn registry_get_returns_cloned_arc() {
    let reg = SnippetExpanderRegistry::new();
    let mock: Arc<dyn SnippetExpander> = Arc::new(MockExpander::new());
    reg.register(Arc::clone(&mock));
    let got = reg.get().unwrap();
    assert!(Arc::ptr_eq(&mock, &got));
}

#[test]
fn registry_service_impl() {
    let services = ServiceRegistry::new();
    let reg = Arc::new(SnippetExpanderRegistry::new());
    services.register(reg);
    assert!(services.get::<SnippetExpanderRegistry>().is_some());
}

#[test]
fn registry_debug() {
    let reg = SnippetExpanderRegistry::new();
    let debug = format!("{reg:?}");
    assert!(debug.contains("SnippetExpanderRegistry"));
    assert!(debug.contains("false"));

    reg.register(Arc::new(MockExpander::new()));
    let debug = format!("{reg:?}");
    assert!(debug.contains("true"));
}

#[test]
fn registry_concurrent_access() {
    let reg = Arc::new(SnippetExpanderRegistry::new());

    let reg_clone = Arc::clone(&reg);
    let writer = std::thread::spawn(move || {
        reg_clone.register(Arc::new(MockExpander::new()));
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
    fn _accepts_ref(_: &dyn SnippetExpander) {}
    fn _accepts_box(_: Box<dyn SnippetExpander>) {}
}

#[test]
fn new_and_default_equivalent() {
    let new = SnippetExpanderRegistry::new();
    let default = SnippetExpanderRegistry::default();
    assert!(new.get().is_none());
    assert!(default.get().is_none());
}
