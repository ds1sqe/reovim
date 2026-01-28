//! Module bootstrap - initializes default modules for the server.
//!
//! This module provides the `create_session_state` function that creates
//! a `SessionState` with fully-initialized module registries.
//!
//! # Architecture
//!
//! The bootstrap process follows the Linux kernel module loading pattern:
//! 1. Create a `ServiceRegistry` for cross-module service discovery
//! 2. Create a `ModuleContext` for module initialization
//! 3. Load default modules (`DefaultsModule::create_modules()`)
//! 4. Initialize each module (calls `Module::init()`)
//! 5. Create `SessionState` that uses services from the registry
//!
//! # Module Self-Registration
//!
//! Modules self-register their services during `init()`:
//! - `VimModule` registers resolvers, keybindings, commands
//! - `UndoModule` registers undo providers
//! - `VfsLocalModule` registers filesystem providers
//! - etc.
//!
//! The `SessionState` then queries these registries via `ServiceRegistry`.

use std::sync::Arc;

use {
    parking_lot::RwLock,
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, ModuleContext, ModuleId, MotionEngine,
        OptionRegistry, ProbeResult, RegisterBank, ServiceRegistry, TextObjectEngine,
    },
    reovim_module_defaults::DefaultsModule,
    reovim_server::SessionState,
};

/// Create a session state with fully-initialized module registries.
///
/// This is the entry point for module loading. It:
/// 1. Creates a `ServiceRegistry` for cross-module service discovery
/// 2. Initializes all default modules (vim, editor, motions, etc.)
/// 3. Returns a `SessionState` ready for use
///
/// # Example
///
/// ```ignore
/// use apps_bin::bootstrap::create_session_state;
/// use reovim_server::{Server, ServerConfig};
///
/// let server = Server::with_session_factory(
///     ServerConfig::default(),
///     Box::new(|| create_session_state()),
/// );
/// server.run().await?;
/// ```
#[must_use]
pub fn create_session_state() -> SessionState {
    // Create shared service registry
    let services = Arc::new(ServiceRegistry::new());

    // Create kernel context with service registry
    let kernel = create_kernel_context(Arc::clone(&services));

    // Create module context for initialization
    let module_ctx = create_module_context(kernel.clone(), Arc::clone(&services));

    // Initialize all default modules
    initialize_modules(&module_ctx);

    // Create session state with the initialized kernel
    // The session state will use services from the registry via kernel.services
    let initial_mode = ModeId::new(ModuleId::new("vim"), "normal");
    let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(reovim_driver_vfs::MockVfs::new());

    SessionState::new(kernel, initial_mode, vfs)
}

/// Create a kernel context with the given service registry.
fn create_kernel_context(services: Arc<ServiceRegistry>) -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(RegisterBank::new())),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        services,
    )
}

/// Create a module context for module initialization.
fn create_module_context(kernel: KernelContext, services: Arc<ServiceRegistry>) -> ModuleContext {
    // Use default paths - modules can create their own subdirectories
    let data_dir = default_data_dir();
    let cache_dir = default_cache_dir();

    ModuleContext::new(kernel, services, data_dir, cache_dir)
}

/// Initialize all default modules.
///
/// Modules self-register their services during `init()`:
/// - Resolvers → `ResolverRegistry`
/// - Commands → `CommandHandlerStore`
/// - Keybindings → `KeybindingStore`
/// - Mode info → `ModeInfoStore`
fn initialize_modules(ctx: &ModuleContext) {
    // Get all default modules
    let modules = DefaultsModule::create_modules();

    tracing::info!(count = modules.len(), "Initializing default modules");

    for mut module in modules {
        let id = module.id();
        let name = module.name();

        tracing::debug!(%id, name, "Initializing module");

        match module.init(ctx) {
            ProbeResult::Success => {
                tracing::info!(%id, name, "Module initialized successfully");
            }
            ProbeResult::Defer(msg) => {
                tracing::warn!(%id, name, %msg, "Module deferred initialization");
                // TODO: Implement deferred module loading
            }
            ProbeResult::Failed(err) => {
                tracing::error!(%id, name, ?err, "Module initialization failed");
                // Continue with other modules - don't fail the whole bootstrap
            }
        }
    }

    tracing::info!("Module initialization complete");
}

/// Get the default data directory for modules.
///
/// Returns `~/.local/share/reovim/modules/` on Unix,
/// or equivalent on other platforms.
fn default_data_dir() -> std::path::PathBuf {
    // Use XDG_DATA_HOME or fallback to ~/.local/share
    std::env::var("XDG_DATA_HOME")
        .map_or_else(
            |_| {
                std::env::var("HOME").map_or_else(
                    |_| std::path::PathBuf::from("."),
                    |h| std::path::PathBuf::from(h).join(".local").join("share"),
                )
            },
            std::path::PathBuf::from,
        )
        .join("reovim")
        .join("modules")
}

/// Get the default cache directory for modules.
///
/// Returns `~/.cache/reovim/modules/` on Unix,
/// or equivalent on other platforms.
fn default_cache_dir() -> std::path::PathBuf {
    // Use XDG_CACHE_HOME or fallback to ~/.cache
    std::env::var("XDG_CACHE_HOME")
        .map_or_else(
            |_| {
                std::env::var("HOME").map_or_else(
                    |_| std::path::PathBuf::from("."),
                    |h| std::path::PathBuf::from(h).join(".cache"),
                )
            },
            std::path::PathBuf::from,
        )
        .join("reovim")
        .join("modules")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session_state() {
        // This test verifies that module bootstrap doesn't panic
        let state = create_session_state();

        // Verify we have a valid mode
        let mode = state.current_mode();
        assert!(mode.name().contains("normal"), "Expected normal mode, got {}", mode.name());
    }

    #[test]
    fn test_modules_register_services() {
        use reovim_driver_input::ResolverRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let kernel = create_kernel_context(Arc::clone(&services));
        let ctx = create_module_context(kernel, Arc::clone(&services));

        initialize_modules(&ctx);

        // After module initialization, services should be registered
        // Check for ResolverRegistry (registered by VimModule)
        let resolver_registry = services.get::<ResolverRegistry>();
        assert!(
            resolver_registry.is_some(),
            "ResolverRegistry should be registered by VimModule"
        );
    }
}
