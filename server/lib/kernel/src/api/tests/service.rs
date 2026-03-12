use super::*;
use std::sync::Arc;

// Test service type
struct TestService {
    value: i32,
}

impl Service for TestService {}

#[test]
fn test_service_registry_register_and_get() {
    let registry = ServiceRegistry::new();

    registry.register(Arc::new(TestService { value: 42 }));

    let service = registry.get::<TestService>();
    assert!(service.is_some());
    assert_eq!(service.unwrap().value, 42);
}

#[test]
fn test_service_registry_get_nonexistent() {
    let registry = ServiceRegistry::new();

    let service = registry.get::<TestService>();
    assert!(service.is_none());
}

#[test]
fn test_service_registry_replace() {
    let registry = ServiceRegistry::new();

    registry.register(Arc::new(TestService { value: 1 }));
    registry.register(Arc::new(TestService { value: 2 }));

    let service = registry.get::<TestService>().unwrap();
    assert_eq!(service.value, 2);
}

#[test]
#[should_panic(expected = "FATAL: No provider for TestService")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_service_registry_get_required_panics() {
    let registry = ServiceRegistry::new();
    let _: Arc<TestService> = registry.get_required("TestService");
}

// Test service key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TestKey {
    Primary,
    Secondary,
}

impl ServiceKey for TestKey {
    fn service_name() -> &'static str {
        "Test"
    }
}

// Test provider trait
trait TestProvider: Send + Sync {
    fn name(&self) -> &'static str;
}

struct PrimaryProvider;
impl TestProvider for PrimaryProvider {
    fn name(&self) -> &'static str {
        "primary"
    }
}

struct SecondaryProvider;
impl TestProvider for SecondaryProvider {
    fn name(&self) -> &'static str {
        "secondary"
    }
}

#[test]
fn test_multi_registry_register_and_get() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));
    registry.register(TestKey::Secondary, Arc::new(SecondaryProvider));

    let primary = registry.get(&TestKey::Primary).unwrap();
    assert_eq!(primary.name(), "primary");

    let secondary = registry.get(&TestKey::Secondary).unwrap();
    assert_eq!(secondary.name(), "secondary");
}

#[test]
fn test_multi_registry_get_nonexistent() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    let provider = registry.get(&TestKey::Primary);
    assert!(provider.is_none());
}

#[test]
fn test_multi_registry_keys() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));
    registry.register(TestKey::Secondary, Arc::new(SecondaryProvider));

    let keys = registry.keys();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&TestKey::Primary));
    assert!(keys.contains(&TestKey::Secondary));
}

#[test]
fn test_multi_registry_values() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));
    registry.register(TestKey::Secondary, Arc::new(SecondaryProvider));

    let values = registry.values();
    assert_eq!(values.len(), 2);
}

#[test]
fn test_multi_registry_values_empty() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();
    assert!(registry.values().is_empty());
}

#[test]
fn test_multi_registry_contains() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

    assert!(registry.contains(&TestKey::Primary));
    assert!(!registry.contains(&TestKey::Secondary));
}

#[test]
fn test_multi_registry_len_and_is_empty() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
}

#[test]
#[should_panic(expected = "FATAL: No Test provider for Primary")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_multi_registry_get_required_panics() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();
    let _: Arc<dyn TestProvider> = registry.get_required(&TestKey::Primary);
}

#[test]
fn test_service_key_service_name() {
    assert_eq!(TestKey::service_name(), "Test");
}

// ========== ServiceRegistry Debug ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_service_registry_debug() {
    let registry = ServiceRegistry::new();
    registry.register(Arc::new(TestService { value: 1 }));

    let debug_str = format!("{registry:?}");
    assert!(debug_str.contains("ServiceRegistry"));
    assert!(debug_str.contains("registered_services"));
}

// ========== ServiceRegistry Default ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_service_registry_default() {
    let registry = ServiceRegistry::default();
    let debug_str = format!("{registry:?}");
    assert!(debug_str.contains('0')); // 0 registered services
}

// ========== get_or_create ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_service_registry_get_or_create() {
    #[derive(Default)]
    struct DefaultableService {
        value: i32,
    }
    impl Service for DefaultableService {}

    let registry = ServiceRegistry::new();

    // First call creates the service
    let svc1 = registry.get_or_create::<DefaultableService>();
    assert_eq!(svc1.value, 0); // Default value

    // Second call returns the cached service
    let svc2 = registry.get_or_create::<DefaultableService>();
    assert_eq!(Arc::as_ptr(&svc1), Arc::as_ptr(&svc2));
}

// ========== MultiServiceRegistry Debug ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_multi_service_registry_debug() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();
    registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

    let debug_str = format!("{registry:?}");
    assert!(debug_str.contains("MultiServiceRegistry"));
    assert!(debug_str.contains("Test")); // service_name()
}

// ========== MultiServiceRegistry Default ==========

#[test]
fn test_multi_service_registry_default() {
    let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::default();
    assert!(registry.is_empty());
}
