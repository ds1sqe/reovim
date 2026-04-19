use std::sync::Arc;

use {
    super::lookup_policy_store::LookupPolicyStore,
    crate::{EagerLookupPolicy, KeyLookupResult, KeyLookupState},
};

#[test]
fn test_new_store_has_no_policy() {
    let store = LookupPolicyStore::new();
    assert!(!store.has_policy());
}

#[test]
fn test_default_store_has_no_policy() {
    let store = LookupPolicyStore::default();
    assert!(!store.has_policy());
}

#[test]
fn test_set_policy() {
    let store = LookupPolicyStore::new();
    store.set(Arc::new(EagerLookupPolicy));
    assert!(store.has_policy());
}

#[test]
fn test_take_returns_policy() {
    let store = LookupPolicyStore::new();
    store.set(Arc::new(EagerLookupPolicy));
    let policy = store.take();
    assert!(policy.is_some());
    assert!(!store.has_policy());
}

#[test]
fn test_take_empty_returns_none() {
    let store = LookupPolicyStore::new();
    assert!(store.take().is_none());
}

#[test]
fn test_taken_policy_works() {
    let store = LookupPolicyStore::new();
    store.set(Arc::new(EagerLookupPolicy));
    let policy = store.take().unwrap();
    let cmd = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "cmd",
    );
    let result = policy.resolve(KeyLookupState::ExactOnly(cmd.clone()));
    assert_eq!(result, KeyLookupResult::Found(cmd));
}

#[test]
fn test_set_replaces_existing_policy() {
    let store = LookupPolicyStore::new();
    store.set(Arc::new(EagerLookupPolicy));
    store.set(Arc::new(EagerLookupPolicy));
    assert!(store.has_policy());
    let _ = store.take();
    assert!(!store.has_policy());
}

#[test]
fn test_debug_format() {
    let store = LookupPolicyStore::new();
    let debug = format!("{store:?}");
    assert!(debug.contains("LookupPolicyStore"));
    assert!(debug.contains("has_policy"));
}

#[test]
fn test_service_impl() {
    use reovim_kernel::api::v1::{Service, ServiceRegistry};

    fn assert_service<T: Service>() {}
    assert_service::<LookupPolicyStore>();

    let registry = ServiceRegistry::new();
    registry.register(Arc::new(LookupPolicyStore::new()));
    let store = registry.get::<LookupPolicyStore>();
    assert!(store.is_some());
}
