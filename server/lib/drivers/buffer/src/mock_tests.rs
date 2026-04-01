use std::sync::Arc;

use reovim_arch::sync::RwLock;
use reovim_kernel::api::v1::{Buffer, BufferId, BufferManager, BufferOps};

use super::*;

fn register_buffer(mgr: &TestBufferManager, content: &str) -> BufferId {
    let buf = Buffer::from_string(content);
    let arc: Arc<RwLock<dyn BufferOps>> = Arc::new(RwLock::new(buf));
    mgr.register(arc)
}

fn register_empty(mgr: &TestBufferManager) -> BufferId {
    let buf = Buffer::new();
    let arc: Arc<RwLock<dyn BufferOps>> = Arc::new(RwLock::new(buf));
    mgr.register(arc)
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
    assert_eq!(arc.read().content(), "test content");
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
