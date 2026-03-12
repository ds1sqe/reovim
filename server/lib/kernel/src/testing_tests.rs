use crate::testing::*;

use crate::api::v1::{Buffer, BufferError, BufferId, BufferManager};

#[test]
fn test_buffer_manager_create_and_get() {
    let manager = TestBufferManager::new();
    let id = manager.create();
    assert!(manager.get(id).is_some());
}

#[test]
fn test_buffer_manager_register_and_get() {
    let manager = TestBufferManager::new();
    let buffer = Buffer::from_string("hello");
    let id = manager.register(buffer);
    let retrieved = manager.get(id).unwrap();
    assert_eq!(retrieved.read().line(0), Some("hello"));
}

#[test]
fn test_buffer_manager_get_nonexistent() {
    let manager = TestBufferManager::new();
    assert!(manager.get(BufferId::from_raw(999)).is_none());
}

#[test]
fn test_buffer_manager_unregister() {
    let manager = TestBufferManager::new();
    let buffer = Buffer::from_string("test");
    let id = manager.register(buffer);
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(unregistered.line(0), Some("test"));
    assert!(manager.get(id).is_none());
}

#[test]
fn test_buffer_manager_unregister_nonexistent() {
    let manager = TestBufferManager::new();
    let id = BufferId::from_raw(999);
    assert_eq!(manager.unregister(id).unwrap_err(), BufferError::NotFound(id));
}

#[test]
fn test_buffer_manager_unregister_with_extra_ref() {
    let manager = TestBufferManager::new();
    let buffer = Buffer::from_string("shared");
    let id = manager.register(buffer);
    // Hold an extra reference to prevent Arc::try_unwrap from succeeding
    let _extra_ref = manager.get(id).unwrap();
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(unregistered.line(0), Some("shared"));
}

#[test]
fn test_buffer_manager_list() {
    let manager = TestBufferManager::new();
    assert!(manager.list().is_empty());
    let id1 = manager.create();
    let id2 = manager.create();
    let list = manager.list();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&id1));
    assert!(list.contains(&id2));
}

#[test]
fn test_buffer_manager_count() {
    let manager = TestBufferManager::new();
    assert_eq!(manager.count(), 0);
    manager.create();
    assert_eq!(manager.count(), 1);
    manager.create();
    assert_eq!(manager.count(), 2);
}

#[test]
fn test_buffer_manager_default() {
    let manager = TestBufferManager::default();
    assert_eq!(manager.count(), 0);
}

#[test]
fn test_create_test_context() {
    let ctx = create_test_context();
    // Buffer manager should work (not the stub)
    let buffer = Buffer::from_string("test content");
    let id = ctx.buffers.register(buffer);
    let retrieved = ctx.buffers.get(id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().read().line(0), Some("test content"));
}

#[test]
fn test_setup_buffer() {
    let ctx = create_test_context();
    let id = setup_buffer(&ctx, "hello\nworld");
    let buf = ctx.buffers.get(id).unwrap();
    let guard = buf.read();
    assert_eq!(guard.line(0), Some("hello"));
    assert_eq!(guard.line(1), Some("world"));
    drop(guard);
}
