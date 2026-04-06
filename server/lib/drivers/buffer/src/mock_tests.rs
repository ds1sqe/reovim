use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{BufferId, BufferManager},
    reovim_provider_text::Buffer,
};

use super::*;

fn register_buffer(mgr: &TestBufferManager, content: &str) -> BufferId {
    let buf = Buffer::from_string(content);
    mgr.register(Arc::new(RwLock::new(buf)))
}

fn register_empty(mgr: &TestBufferManager) -> BufferId {
    let buf = Buffer::new();
    mgr.register(Arc::new(RwLock::new(buf)))
}

/// Read all bytes from a `dyn KernelBuffer` as a UTF-8 string.
fn read_content(buf: &dyn reovim_kernel::api::v1::KernelBuffer) -> String {
    let len = buf.byte_len();
    let mut bytes = vec![0u8; len];
    buf.read_bytes(0, &mut bytes);
    String::from_utf8(bytes).expect("test buffer should be UTF-8")
}

#[test]
fn test_register_and_get() {
    let mgr = TestBufferManager::new();
    let id = register_empty(&mgr);
    assert!(mgr.get(id).is_some());
    assert_eq!(mgr.count(), 1);
}

#[test]
fn test_register_with_content() {
    let mgr = TestBufferManager::new();
    let id = register_buffer(&mgr, "test content");
    let arc = mgr.get(id).unwrap();
    assert_eq!(read_content(&*arc.read()), "test content");
}

#[test]
fn test_unregister() {
    let mgr = TestBufferManager::new();
    let id = register_empty(&mgr);
    assert!(mgr.unregister(id).is_some());
    assert!(mgr.get(id).is_none());
}

#[test]
fn test_list() {
    let mgr = TestBufferManager::new();
    let id1 = register_empty(&mgr);
    let id2 = register_empty(&mgr);
    let list = mgr.list();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&id1));
    assert!(list.contains(&id2));
}

#[test]
fn test_default() {
    let mgr = TestBufferManager::default();
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_unregister_not_found() {
    let mgr = TestBufferManager::new();
    let fake_id = BufferId::from_raw(9999);
    assert!(mgr.unregister(fake_id).is_none());
}

#[test]
fn test_get_nonexistent() {
    let mgr = TestBufferManager::new();
    let fake_id = BufferId::from_raw(9999);
    assert!(mgr.get(fake_id).is_none());
}

#[test]
fn test_unregister_with_shared_reference() {
    let mgr = TestBufferManager::new();
    let id = register_empty(&mgr);
    let _extra = mgr.get(id).unwrap();
    let result = mgr.unregister(id);
    assert!(result.is_some());
}
