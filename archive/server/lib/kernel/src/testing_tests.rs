use std::sync::Arc;

use crate::{
    api::v1::{BufferId, BufferManager, RwLock},
    testing::*,
};

/// Helper: read all bytes from a `dyn KernelBuffer` as a UTF-8 string.
fn read_content(buf: &dyn crate::api::v1::KernelBuffer) -> String {
    let len = buf.byte_len();
    let mut bytes = vec![0u8; len];
    buf.read_bytes(0, &mut bytes);
    String::from_utf8(bytes).expect("test buffer should be UTF-8")
}

#[test]
fn test_buffer_manager_register_and_get() {
    let manager = TestBufferManager::new();
    let buffer = TestTextBuffer::new("hello");
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    let retrieved = manager.get(id).unwrap();
    assert_eq!(read_content(&*retrieved.read()), "hello");
}

#[test]
fn test_buffer_manager_get_nonexistent() {
    let manager = TestBufferManager::new();
    assert!(manager.get(BufferId::from_raw(999)).is_none());
}

#[test]
fn test_buffer_manager_unregister() {
    let manager = TestBufferManager::new();
    let buffer = TestTextBuffer::new("test");
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(read_content(&*unregistered.read()), "test");
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
    let buffer = TestTextBuffer::new("shared");
    let id = manager.register(Arc::new(RwLock::new(buffer)));
    // Hold an extra reference
    let _extra_ref = manager.get(id).unwrap();
    let unregistered = manager.unregister(id).unwrap();
    assert_eq!(read_content(&*unregistered.read()), "shared");
}

#[test]
fn test_buffer_manager_list() {
    let manager = TestBufferManager::new();
    assert!(manager.list().is_empty());
    let buf1 = TestTextBuffer::new("");
    let buf2 = TestTextBuffer::new("");
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
    manager.register(Arc::new(RwLock::new(TestTextBuffer::new(""))));
    assert_eq!(manager.count(), 1);
    manager.register(Arc::new(RwLock::new(TestTextBuffer::new(""))));
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
    let buffer = TestTextBuffer::new("test content");
    let id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));
    let retrieved = ctx.buffers.get(id);
    assert!(retrieved.is_some());
    assert_eq!(read_content(&*retrieved.unwrap().read()), "test content");
}

#[test]
fn test_register_text_buffer() {
    let ctx = create_test_context();
    let id = register_text_buffer(&ctx, "hello\nworld");
    let buf = ctx.buffers.get(id).unwrap();
    let guard = buf.read();
    assert_eq!(read_content(&*guard), "hello\nworld");
    drop(guard);
}
