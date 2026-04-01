use std::sync::Arc;

use crate::testing::*;

use crate::api::v1::{Buffer, BufferId, BufferManager, RwLock};

#[test]
fn test_buffer_manager_register_and_get() {
    let manager = TestBufferManager::new();
    let buffer = Buffer::from_string("hello");
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    let retrieved = manager.get(id).unwrap();
    assert_eq!(retrieved.read().line(0).as_deref(), Some("hello"));
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
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(unregistered.read().line(0).as_deref(), Some("test"));
    assert!(manager.get(id).is_none());
}

#[test]
fn test_buffer_manager_unregister_nonexistent() {
    let manager = TestBufferManager::new();
    let id = BufferId::from_raw(999);
    assert!(manager.unregister(id).is_none());
}

#[test]
fn test_buffer_manager_unregister_with_extra_ref() {
    let manager = TestBufferManager::new();
    let buffer = Buffer::from_string("shared");
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    // Hold an extra reference
    let _extra_ref = manager.get(id).unwrap();
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(unregistered.read().line(0).as_deref(), Some("shared"));
}

#[test]
fn test_buffer_manager_list() {
    let manager = TestBufferManager::new();
    assert!(manager.list().is_empty());
    let buf1 = Buffer::new();
    let buf2 = Buffer::new();
    let id1 = manager.register(Arc::new(RwLock::new(buf1)));
    let id2 = manager.register(Arc::new(RwLock::new(buf2)));
    let list = manager.list();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&id1));
    assert!(list.contains(&id2));
}

#[test]
fn test_buffer_manager_count() {
    let manager = TestBufferManager::new();
    assert_eq!(manager.count(), 0);
    manager.register(Arc::new(RwLock::new(Buffer::new())));
    assert_eq!(manager.count(), 1);
    manager.register(Arc::new(RwLock::new(Buffer::new())));
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
    let id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));
    let retrieved = ctx.buffers.get(id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().read().line(0).as_deref(), Some("test content"));
}

#[test]
fn test_setup_buffer() {
    let ctx = create_test_context();
    let id = setup_buffer(&ctx, "hello\nworld");
    let buf = ctx.buffers.get(id).unwrap();
    let guard = buf.read();
    assert_eq!(guard.line(0).as_deref(), Some("hello"));
    assert_eq!(guard.line(1).as_deref(), Some("world"));
    drop(guard);
}
