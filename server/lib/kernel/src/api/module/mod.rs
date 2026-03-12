//! Module identity, traits, and registration types.
//!
//! Linux equivalent: `include/linux/module.h`
//!
//! This module provides the core module mechanism for the kernel. Modules are
//! loadable components that can extend the editor's functionality. The kernel
//! defines the `Module` trait (mechanism); the runner implements loading (policy).
//!
//! # Linux Kernel Patterns
//!
//! The design follows Linux kernel patterns:
//! - `Module` trait ≈ `struct module` + `module_init`/`module_exit`
//! - `ProbeResult` ≈ `probe()` return with `-EPROBE_DEFER` support
//! - `RegistrationFlags` ≈ module flags and capabilities
//! - Registration types ≈ `struct platform_driver`, `struct notifier_block`

mod error;
mod flags;
mod id;
mod probe;
mod registration;
mod state;

pub use {
    error::{ModuleError, ProbeResult},
    flags::RegistrationFlags,
    id::ModuleId,
    probe::ModuleProbe,
    registration::{CommandRegistration, EventHandlerRegistration, KeybindingRegistration},
    state::{ModuleInfo, ModuleState},
};

use super::{
    context::ModuleContext,
    version::{API_VERSION, Version},
};

// Import BufferId for on_buffer_focus hook
use crate::mm::BufferId;

/// Trait for all loadable modules.
///
/// Linux equivalent: `struct module` with `module_init`/`module_exit`.
///
/// Modules encapsulate functionality that can be loaded dynamically at runtime.
/// The kernel defines this trait (mechanism); the runner implements loading (policy).
///
/// # Lifecycle
///
/// 1. Module is loaded (by runner's module loader)
/// 2. `init()` is called with `ModuleContext`
///    - Returns `ProbeResult::Success` → module is ready
///    - Returns `ProbeResult::Defer` → retry later
///    - Returns `ProbeResult::Failed` → permanent failure
/// 3. Module is now `Running`
/// 4. `exit()` is called before unload
///
/// # Thread Safety
///
/// Modules must be `Send + Sync` as they may be accessed from multiple threads.
/// The threading model is:
///
/// - **Exclusive access methods** (`&mut self`): `init()` and `exit()` are guaranteed
///   to be called with exclusive access. The runner/kernel ensures no concurrent calls.
/// - **Shared access methods** (`&self`): `id()`, `name()`, `version()`, `commands()`,
///   `keybindings()`, `event_handlers()`, etc. may be called concurrently from multiple
///   threads. Implementations should return const data or use internal synchronization.
/// - **Hot reload methods**: `save_state()` (`&self`) and `restore_state()` (`&mut self`)
///   follow the same exclusive/shared access patterns.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// pub struct MyModule;
///
/// impl Module for MyModule {
///     fn id(&self) -> ModuleId { ModuleId::new("my-module") }
///     fn name(&self) -> &'static str { "My Module" }
///     fn version(&self) -> Version { Version::new(1, 0, 0) }
///
///     fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
///         pr_info!("MyModule initialized");
///         ProbeResult::Success
///     }
///
///     fn exit(&mut self) -> Result<(), ModuleError> {
///         pr_info!("MyModule exiting");
///         Ok(())
///     }
/// }
/// ```
pub trait Module: Send + Sync + 'static {
    // ========================================================================
    // Identity
    // ========================================================================

    /// Unique module identifier.
    ///
    /// Convention: kebab-case, e.g., "lang-rust", "feat-completion"
    fn id(&self) -> ModuleId;

    /// Human-readable name.
    fn name(&self) -> &'static str;

    /// Module version.
    fn version(&self) -> Version;

    /// Required kernel API version.
    ///
    /// Defaults to current API version. Override to require specific version.
    fn api_version(&self) -> Version {
        API_VERSION
    }

    // ========================================================================
    // Dependencies
    // ========================================================================

    /// Required dependencies (must be loaded before this module).
    ///
    /// The runner will ensure all required dependencies are loaded and initialized
    /// before calling `init()` on this module. If any dependency fails to load,
    /// this module will not be initialized.
    fn dependencies(&self) -> Vec<ModuleId> {
        Vec::new()
    }

    /// Optional dependencies (load before if available, but not required).
    ///
    /// # Semantics
    ///
    /// - Optional dependencies are loaded before this module **if they exist**
    /// - If an optional dependency fails to load, this module can still initialize
    /// - Unlike `dependencies()`, missing optional dependencies don't cause init failure
    /// - Useful for feature detection and graceful degradation
    ///
    /// # Use Cases
    ///
    /// - LSP enhancement: Load LSP plugin if available for better completions
    /// - Syntax integration: Use treesitter if available, fall back to regex
    /// - Theme support: Load icon theme if available
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn optional_dependencies(&self) -> Vec<ModuleId> {
    ///     vec![
    ///         ModuleId::new("feat-lsp"),       // Enhance with LSP if available
    ///         ModuleId::new("feat-treesitter"), // Better syntax if available
    ///     ]
    /// }
    /// ```
    fn optional_dependencies(&self) -> Vec<ModuleId> {
        Vec::new()
    }

    // ========================================================================
    // Lifecycle (Linux: module_init / module_exit)
    // ========================================================================

    /// Initialize module (Linux equivalent: `probe()` function).
    ///
    /// Returns `ProbeResult` to support deferred probing:
    /// - `Success` - Module is ready
    /// - `Defer(reason)` - Retry later (like Linux `-EPROBE_DEFER`)
    /// - `Failed(err)` - Permanent failure
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;

    /// Cleanup module (Linux equivalent: `remove()` function).
    ///
    /// # Errors
    ///
    /// Returns `ModuleError` if cleanup fails.
    fn exit(&mut self) -> Result<(), ModuleError>;

    // ========================================================================
    // Lifecycle Hooks (Epic #417 Part 2)
    // ========================================================================

    /// Called after ALL modules are loaded (post-init phase).
    ///
    /// Use for cross-module discovery via `ServiceRegistry`. At this point,
    /// all modules have completed their `init()` and registered their services.
    ///
    /// # Use Cases
    ///
    /// - Query services registered by other modules
    /// - Set up inter-module communication channels
    /// - Perform late initialization that depends on other modules
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn on_all_loaded(&mut self, ctx: &ModuleContext) {
    ///     // Now safe to query services from other modules
    ///     if let Some(lsp) = ctx.services.get::<dyn LspProvider>() {
    ///         self.lsp_client = Some(lsp);
    ///     }
    /// }
    /// ```
    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        // Default: no-op
    }

    /// Called when session focuses on a buffer.
    ///
    /// Use for lazy initialization of buffer-specific state. This hook is
    /// called whenever the active buffer changes, allowing modules to:
    ///
    /// - Load buffer-specific configuration
    /// - Initialize syntax highlighting
    /// - Set up LSP connections for the buffer's language
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The newly focused buffer
    /// * `ctx` - Module context for accessing kernel services
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn on_buffer_focus(&mut self, buffer_id: BufferId, ctx: &ModuleContext) {
    ///     // Load undo history for this buffer
    ///     if let Some(undo_provider) = ctx.services.get::<dyn UndoProvider>() {
    ///         undo_provider.load_graceful(buffer_id, &self.buffer_path);
    ///     }
    /// }
    /// ```
    fn on_buffer_focus(&mut self, _buffer_id: BufferId, _ctx: &ModuleContext) {
        // Default: no-op
    }

    /// Called before module unload for cleanup.
    ///
    /// Unlike `exit()`, this hook is specifically for releasing resources
    /// registered with the kernel (services, event handlers, etc.).
    ///
    /// # Difference from `exit()`
    ///
    /// - `exit()`: General cleanup, may fail
    /// - `on_unload()`: Release kernel resources, should not fail
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn on_unload(&mut self) -> Result<(), ModuleError> {
    ///     // Unregister services
    ///     if let Some(registry) = self.service_registry.take() {
    ///         registry.unregister_all();
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ModuleError` if resource release fails.
    fn on_unload(&mut self) -> Result<(), ModuleError> {
        // Default: no-op
        Ok(())
    }

    // ========================================================================
    // Registration (method-based per Clean Arch Proposal)
    // ========================================================================

    /// Get command registrations.
    fn commands(&self) -> Vec<CommandRegistration> {
        Vec::new()
    }

    /// Get keybinding registrations.
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        Vec::new()
    }

    /// Get event handler registrations.
    fn event_handlers(&self) -> Vec<EventHandlerRegistration> {
        Vec::new()
    }

    // ========================================================================
    // Hot Reload Support
    // ========================================================================

    /// Whether this module supports hot reload.
    ///
    /// If true, `save_state()` and `restore_state()` should be implemented.
    /// Hot reload allows updating module code without restarting the editor.
    fn supports_hot_reload(&self) -> bool {
        false
    }

    /// Save module state for hot reload.
    ///
    /// Returns opaque bytes that `restore_state()` can use to restore state.
    /// Return `None` if no state to save.
    ///
    /// # Serialization Format
    ///
    /// The binary format is module-specific. Recommended practices:
    ///
    /// - **Include a version header** for forward compatibility (e.g., first 4 bytes)
    /// - **Use a stable serialization format** like `bincode`, `postcard`, or `rmp-serde`
    /// - **Keep state minimal** - only save what's necessary to restore user experience
    /// - **Handle missing fields gracefully** in `restore_state()`
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn save_state(&self) -> Option<Box<[u8]>> {
    ///     let state = MyModuleState {
    ///         version: 1,
    ///         cursor_history: self.cursor_history.clone(),
    ///         settings: self.settings.clone(),
    ///     };
    ///     bincode::serialize(&state).ok().map(Vec::into_boxed_slice)
    /// }
    /// ```
    fn save_state(&self) -> Option<Box<[u8]>> {
        None
    }

    /// Restore module state after hot reload.
    ///
    /// Called with bytes from previous `save_state()` call.
    ///
    /// # State Version Compatibility
    ///
    /// Modules should handle version mismatches gracefully:
    /// - If the saved state version is **newer** than current, return an error
    /// - If the saved state version is **older**, migrate or use defaults
    /// - If deserialization fails, return an error (module will re-initialize)
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::InitFailed` if:
    /// - Hot reload is not supported
    /// - State version is incompatible
    /// - Deserialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
    ///     let saved: MyModuleState = bincode::deserialize(state)
    ///         .map_err(|e| ModuleError::InitFailed(format!("deserialize: {e}")))?;
    ///
    ///     if saved.version > CURRENT_VERSION {
    ///         return Err(ModuleError::InitFailed("state version too new".into()));
    ///     }
    ///
    ///     self.cursor_history = saved.cursor_history;
    ///     self.settings = saved.settings;
    ///     Ok(())
    /// }
    /// ```
    fn restore_state(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::InitFailed("hot reload not supported".into()))
    }
}
