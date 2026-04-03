//! Kernel context for drivers and modules.
//!
//! Provides a unified context struct bundling kernel services for easy access.

use std::{fmt, path::PathBuf, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::{
    core::{MarkBank, OptionRegistry},
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
    /// Global mark storage (A-Z, shared special marks) (#515).
    ///
    /// Per-client local marks (a-z) are stored in `EditingState.local_marks`.
    pub global_marks: Arc<RwLock<MarkBank>>,
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
        global_marks: Arc<RwLock<MarkBank>>,
        options: Arc<OptionRegistry>,
        services: Arc<ServiceRegistry>,
    ) -> Self {
        Self {
            event_bus,
            buffers,
            global_marks,
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
            .field("global_marks", &"Arc<RwLock<MarkBank>>")
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
        use crate::core::MarkBank;

        Self {
            event_bus,
            buffers: Arc::new(StubBufferManager),
            global_marks: Arc::new(RwLock::new(MarkBank::new())),
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
        use crate::core::MarkBank;

        Self {
            event_bus: Arc::new(crate::ipc::EventBus::new()),
            buffers: Arc::new(StubBufferManager),
            global_marks: Arc::new(RwLock::new(MarkBank::new())),
            options: Arc::new(OptionRegistry::new()),
            services: Arc::new(ServiceRegistry::new()),
        }
    }
}

/// Stub buffer manager for testing.
///
/// This implementation does nothing and returns empty/None for all operations.
/// Used only for tests where the buffer manager is not actually accessed.
pub struct StubBufferManager;

impl BufferManager for StubBufferManager {
    fn get(&self, _id: crate::mm::BufferId) -> Option<Arc<RwLock<dyn crate::api::BufferOps>>> {
        None
    }

    fn register(&self, buffer: Arc<RwLock<dyn crate::api::BufferOps>>) -> crate::mm::BufferId {
        buffer.read().id()
    }

    fn unregister(
        &self,
        _id: crate::mm::BufferId,
    ) -> Option<Arc<RwLock<dyn crate::api::BufferOps>>> {
        None
    }

    fn list(&self) -> Vec<crate::mm::BufferId> {
        Vec::new()
    }

    fn count(&self) -> usize {
        0
    }
}
