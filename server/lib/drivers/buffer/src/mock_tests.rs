use super::*;

#[test]
fn test_create_and_get() {
    let mgr = TestBufferManager::new();
    let id = mgr.create();
    assert!(mgr.get(id).is_some());
    assert_eq!(mgr.count(), 1);
}

#[test]
fn test_register() {
    let mgr = TestBufferManager::new();
    let buffer = Buffer::from_string("test content");
    let id = mgr.register(buffer);
    assert!(mgr.get(id).is_some());
}

#[test]
fn test_unregister() {
    let mgr = TestBufferManager::new();
    let id = mgr.create();
    assert!(mgr.unregister(id).is_ok());
    assert!(mgr.get(id).is_none());
}

#[test]
fn test_list() {
    let mgr = TestBufferManager::new();
    let id1 = mgr.create();
    let id2 = mgr.create();
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
    assert!(mgr.unregister(fake_id).is_err());
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
    let id = mgr.create();
    // Hold an extra reference to trigger the Err(arc) => clone path
    let _extra = mgr.get(id).unwrap();
    let result = mgr.unregister(id);
    assert!(result.is_ok());
}
