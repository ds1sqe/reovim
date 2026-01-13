//! Kernel context for drivers and modules.
//!
//! Provides a unified context struct bundling kernel services for easy access.

use std::{fmt, path::PathBuf, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::{
    core::{MarkBank, MotionEngine, OptionRegistry, RegisterBank, TextObjectEngine},
    ipc::EventBus,
};

use super::{buffer_manager::BufferManager, module::ModuleId};

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
}

impl KernelContext {
    /// Create a new kernel context.
    pub fn new(
        event_bus: Arc<EventBus>,
        buffers: Arc<dyn BufferManager>,
        motion: Arc<MotionEngine>,
        text_objects: Arc<TextObjectEngine>,
        registers: Arc<RwLock<RegisterBank>>,
        marks: Arc<RwLock<MarkBank>>,
        options: Arc<OptionRegistry>,
    ) -> Self {
        Self {
            event_bus,
            buffers,
            motion,
            text_objects,
            registers,
            marks,
            options,
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
            .finish()
    }
}

// ============================================================================
// ModuleContext
// ============================================================================

/// Context provided to modules during initialization.
///
/// Extends `KernelContext` with module-specific paths for data and cache storage.
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
///         ProbeResult::Success
///     }
/// }
/// ```
#[derive(Clone)]
pub struct ModuleContext {
    /// Kernel context for core services.
    pub kernel: KernelContext,
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
    pub const fn new(kernel: KernelContext, data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            kernel,
            data_dir,
            cache_dir,
            loaded_optional_deps: Vec::new(),
        }
    }

    /// Create a module context with optional dependencies info.
    #[must_use]
    pub const fn with_optional_deps(
        kernel: KernelContext,
        data_dir: PathBuf,
        cache_dir: PathBuf,
        loaded_optional_deps: Vec<ModuleId>,
    ) -> Self {
        Self {
            kernel,
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
            data_dir: PathBuf::from("/tmp/reovim-test/data"),
            cache_dir: PathBuf::from("/tmp/reovim-test/cache"),
            loaded_optional_deps: Vec::new(),
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
