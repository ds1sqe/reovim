use super::*;

use std::path::PathBuf;

use crate::api::context::StubBufferManager;

// ========== KernelContext tests ==========

#[test]
fn test_kernel_context_default() {
    let ctx = KernelContext::default();
    // StubBufferManager returns 0 count and empty list
    assert_eq!(ctx.buffers.count(), 0);
    assert!(ctx.buffers.list().is_empty());
}

#[test]
fn test_kernel_context_clone() {
    let ctx = KernelContext::default();
    let ctx2 = ctx.clone();
    // Both should share the same event bus (Arc)
    assert_eq!(ctx.buffers.count(), ctx2.buffers.count());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_kernel_context_debug() {
    let ctx = KernelContext::default();
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("KernelContext"));
    assert!(debug_str.contains("Arc<EventBus>"));
    assert!(debug_str.contains("Arc<dyn BufferManager>"));
    assert!(debug_str.contains("Arc<MotionEngine>"));
    assert!(debug_str.contains("Arc<TextObjectEngine>"));
    assert!(debug_str.contains("global_marks"));
    assert!(debug_str.contains("Arc<RwLock<MarkBank>>"));
    assert!(debug_str.contains("Arc<OptionRegistry>"));
    assert!(debug_str.contains("Arc<ServiceRegistry>"));
}

#[test]
fn test_kernel_context_new() {
    use {
        crate::core::{MarkBank, MotionEngine, TextObjectEngine},
        std::sync::Arc,
    };

    let event_bus = Arc::new(EventBus::new());
    let buffers: Arc<dyn BufferManager> = Arc::new(StubBufferManager);
    let motion = Arc::new(MotionEngine);
    let text_objects = Arc::new(TextObjectEngine);
    let global_marks = Arc::new(RwLock::new(MarkBank::new()));
    let options = Arc::new(OptionRegistry::new());
    let services = Arc::new(ServiceRegistry::new());

    let ctx = KernelContext::new(
        event_bus,
        buffers,
        motion,
        text_objects,
        global_marks,
        options,
        services,
    );

    assert_eq!(ctx.buffers.count(), 0);
}

#[test]
fn test_kernel_context_with_event_bus_and_services() {
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());

    let ctx = KernelContext::with_event_bus_and_services(event_bus, services);
    assert_eq!(ctx.buffers.count(), 0);
}

#[test]
fn test_kernel_context_with_event_bus_services_and_options() {
    use std::sync::Arc;

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());
    let options = Arc::new(OptionRegistry::new());

    let ctx = KernelContext::with_event_bus_services_and_options(event_bus, services, options);
    assert_eq!(ctx.buffers.count(), 0);
}

// ========== StubBufferManager tests ==========

#[test]
fn test_stub_buffer_manager_get() {
    let manager = StubBufferManager;
    let id = crate::mm::BufferId::from_raw(42);
    assert!(manager.get(id).is_none());
}

#[test]
fn test_stub_buffer_manager_register_returns_buffer_id() {
    use std::sync::Arc;

    let manager = StubBufferManager;
    let buf1 = crate::mm::Buffer::new();
    let buf2 = crate::mm::Buffer::new();
    let id1 = manager.register(Arc::new(RwLock::new(buf1)));
    let id2 = manager.register(Arc::new(RwLock::new(buf2)));
    // Each call returns the buffer's own ID
    assert_ne!(id1, id2);
}

#[test]
fn test_stub_buffer_manager_register() {
    use std::sync::Arc;

    let manager = StubBufferManager;
    let buffer = crate::mm::Buffer::new();
    let _id = manager.register(Arc::new(RwLock::new(buffer)));
    // register returns the buffer's ID but doesn't store anything
    assert_eq!(manager.count(), 0);
}

#[test]
fn test_stub_buffer_manager_unregister() {
    let manager = StubBufferManager;
    let id = crate::mm::BufferId::from_raw(42);
    let result = manager.unregister(id);
    // Stub always returns None (nothing stored)
    assert!(result.is_none());
}

#[test]
fn test_stub_buffer_manager_list() {
    let manager = StubBufferManager;
    assert!(manager.list().is_empty());
}

#[test]
fn test_stub_buffer_manager_count() {
    let manager = StubBufferManager;
    assert_eq!(manager.count(), 0);
}

// ========== ModuleContext tests ==========

#[test]
fn test_module_context_default() {
    let ctx = ModuleContext::default();
    assert_eq!(ctx.data_dir, PathBuf::from("/tmp/reovim-test/data"));
    assert_eq!(ctx.cache_dir, PathBuf::from("/tmp/reovim-test/cache"));
    assert!(ctx.optional_deps().is_empty());
}

#[test]
fn test_module_context_new() {
    use std::sync::Arc;

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let data_dir = PathBuf::from("/test/data");
    let cache_dir = PathBuf::from("/test/cache");

    let ctx = ModuleContext::new(kernel, services, data_dir.clone(), cache_dir.clone());
    assert_eq!(ctx.data_dir, data_dir);
    assert_eq!(ctx.cache_dir, cache_dir);
    assert!(ctx.optional_deps().is_empty());
}

#[test]
fn test_module_context_with_optional_deps() {
    use std::sync::Arc;

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let deps = vec![ModuleId::new("lsp"), ModuleId::new("treesitter")];

    let ctx = ModuleContext::with_optional_deps(
        kernel,
        services,
        PathBuf::from("/test/data"),
        PathBuf::from("/test/cache"),
        deps,
    );

    assert_eq!(ctx.optional_deps().len(), 2);
    assert!(ctx.has_optional_dep(&ModuleId::new("lsp")));
    assert!(ctx.has_optional_dep(&ModuleId::new("treesitter")));
    assert!(!ctx.has_optional_dep(&ModuleId::new("nonexistent")));
}

#[test]
fn test_module_context_has_optional_dep_empty() {
    let ctx = ModuleContext::default();
    assert!(!ctx.has_optional_dep(&ModuleId::new("anything")));
}

#[test]
fn test_module_context_clone() {
    let ctx = ModuleContext::default();
    let ctx2 = ctx.clone();
    assert_eq!(ctx.data_dir, ctx2.data_dir);
    assert_eq!(ctx.cache_dir, ctx2.cache_dir);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context_debug() {
    let ctx = ModuleContext::default();
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("ModuleContext"));
    assert!(debug_str.contains("kernel"));
    assert!(debug_str.contains("services"));
    assert!(debug_str.contains("data_dir"));
    assert!(debug_str.contains("cache_dir"));
    assert!(debug_str.contains("loaded_optional_deps"));
}
