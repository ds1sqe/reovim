use std::sync::Arc;

use {
    reovim_arch::sync::RwLock, reovim_kernel::api::v1::ServiceRegistry,
    reovim_provider_text::Buffer,
};

use super::TextBufferRegistry;

#[test]
fn register_and_get() {
    let registry = TextBufferRegistry::new();
    let buf = Arc::new(RwLock::new(Buffer::from_string("hello")));
    let id = registry.register(buf);

    let retrieved = registry.get(id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().read().content(), "hello");
}

#[test]
fn get_missing_returns_none() {
    let registry = TextBufferRegistry::new();
    let buf = Arc::new(RwLock::new(Buffer::from_string("")));
    let id = buf.read().id();
    assert!(registry.get(id).is_none());
}

#[test]
fn unregister_returns_buffer() {
    let registry = TextBufferRegistry::new();
    let buf = Arc::new(RwLock::new(Buffer::from_string("world")));
    let id = registry.register(buf);

    let removed = registry.unregister(id);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().read().content(), "world");
    assert!(registry.get(id).is_none());
}

#[test]
fn unregister_missing_returns_none() {
    let registry = TextBufferRegistry::new();
    let buf = Arc::new(RwLock::new(Buffer::from_string("")));
    let id = buf.read().id();
    assert!(registry.unregister(id).is_none());
}

#[test]
fn count_and_list() {
    let registry = TextBufferRegistry::new();
    assert_eq!(registry.count(), 0);
    assert!(registry.list().is_empty());

    let buf1 = Arc::new(RwLock::new(Buffer::from_string("a")));
    let buf2 = Arc::new(RwLock::new(Buffer::from_string("b")));
    let id1 = registry.register(buf1);
    let id2 = registry.register(buf2);

    assert_eq!(registry.count(), 2);
    let ids = registry.list();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn service_registry_integration() {
    let services = ServiceRegistry::new();
    services.register(Arc::new(TextBufferRegistry::new()));

    let reg = services.get::<TextBufferRegistry>();
    assert!(reg.is_some());

    let reg = reg.unwrap();
    let buf = Arc::new(RwLock::new(Buffer::from_string("via service")));
    let id = reg.register(buf);

    assert_eq!(reg.get(id).unwrap().read().content(), "via service");
}

#[test]
fn default_creates_empty() {
    let registry = TextBufferRegistry::default();
    assert_eq!(registry.count(), 0);
}
