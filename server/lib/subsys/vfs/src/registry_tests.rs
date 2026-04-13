use std::sync::Arc;

use {super::*, crate::MockVfs};

#[test]
fn test_registry_register_and_get() {
    let registry = VfsProviderRegistry::new();

    let mock_vfs = Arc::new(MockVfs::new());
    registry.register(VfsScheme::File, mock_vfs);

    let retrieved = registry.get(&VfsScheme::File);
    assert!(retrieved.is_some());
}

#[test]
fn test_registry_keys() {
    let registry = VfsProviderRegistry::new();

    registry.register(VfsScheme::File, Arc::new(MockVfs::new()));
    registry.register(VfsScheme::Memory, Arc::new(MockVfs::new()));

    let keys = registry.keys();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&VfsScheme::File));
    assert!(keys.contains(&VfsScheme::Memory));
}
