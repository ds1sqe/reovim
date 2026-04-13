use std::sync::Arc;

use reovim_kernel::api::v1::ServiceRegistry;

use super::*;

// ========================================================================
// ComponentData tests
// ========================================================================

#[test]
fn test_data_new() {
    let data = ComponentData::new("hello");
    assert_eq!(data.text, "hello");
    assert!(data.visible);
    assert!(data.min_width.is_none());
    assert_eq!(data.truncation_priority, 128);
}

#[test]
fn test_data_hidden() {
    let data = ComponentData::hidden();
    assert!(data.text.is_empty());
    assert!(!data.visible);
    assert_eq!(data.truncation_priority, 0);
}

#[test]
fn test_data_default_is_hidden() {
    let data = ComponentData::default();
    assert!(!data.visible);
}

#[test]
fn test_data_when_true() {
    let data = ComponentData::new("text").when(true);
    assert!(data.visible);
}

#[test]
fn test_data_when_false() {
    let data = ComponentData::new("text").when(false);
    assert!(!data.visible);
}

#[test]
fn test_data_with_min_width() {
    let data = ComponentData::new("text").with_min_width(10);
    assert_eq!(data.min_width, Some(10));
}

#[test]
fn test_data_with_priority() {
    let data = ComponentData::new("text").with_priority(200);
    assert_eq!(data.truncation_priority, 200);
}

#[test]
fn test_data_clone() {
    let data = ComponentData::new("hello")
        .with_min_width(5)
        .with_priority(42);
    #[allow(clippy::redundant_clone)]
    let cloned = data.clone();
    assert_eq!(cloned.text, "hello");
    assert_eq!(cloned.min_width, Some(5));
    assert_eq!(cloned.truncation_priority, 42);
}

#[test]
fn test_data_debug() {
    let data = ComponentData::new("test");
    let debug = format!("{data:?}");
    assert!(debug.contains("ComponentData"));
}

// ========================================================================
// ComponentDataProviderKey tests
// ========================================================================

#[test]
fn test_key_new() {
    let key = ComponentDataProviderKey::new("branch");
    assert_eq!(key.id(), "branch");
}

#[test]
fn test_key_equality() {
    let k1 = ComponentDataProviderKey::new("branch");
    let k2 = ComponentDataProviderKey::new("branch");
    let k3 = ComponentDataProviderKey::new("mode");
    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn test_key_clone() {
    let key = ComponentDataProviderKey::new("branch");
    let cloned = key.clone();
    assert_eq!(key, cloned);
}

#[test]
fn test_key_debug() {
    let key = ComponentDataProviderKey::new("test");
    let debug = format!("{key:?}");
    assert!(debug.contains("test"));
}

// ========================================================================
// ComponentDataProvider trait tests
// ========================================================================

struct TestProvider;

impl ComponentDataProvider for TestProvider {
    fn id(&self) -> &'static str {
        "test"
    }

    fn data(&self, ctx: &ComponentDataContext) -> ComponentData {
        ctx.git_branch
            .as_ref()
            .map_or_else(ComponentData::hidden, |branch| ComponentData::new(format!(" {branch} ")))
    }
}

#[test]
fn test_provider_id() {
    let provider = TestProvider;
    assert_eq!(provider.id(), "test");
}

#[test]
fn test_provider_data_visible() {
    let provider = TestProvider;
    let ctx = ComponentDataContext {
        git_branch: Some("main".to_string()),
        ..ComponentDataContext::default()
    };
    let data = provider.data(&ctx);
    assert!(data.visible);
    assert_eq!(data.text, " main ");
}

#[test]
fn test_provider_data_hidden() {
    let provider = TestProvider;
    let ctx = ComponentDataContext::default();
    let data = provider.data(&ctx);
    assert!(!data.visible);
}

#[test]
fn test_provider_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TestProvider>();
}

// ========================================================================
// Registry tests
// ========================================================================

#[test]
fn test_registry_register_and_get() {
    let registry = ComponentDataProviderRegistry::new();
    registry.register(ComponentDataProviderKey::new("test"), Arc::new(TestProvider));
    let provider = registry.get(&ComponentDataProviderKey::new("test"));
    assert!(provider.is_some());
    assert_eq!(provider.unwrap().id(), "test");
}

#[test]
fn test_registry_empty() {
    let registry = ComponentDataProviderRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_values() {
    let registry = ComponentDataProviderRegistry::new();
    registry.register(ComponentDataProviderKey::new("test"), Arc::new(TestProvider));
    let values = registry.values();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].id(), "test");
}

#[test]
fn test_registry_in_service_registry() {
    let services = ServiceRegistry::new();
    let registry = services.get_or_create::<ComponentDataProviderRegistry>();
    registry.register(ComponentDataProviderKey::new("test"), Arc::new(TestProvider));
    assert_eq!(registry.len(), 1);
}
