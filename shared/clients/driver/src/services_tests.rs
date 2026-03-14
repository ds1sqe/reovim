use std::sync::Arc;

use super::ClientServiceRegistry;

struct FooService(u32);
struct BarService(String);

#[test]
fn register_and_get() {
    let registry = ClientServiceRegistry::new();
    registry.register(Arc::new(FooService(42)));
    let svc = registry.get::<FooService>().unwrap();
    assert_eq!(svc.0, 42);
}

#[test]
fn get_missing_returns_none() {
    let registry = ClientServiceRegistry::new();
    assert!(registry.get::<FooService>().is_none());
}

#[test]
fn replace_existing_service() {
    let registry = ClientServiceRegistry::new();
    registry.register(Arc::new(FooService(1)));
    registry.register(Arc::new(FooService(2)));
    let svc = registry.get::<FooService>().unwrap();
    assert_eq!(svc.0, 2);
}

#[test]
fn multiple_types() {
    let registry = ClientServiceRegistry::new();
    registry.register(Arc::new(FooService(10)));
    registry.register(Arc::new(BarService("hello".to_string())));
    assert_eq!(registry.get::<FooService>().unwrap().0, 10);
    assert_eq!(registry.get::<BarService>().unwrap().0, "hello");
}

#[test]
fn contains_registered() {
    let registry = ClientServiceRegistry::new();
    assert!(!registry.contains::<FooService>());
    registry.register(Arc::new(FooService(0)));
    assert!(registry.contains::<FooService>());
}

#[test]
fn len_and_is_empty() {
    let registry = ClientServiceRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    registry.register(Arc::new(FooService(0)));
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    registry.register(Arc::new(BarService(String::new())));
    assert_eq!(registry.len(), 2);
}

#[test]
fn default_is_empty() {
    let registry = ClientServiceRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn debug_impl() {
    let registry = ClientServiceRegistry::new();
    let debug = format!("{registry:?}");
    assert!(debug.contains("ClientServiceRegistry"));
}
