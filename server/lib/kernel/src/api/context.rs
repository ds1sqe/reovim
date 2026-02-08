//! Kernel context for drivers and modules.
//!
//! Provides a unified context struct bundling kernel services for easy access.

use std::{fmt, path::PathBuf, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::{
    core::{MarkBank, MotionEngine, OptionRegistry, RegisterBank, TextObjectEngine},
    ipc::EventBus,
};

use super::{buffer_manager::BufferManager, module::ModuleId, service::ServiceRegistry};

// ============================================================================
// KernelContext
// ============================================================================

/// Kernel context bundling all kernel services.
///
/// This struct provides drivers and modules with access to kernel services.
/// All fields are `Arc`-wrapped for cheap cloning and safe concurrent access.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::KernelContext;
///
/// fn setup_driver(ctx: KernelContext) {
///     // Access event bus
///     let _bus = ctx.event_bus.clone();
///
///     // Access buffer manager
///     let count = ctx.buffers.count();
///
///     // Context is cheap to clone
///     let ctx2 = ctx.clone();
/// }
/// ```
#[derive(Clone)]
pub struct KernelContext {
    /// Event bus for publish/subscribe communication.
    pub event_bus: Arc<EventBus>,
    /// Buffer manager for buffer storage and retrieval.
    pub buffers: Arc<dyn BufferManager>,
    /// Motion calculation engine.
    pub motion: Arc<MotionEngine>,
    /// Text object calculation engine.
    pub text_objects: Arc<TextObjectEngine>,
    /// Register storage for yank/paste operations.
    pub registers: Arc<RwLock<RegisterBank>>,
    /// Mark storage for bookmark operations.
    pub marks: Arc<RwLock<MarkBank>>,
    /// Option registry for editor settings.
    pub options: Arc<OptionRegistry>,
    /// Service registry for cross-module service discovery.
    ///
    /// Provides access to driver and module services like `ClipboardProvider`.
    /// This is the same registry used in `ModuleContext.services`.
    pub services: Arc<ServiceRegistry>,
}

impl KernelContext {
    /// Create a new kernel context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_bus: Arc<EventBus>,
        buffers: Arc<dyn BufferManager>,
        motion: Arc<MotionEngine>,
        text_objects: Arc<TextObjectEngine>,
        registers: Arc<RwLock<RegisterBank>>,
        marks: Arc<RwLock<MarkBank>>,
        options: Arc<OptionRegistry>,
        services: Arc<ServiceRegistry>,
    ) -> Self {
        Self {
            event_bus,
            buffers,
            motion,
            text_objects,
            registers,
            marks,
            options,
            services,
        }
    }
}

impl fmt::Debug for KernelContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KernelContext")
            .field("event_bus", &"Arc<EventBus>")
            .field("buffers", &"Arc<dyn BufferManager>")
            .field("motion", &"Arc<MotionEngine>")
            .field("text_objects", &"Arc<TextObjectEngine>")
            .field("registers", &"Arc<RwLock<RegisterBank>>")
            .field("marks", &"Arc<RwLock<MarkBank>>")
            .field("options", &"Arc<OptionRegistry>")
            .field("services", &"Arc<ServiceRegistry>")
            .finish()
    }
}

// ============================================================================
// ModuleContext
// ============================================================================

/// Context provided to modules during initialization.
///
/// Extends `KernelContext` with module-specific paths for data and cache storage,
/// plus access to the cross-module service registry.
/// This is passed to `Module::init()` and provides everything a module needs
/// to initialize itself.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult};
///
/// impl Module for MyModule {
///     fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
///         // Access kernel services
///         let bus = &ctx.kernel.event_bus;
///
///         // Access module-specific directories
///         let config_path = ctx.data_dir.join("config.toml");
///         let cache_path = ctx.cache_dir.join("cache.bin");
///
///         // Register services for other modules to discover
///         ctx.services.register(Arc::new(MyService::new()));
///
///         ProbeResult::Success
///     }
/// }
/// ```
#[derive(Clone)]
pub struct ModuleContext {
    /// Kernel context for core services.
    pub kernel: KernelContext,
    /// Service registry for cross-module service discovery.
    ///
    /// Modules can register their services here during `init()` for other
    /// modules to discover. See [`ServiceRegistry`] for usage patterns.
    pub services: Arc<ServiceRegistry>,
    /// Module's data directory (persistent storage).
    ///
    /// e.g., `~/.local/share/reovim/modules/<module-id>/`
    pub data_dir: PathBuf,
    /// Module's cache directory (ephemeral storage).
    ///
    /// e.g., `~/.cache/reovim/modules/<module-id>/`
    pub cache_dir: PathBuf,
    /// Optional dependencies that were successfully loaded (Phase 4.6 addition).
    ///
    /// Populated by the module registry before calling `init()`.
    /// Use `has_optional_dep()` or `optional_deps()` to query.
    loaded_optional_deps: Vec<ModuleId>,
}

impl ModuleContext {
    /// Create a new module context.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Arc::new is not const
    pub fn new(
        kernel: KernelContext,
        services: Arc<ServiceRegistry>,
        data_dir: PathBuf,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            kernel,
            services,
            data_dir,
            cache_dir,
            loaded_optional_deps: Vec::new(),
        }
    }

    /// Create a module context with optional dependencies info.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Arc::new is not const
    pub fn with_optional_deps(
        kernel: KernelContext,
        services: Arc<ServiceRegistry>,
        data_dir: PathBuf,
        cache_dir: PathBuf,
        loaded_optional_deps: Vec<ModuleId>,
    ) -> Self {
        Self {
            kernel,
            services,
            data_dir,
            cache_dir,
            loaded_optional_deps,
        }
    }

    /// Check if an optional dependency was loaded.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_kernel::api::v1::{ModuleContext, ModuleId};
    ///
    /// fn init(ctx: &ModuleContext) {
    ///     if ctx.has_optional_dep(&ModuleId::new("lsp")) {
    ///         // Enable LSP integration
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn has_optional_dep(&self, id: &ModuleId) -> bool {
        self.loaded_optional_deps.iter().any(|dep| dep == id)
    }

    /// Get all loaded optional dependencies.
    ///
    /// Returns a slice of `ModuleId`s for optional dependencies that were
    /// successfully loaded before this module's initialization.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_kernel::api::v1::ModuleContext;
    ///
    /// fn init(ctx: &ModuleContext) {
    ///     for dep in ctx.optional_deps() {
    ///         println!("Optional dep available: {}", dep.as_str());
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn optional_deps(&self) -> &[ModuleId] {
        &self.loaded_optional_deps
    }
}

impl fmt::Debug for ModuleContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleContext")
            .field("kernel", &self.kernel)
            .field("services", &self.services)
            .field("data_dir", &self.data_dir)
            .field("cache_dir", &self.cache_dir)
            .field("loaded_optional_deps", &self.loaded_optional_deps)
            .finish()
    }
}

impl Default for ModuleContext {
    /// Create a default `ModuleContext` for testing purposes.
    ///
    /// Uses stub implementations and temporary directories.
    /// Should only be used in tests where context fields are not accessed.
    fn default() -> Self {
        Self {
            kernel: KernelContext::default(),
            services: Arc::new(ServiceRegistry::new()),
            data_dir: PathBuf::from("/tmp/reovim-test/data"),
            cache_dir: PathBuf::from("/tmp/reovim-test/cache"),
            loaded_optional_deps: Vec::new(),
        }
    }
}

impl KernelContext {
    /// Create a `KernelContext` with a shared event bus and services.
    ///
    /// Uses stub implementations for other fields. Useful for module
    /// initialization where modules need to subscribe to events but
    /// don't need buffer management yet (#440).
    ///
    /// # Arguments
    ///
    /// * `event_bus` - Shared event bus for subscriptions
    /// * `services` - Shared service registry
    #[must_use]
    pub fn with_event_bus_and_services(
        event_bus: Arc<EventBus>,
        services: Arc<ServiceRegistry>,
    ) -> Self {
        Self::with_event_bus_services_and_options(
            event_bus,
            services,
            Arc::new(OptionRegistry::new()),
        )
    }

    /// Create a `KernelContext` with shared event bus, services, and options.
    ///
    /// This variant allows sharing the option registry between module init
    /// and the final session context (#458). Modules can register options
    /// during `init()` and they will be available in the session.
    ///
    /// # Arguments
    ///
    /// * `event_bus` - Shared event bus for subscriptions
    /// * `services` - Shared service registry
    /// * `options` - Shared option registry
    #[must_use]
    pub fn with_event_bus_services_and_options(
        event_bus: Arc<EventBus>,
        services: Arc<ServiceRegistry>,
        options: Arc<OptionRegistry>,
    ) -> Self {
        use crate::core::{MarkBank, MotionEngine, RegisterBank, TextObjectEngine};

        Self {
            event_bus,
            buffers: Arc::new(StubBufferManager),
            motion: Arc::new(MotionEngine),
            text_objects: Arc::new(TextObjectEngine),
            registers: Arc::new(RwLock::new(RegisterBank::new())),
            marks: Arc::new(RwLock::new(MarkBank::new())),
            options,
            services,
        }
    }
}

impl Default for KernelContext {
    /// Create a default `KernelContext` for testing purposes.
    ///
    /// Uses stub implementations. Should only be used in tests.
    fn default() -> Self {
        use crate::core::{MarkBank, MotionEngine, RegisterBank, TextObjectEngine};

        Self {
            event_bus: Arc::new(crate::ipc::EventBus::new()),
            buffers: Arc::new(StubBufferManager),
            motion: Arc::new(MotionEngine),
            text_objects: Arc::new(TextObjectEngine),
            registers: Arc::new(RwLock::new(RegisterBank::new())),
            marks: Arc::new(RwLock::new(MarkBank::new())),
            options: Arc::new(OptionRegistry::new()),
            services: Arc::new(ServiceRegistry::new()),
        }
    }
}

/// Stub buffer manager for testing.
///
/// This implementation does nothing and returns empty/None for all operations.
/// Used only for tests where the buffer manager is not actually accessed.
struct StubBufferManager;

impl BufferManager for StubBufferManager {
    fn get(
        &self,
        _id: crate::mm::BufferId,
    ) -> Option<Arc<reovim_arch::sync::RwLock<crate::mm::Buffer>>> {
        None
    }

    fn create(&self) -> crate::mm::BufferId {
        crate::mm::BufferId::new()
    }

    fn register(&self, _buffer: crate::mm::Buffer) -> crate::mm::BufferId {
        crate::mm::BufferId::new()
    }

    fn unregister(
        &self,
        id: crate::mm::BufferId,
    ) -> Result<crate::mm::Buffer, super::buffer_manager::BufferError> {
        Err(super::buffer_manager::BufferError::NotFound(id))
    }

    fn list(&self) -> Vec<crate::mm::BufferId> {
        Vec::new()
    }

    fn count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_kernel_context_debug() {
        let ctx = KernelContext::default();
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("KernelContext"));
        assert!(debug_str.contains("Arc<EventBus>"));
        assert!(debug_str.contains("Arc<dyn BufferManager>"));
        assert!(debug_str.contains("Arc<MotionEngine>"));
        assert!(debug_str.contains("Arc<TextObjectEngine>"));
        assert!(debug_str.contains("Arc<RwLock<RegisterBank>>"));
        assert!(debug_str.contains("Arc<RwLock<MarkBank>>"));
        assert!(debug_str.contains("Arc<OptionRegistry>"));
        assert!(debug_str.contains("Arc<ServiceRegistry>"));
    }

    #[test]
    fn test_kernel_context_new() {
        use crate::core::{MarkBank, MotionEngine, RegisterBank, TextObjectEngine};

        let event_bus = Arc::new(EventBus::new());
        let buffers: Arc<dyn BufferManager> = Arc::new(StubBufferManager);
        let motion = Arc::new(MotionEngine);
        let text_objects = Arc::new(TextObjectEngine);
        let registers = Arc::new(RwLock::new(RegisterBank::new()));
        let marks = Arc::new(RwLock::new(MarkBank::new()));
        let options = Arc::new(OptionRegistry::new());
        let services = Arc::new(ServiceRegistry::new());

        let ctx = KernelContext::new(
            event_bus,
            buffers,
            motion,
            text_objects,
            registers,
            marks,
            options,
            services,
        );

        assert_eq!(ctx.buffers.count(), 0);
    }

    #[test]
    fn test_kernel_context_with_event_bus_and_services() {
        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());

        let ctx = KernelContext::with_event_bus_and_services(event_bus, services);
        assert_eq!(ctx.buffers.count(), 0);
    }

    #[test]
    fn test_kernel_context_with_event_bus_services_and_options() {
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
    fn test_stub_buffer_manager_create() {
        let manager = StubBufferManager;
        let id1 = manager.create();
        let id2 = manager.create();
        // Each call creates a new unique BufferId
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_stub_buffer_manager_register() {
        let manager = StubBufferManager;
        let buffer = crate::mm::Buffer::new();
        let _id = manager.register(buffer);
        // register returns a new BufferId but doesn't store anything
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_stub_buffer_manager_unregister() {
        let manager = StubBufferManager;
        let id = crate::mm::BufferId::from_raw(42);
        let result = manager.unregister(id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), super::super::buffer_manager::BufferError::NotFound(id));
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
}
