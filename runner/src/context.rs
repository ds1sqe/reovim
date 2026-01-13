//! Kernel context builder and utilities.
//!
//! Provides builder pattern for constructing `KernelContext` and `ModuleContext`
//! with sensible defaults.

use {
    crate::buffer_manager::SimpleBufferManager,
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{
        BufferManager, EventBus, KernelContext, MarkBank, ModuleContext, ModuleId, MotionEngine,
        RegisterBank, TextObjectEngine,
    },
    std::{path::Path, sync::Arc},
};

/// Builder for `KernelContext`.
///
/// Allows incremental construction with sensible defaults.
/// All unset fields will be initialized with default implementations.
///
/// # Example
///
/// ```ignore
/// use runner::context::KernelContextBuilder;
///
/// let ctx = KernelContextBuilder::new()
///     .buffers(Arc::new(SimpleBufferManager::new()))
///     .build();
/// ```
pub struct KernelContextBuilder {
    event_bus: Option<Arc<EventBus>>,
    buffers: Option<Arc<dyn BufferManager>>,
    motion: Option<Arc<MotionEngine>>,
    text_objects: Option<Arc<TextObjectEngine>>,
    registers: Option<Arc<RwLock<RegisterBank>>>,
    marks: Option<Arc<RwLock<MarkBank>>>,
}

impl KernelContextBuilder {
    /// Create a new builder with no fields set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_bus: None,
            buffers: None,
            motion: None,
            text_objects: None,
            registers: None,
            marks: None,
        }
    }

    /// Set the event bus.
    #[must_use]
    pub fn event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Set the buffer manager.
    #[must_use]
    pub fn buffers(mut self, mgr: Arc<dyn BufferManager>) -> Self {
        self.buffers = Some(mgr);
        self
    }

    /// Set the motion engine.
    #[must_use]
    pub fn motion(mut self, engine: Arc<MotionEngine>) -> Self {
        self.motion = Some(engine);
        self
    }

    /// Set the text object engine.
    #[must_use]
    pub fn text_objects(mut self, engine: Arc<TextObjectEngine>) -> Self {
        self.text_objects = Some(engine);
        self
    }

    /// Set the register bank.
    #[must_use]
    pub fn registers(mut self, bank: Arc<RwLock<RegisterBank>>) -> Self {
        self.registers = Some(bank);
        self
    }

    /// Set the mark bank.
    #[must_use]
    pub fn marks(mut self, bank: Arc<RwLock<MarkBank>>) -> Self {
        self.marks = Some(bank);
        self
    }

    /// Build `KernelContext` with defaults for any unset fields.
    #[must_use]
    pub fn build(self) -> KernelContext {
        KernelContext::new(
            self.event_bus.unwrap_or_else(|| Arc::new(EventBus::new())),
            self.buffers
                .unwrap_or_else(|| Arc::new(SimpleBufferManager::new())),
            self.motion.unwrap_or_else(|| Arc::new(MotionEngine)),
            self.text_objects
                .unwrap_or_else(|| Arc::new(TextObjectEngine)),
            self.registers
                .unwrap_or_else(|| Arc::new(RwLock::new(RegisterBank::new()))),
            self.marks
                .unwrap_or_else(|| Arc::new(RwLock::new(MarkBank::new()))),
        )
    }
}

impl Default for KernelContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Build default `KernelContext` with `SimpleBufferManager`.
///
/// This is the standard entry point for creating a kernel context
/// in the runner application.
///
/// # Example
///
/// ```ignore
/// use runner::context::build_default_kernel_context;
///
/// let ctx = build_default_kernel_context();
/// let id = ctx.buffers.create();
/// ```
#[must_use]
pub fn build_default_kernel_context() -> KernelContext {
    KernelContextBuilder::new()
        .buffers(Arc::new(SimpleBufferManager::new()))
        .build()
}

/// Build `ModuleContext` for a specific module.
///
/// Creates per-module data and cache directories under the given base paths.
/// Directories are created if they don't exist.
///
/// # Arguments
///
/// * `kernel` - Kernel context for core services
/// * `module_id` - Module identifier (used for directory naming)
/// * `data_base` - Base data directory (e.g., `~/.local/share/reovim/`)
/// * `cache_base` - Base cache directory (e.g., `~/.cache/reovim/`)
///
/// # Example
///
/// ```ignore
/// use runner::context::{build_default_kernel_context, build_module_context};
/// use std::path::Path;
///
/// let kernel = build_default_kernel_context();
/// let data = Path::new("/home/user/.local/share/reovim");
/// let cache = Path::new("/home/user/.cache/reovim");
///
/// let ctx = build_module_context(kernel, "my-module", data, cache);
/// assert!(ctx.data_dir.exists());
/// ```
pub fn build_module_context(
    kernel: KernelContext,
    module_id: &str,
    data_base: &Path,
    cache_base: &Path,
) -> ModuleContext {
    let data_dir = data_base.join("modules").join(module_id);
    let cache_dir = cache_base.join("modules").join(module_id);

    // Ensure directories exist (ignore errors - modules handle missing dirs)
    std::fs::create_dir_all(&data_dir).ok();
    std::fs::create_dir_all(&cache_dir).ok();

    ModuleContext::new(kernel, data_dir, cache_dir)
}

/// Build `ModuleContext` with optional dependencies info.
///
/// Like `build_module_context`, but also includes information about
/// which optional dependencies were successfully loaded.
///
/// # Arguments
///
/// * `kernel` - Kernel context for core services
/// * `module_id` - Module identifier
/// * `data_base` - Base data directory
/// * `cache_base` - Base cache directory
/// * `loaded_optional_deps` - List of optional deps that were loaded
pub fn build_module_context_with_deps(
    kernel: KernelContext,
    module_id: &str,
    data_base: &Path,
    cache_base: &Path,
    loaded_optional_deps: Vec<ModuleId>,
) -> ModuleContext {
    let data_dir = data_base.join("modules").join(module_id);
    let cache_dir = cache_base.join("modules").join(module_id);

    std::fs::create_dir_all(&data_dir).ok();
    std::fs::create_dir_all(&cache_dir).ok();

    ModuleContext::with_optional_deps(kernel, data_dir, cache_dir, loaded_optional_deps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let ctx = KernelContextBuilder::new().build();
        // Should be able to create buffer
        let id = ctx.buffers.create();
        assert!(ctx.buffers.get(id).is_some());
    }

    #[test]
    fn test_builder_with_custom_buffer_manager() {
        let mgr: Arc<dyn BufferManager> = Arc::new(SimpleBufferManager::new());
        let ctx = KernelContextBuilder::new().buffers(mgr).build();

        // Create via context, verify via original manager
        let id = ctx.buffers.create();
        assert_eq!(ctx.buffers.count(), 1);
        assert!(ctx.buffers.get(id).is_some());
    }

    #[test]
    fn test_build_default_kernel_context() {
        let ctx = build_default_kernel_context();
        assert_eq!(ctx.buffers.count(), 0);

        let id = ctx.buffers.create();
        assert_eq!(ctx.buffers.count(), 1);
        assert!(ctx.buffers.get(id).is_some());
    }

    #[test]
    fn test_build_module_context() {
        let kernel = build_default_kernel_context();
        let data_base = std::env::temp_dir().join("reovim_test_ctx_data");
        let cache_base = std::env::temp_dir().join("reovim_test_ctx_cache");

        let ctx = build_module_context(kernel, "test-module", &data_base, &cache_base);

        // Verify directories were created
        assert!(ctx.data_dir.exists());
        assert!(ctx.cache_dir.exists());
        assert!(ctx.data_dir.ends_with("modules/test-module"));
        assert!(ctx.cache_dir.ends_with("modules/test-module"));

        // Cleanup
        std::fs::remove_dir_all(&data_base).ok();
        std::fs::remove_dir_all(&cache_base).ok();
    }

    #[test]
    fn test_build_module_context_with_deps() {
        let kernel = build_default_kernel_context();
        let data_base = std::env::temp_dir().join("reovim_test_ctx_deps_data");
        let cache_base = std::env::temp_dir().join("reovim_test_ctx_deps_cache");

        let deps = vec![ModuleId::new("lsp"), ModuleId::new("syntax")];
        let ctx =
            build_module_context_with_deps(kernel, "test-module", &data_base, &cache_base, deps);

        assert!(ctx.has_optional_dep(&ModuleId::new("lsp")));
        assert!(ctx.has_optional_dep(&ModuleId::new("syntax")));
        assert!(!ctx.has_optional_dep(&ModuleId::new("unknown")));
        assert_eq!(ctx.optional_deps().len(), 2);

        // Cleanup
        std::fs::remove_dir_all(&data_base).ok();
        std::fs::remove_dir_all(&cache_base).ok();
    }
}
