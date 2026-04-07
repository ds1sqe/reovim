use std::sync::Arc;

use reovim_kernel::api::v1::{BufferId, ServiceRegistry};

use {super::BufferReadAccess, crate::TextBufferRegistry};

#[test]
fn test_buffer_read_access_registered_and_retrieved() {
    let registry = Arc::new(TextBufferRegistry::new());
    let services = ServiceRegistry::new();
    services.register(Arc::new(BufferReadAccess::new(Arc::clone(&registry))));

    let access = services.get::<BufferReadAccess>();
    assert!(access.is_some());
}

#[test]
fn test_buffer_read_access_reads_buffer() {
    let registry = Arc::new(TextBufferRegistry::new());
    let buf = reovim_provider_text::Buffer::from_string("hello\nworld");
    let bid = registry.register(Arc::new(reovim_arch::sync::RwLock::new(buf)));

    let access = BufferReadAccess::new(Arc::clone(&registry));
    let buffer_lock = access.get(bid);
    assert!(buffer_lock.is_some());

    let guard = buffer_lock.unwrap();
    let buffer = guard.read();
    let line0 = buffer.line(0).map(std::borrow::Cow::into_owned);
    let line1 = buffer.line(1).map(std::borrow::Cow::into_owned);
    drop(buffer);

    assert_eq!(line0.as_deref(), Some("hello"));
    assert_eq!(line1.as_deref(), Some("world"));
}

#[test]
fn test_buffer_read_access_missing_buffer() {
    let registry = Arc::new(TextBufferRegistry::new());
    let access = BufferReadAccess::new(registry);

    assert!(access.get(BufferId::from_raw(999)).is_none());
}
