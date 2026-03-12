use super::*;

/// Verify that `GitProvider` is object-safe (can be used as `dyn GitProvider`).
#[test]
fn git_provider_is_object_safe() {
    fn assert_object_safe(_: &dyn GitProvider) {}
    // Compile-time check only — never called.
    let _ = assert_object_safe;
}

/// Verify that `dyn GitProvider` satisfies Send + Sync.
#[test]
fn git_provider_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Box<dyn GitProvider>>();
    assert_sync::<Box<dyn GitProvider>>();
}
