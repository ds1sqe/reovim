use super::*;

#[test]
fn test_create_buffer() {
    let mgr = SimpleBufferManager::new();
    let id = mgr.create();
    assert!(mgr.get(id).is_some());
    assert_eq!(mgr.count(), 1);
}

#[test]
fn test_register_buffer() {
    let mgr = SimpleBufferManager::new();
    let buffer = Buffer::from_string("hello");
    let id = mgr.register(buffer);
    assert!(mgr.get(id).is_some());
}

#[test]
fn test_unregister_buffer() {
    let mgr = SimpleBufferManager::new();
    let id = mgr.create();
    let result = mgr.unregister(id);
    assert!(result.is_ok());
    assert!(mgr.get(id).is_none());
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_unregister_not_found() {
    let mgr = SimpleBufferManager::new();
    let fake_id = BufferId::new();
    let result = mgr.unregister(fake_id);
    assert!(matches!(result, Err(BufferError::NotFound(_))));
}

#[test]
fn test_list_buffers() {
    let mgr = SimpleBufferManager::new();
    let id1 = mgr.create();
    let id2 = mgr.create();
    let list = mgr.list();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&id1));
    assert!(list.contains(&id2));
}

#[test]
fn test_module_id() {
    let module = BufferSimpleModule::new();
    assert_eq!(module.id().as_str(), "buffer-simple");
}

#[test]
fn test_module_name() {
    let module = BufferSimpleModule::new();
    assert_eq!(module.name(), "Simple Buffer Manager");
}

#[test]
fn test_module_version() {
    let module = BufferSimpleModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_default_creates_same_as_new() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: BufferSimpleModule = create_default();
    let from_new = BufferSimpleModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_simple_buffer_manager_default() {
    let mgr = SimpleBufferManager::default();
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_exit_succeeds() {
    let mut module = BufferSimpleModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_register_buffer_preserves_content() {
    let mgr = SimpleBufferManager::new();
    let buffer = Buffer::from_string("hello world");
    let id = mgr.register(buffer);
    let retrieved = mgr.get(id).unwrap();
    let content = retrieved.read().content();
    assert_eq!(content, "hello world");
}

#[test]
fn test_unregister_returns_buffer_content() {
    let mgr = SimpleBufferManager::new();
    let buffer = Buffer::from_string("test content");
    let id = mgr.register(buffer);
    let result = mgr.unregister(id).unwrap();
    assert_eq!(result.content(), "test content");
}

#[test]
fn test_get_nonexistent_returns_none() {
    let mgr = SimpleBufferManager::new();
    let fake_id = BufferId::new();
    assert!(mgr.get(fake_id).is_none());
}

#[test]
fn test_list_empty_manager() {
    let mgr = SimpleBufferManager::new();
    assert!(mgr.list().is_empty());
}

#[test]
fn test_count_starts_at_zero() {
    let mgr = SimpleBufferManager::new();
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_create_multiple_buffers() {
    let mgr = SimpleBufferManager::new();
    let id1 = mgr.create();
    let id2 = mgr.create();
    let id3 = mgr.create();
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
    assert_eq!(mgr.count(), 3);
}

#[test]
fn test_unregister_with_shared_reference() {
    let mgr = SimpleBufferManager::new();
    let buffer = Buffer::from_string("shared");
    let id = mgr.register(buffer);

    // Hold a clone of the Arc to simulate shared reference
    let _extra_ref = mgr.get(id).unwrap();

    // Unregister should still work by cloning the buffer
    let result = mgr.unregister(id);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().content(), "shared");
}

#[test]
fn test_dependencies_default_empty() {
    let module = BufferSimpleModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_init_registers_buffer_manager() {
    use {
        reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
        std::{path::PathBuf, sync::Arc},
    };

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = BufferSimpleModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify that BufferManagerRegistry was created in services
    let registry = services.get::<BufferManagerRegistry>();
    assert!(registry.is_some(), "BufferManagerRegistry should be registered in services");
}
