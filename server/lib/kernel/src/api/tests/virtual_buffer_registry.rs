//! Tests for `VirtualBufferRegistry` and `SimpleVirtualBufferRegistry`.

use std::sync::Arc;

use crate::api::v1::*;

fn make_vbuf(content: &str) -> VirtualBuffer {
    let mapping = Arc::new(HeapMapping(content.as_bytes().to_vec()));
    let line_index = LineIndex::from_bytes(content.as_bytes()).unwrap();
    VirtualBuffer::new(mapping, line_index)
}

#[test]
fn register_and_get() {
    let registry = SimpleVirtualBufferRegistry::new();
    let vbuf = make_vbuf("hello\nworld");
    let id = vbuf.id();

    let registered_id = registry.register(vbuf);
    assert_eq!(registered_id, id);

    let retrieved = registry.get(id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().read().content(), "hello\nworld");
}

#[test]
fn get_nonexistent() {
    let registry = SimpleVirtualBufferRegistry::new();
    let id = BufferId::from_raw(999);
    assert!(registry.get(id).is_none());
}

#[test]
fn is_virtual() {
    let registry = SimpleVirtualBufferRegistry::new();
    let vbuf = make_vbuf("test");
    let id = vbuf.id();

    assert!(!registry.is_virtual(id));
    registry.register(vbuf);
    assert!(registry.is_virtual(id));
}

#[test]
fn unregister() {
    let registry = SimpleVirtualBufferRegistry::new();
    let vbuf = make_vbuf("test");
    let id = vbuf.id();

    registry.register(vbuf);
    assert!(registry.is_virtual(id));

    let unregistered = registry.unregister(id);
    assert!(unregistered.is_some());
    assert_eq!(unregistered.unwrap().content(), "test");
    assert!(!registry.is_virtual(id));
}

#[test]
fn unregister_nonexistent() {
    let registry = SimpleVirtualBufferRegistry::new();
    let id = BufferId::from_raw(999);
    assert!(registry.unregister(id).is_none());
}

#[test]
fn list() {
    let registry = SimpleVirtualBufferRegistry::new();
    let vbuf1 = make_vbuf("a");
    let vbuf2 = make_vbuf("b");
    let id1 = vbuf1.id();
    let id2 = vbuf2.id();

    registry.register(vbuf1);
    registry.register(vbuf2);

    let mut ids = registry.list();
    ids.sort();
    let mut expected = vec![id1, id2];
    expected.sort();
    assert_eq!(ids, expected);
}

#[test]
fn count() {
    let registry = SimpleVirtualBufferRegistry::new();
    assert_eq!(registry.count(), 0);

    registry.register(make_vbuf("a"));
    assert_eq!(registry.count(), 1);

    registry.register(make_vbuf("b"));
    assert_eq!(registry.count(), 2);
}

#[test]
fn default() {
    let registry = SimpleVirtualBufferRegistry::default();
    assert_eq!(registry.count(), 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_formatting() {
    let registry = SimpleVirtualBufferRegistry::new();
    let debug = format!("{registry:?}");
    assert!(debug.contains("SimpleVirtualBufferRegistry"));
    assert!(debug.contains("count"));
}

#[test]
fn service_trait() {
    // Verify SimpleVirtualBufferRegistry implements Service
    let registry = Arc::new(SimpleVirtualBufferRegistry::new());
    let _service: Arc<dyn Service> = registry;
}
