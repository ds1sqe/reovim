use super::*;

#[test]
fn test_provider_priority_ordering() {
    assert!(ProviderPriority::Override > ProviderPriority::Default);
}

#[test]
fn test_provider_priority_default() {
    assert_eq!(ProviderPriority::default(), ProviderPriority::Default);
}

#[test]
fn test_provider_priority_debug() {
    let p = ProviderPriority::Default;
    let debug = format!("{p:?}");
    assert!(debug.contains("Default"));
}

#[test]
fn test_provider_priority_clone() {
    let a = ProviderPriority::Override;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_provider_priority_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ProviderPriority::Default);
    set.insert(ProviderPriority::Override);
    set.insert(ProviderPriority::Default); // Duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_provider_priority_not_equal() {
    assert_ne!(ProviderPriority::Default, ProviderPriority::Override);
}

// Test a concrete VfsProvider implementation
struct TestProvider;

impl VfsProvider for TestProvider {
    fn provider_id(&self) -> &ModuleId {
        static ID: std::sync::OnceLock<ModuleId> = std::sync::OnceLock::new();
        ID.get_or_init(|| ModuleId::new("test-provider"))
    }

    fn can_handle(&self, scheme: &str) -> bool {
        scheme == "test" || scheme.is_empty()
    }

    fn create(&self, scheme: &str) -> Option<Arc<dyn crate::VfsDriver>> {
        if self.can_handle(scheme) {
            Some(Arc::new(crate::MockVfs::new()))
        } else {
            None
        }
    }
}

#[test]
fn test_vfs_provider_impl() {
    let provider = TestProvider;

    assert_eq!(provider.provider_id().as_str(), "test-provider");
    assert!(provider.can_handle("test"));
    assert!(provider.can_handle(""));
    assert!(!provider.can_handle("unknown"));
}

#[test]
fn test_vfs_provider_create() {
    let provider = TestProvider;

    let driver = provider.create("test");
    assert!(driver.is_some());

    let driver = provider.create("unknown");
    assert!(driver.is_none());
}

#[test]
fn test_vfs_provider_default_priority() {
    let provider = TestProvider;
    assert_eq!(provider.priority(), ProviderPriority::Default);
}
