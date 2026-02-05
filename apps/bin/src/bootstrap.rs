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
    reovim_driver_command::{CommandHandlerStore, ExCommandHandlerStore, ExCommandRegistry},
    reovim_driver_input::{
        BindingLayer, KeySequence, KeybindingStore, ModeInfoStore, ResolverRegistry,
    },
    reovim_driver_syntax::SyntaxFactoryStore,
    reovim_driver_vfs::VfsInstance,
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, ModuleContext, ModuleId, MotionEngine,
        OptionRegistry, ProbeResult, RegisterBank, ServiceRegistry, TextObjectEngine,
    },
    reovim_module_defaults::DefaultsModule,
    reovim_server::{
        CommandRegistry, KeymapRegistry, ModeEntry, ModeRegistry, SessionState, SyntaxSessionState,
    },
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

    // Extract registries from ServiceRegistry (populated by modules during init)
    let (mode_registry, command_registry, keymap_registry, resolver_registry) =
        extract_registries(&services);

    // Extract ex-command handlers and create registry (#465)
    extract_ex_command_registry(&services);

    // Create session state with populated registries
    let initial_mode = ModeId::new(ModuleId::new("vim"), "normal");
    // Use StandardVfs for real file system operations (required for :e command)
    let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> =
        Arc::new(reovim_driver_vfs::StandardVfs::new());

    // Register VFS in ServiceRegistry so ex-commands can access it (#465)
    services.register(Arc::new(VfsInstance::new(Arc::clone(&vfs))));

    let mut state = SessionState::with_registries(
        kernel,
        initial_mode,
        vfs,
        mode_registry,
        command_registry,
        keymap_registry,
        resolver_registry,
        None, // No compositor (client-side concern)
    );

    // Extract syntax factory from SyntaxFactoryStore (populated by treesitter modules)
    // and configure SyntaxSessionState
    configure_syntax_highlighting(&mut state, &services);

    // Trigger empty session handlers to create scratch buffer if needed
    trigger_empty_session_handlers(&mut state, &services);

    state
}

/// Extract registries from `ServiceRegistry` after module initialization.
///
/// Modules self-register their services into `ServiceRegistry` during `init()`.
/// This function extracts those registrations and converts them into the
/// server-side registry types that `SessionState` uses:
///
/// | Module Store | Server Registry | Purpose |
/// |-------------|-----------------|---------|
/// | `ModeInfoStore` | `ModeRegistry` | Mode metadata |
/// | `CommandHandlerStore` | `CommandRegistry` | Command dispatch |
/// | `KeybindingStore` | `KeymapRegistry` | Key → command mapping |
/// | `ResolverRegistry` | `ResolverRegistry` | Mode-specific key interpretation |
fn extract_registries(
    services: &Arc<ServiceRegistry>,
) -> (ModeRegistry, CommandRegistry, KeymapRegistry, ResolverRegistry) {
    // 1. Modes: ModeInfoStore → ModeRegistry
    //    ModeEntry::from_info() converts driver-side ModeInfo to server-side ModeEntry
    let mut mode_registry = ModeRegistry::new();
    if let Some(store) = services.get::<ModeInfoStore>() {
        for info in store.take_modes() {
            mode_registry.register(ModeEntry::from_info(info));
        }
    }
    tracing::info!(count = mode_registry.len(), "Extracted modes");

    // 2. Commands: CommandHandlerStore → CommandRegistry
    //    Arc<dyn CommandHandler> is shared directly (no conversion needed)
    let mut command_registry = CommandRegistry::new();
    if let Some(store) = services.get::<CommandHandlerStore>() {
        for handler in store.take_handlers() {
            command_registry.register(handler);
        }
    }
    tracing::info!(count = command_registry.len(), "Extracted commands");

    // 3. Keybindings: KeybindingStore → KeymapRegistry
    //    Each KeybindingRegistration declares modes as "module:name" strings
    //    (e.g., "vim:normal"). We resolve these to ModeId via mode_registry.
    let mut keymap_registry = KeymapRegistry::new();
    if let Some(store) = services.get::<KeybindingStore>() {
        let mut wired = 0usize;
        for binding in store.take_keybindings() {
            if !binding.enabled {
                continue;
            }
            let Some(keys) = KeySequence::parse(binding.keys) else {
                tracing::warn!(keys = binding.keys, "Failed to parse keybinding");
                continue;
            };

            for mode_str in binding.modes {
                if let Some(mode_id) = resolve_mode_str(mode_str, &mode_registry) {
                    keymap_registry.register_at_layer(
                        BindingLayer::Policy,
                        mode_id,
                        keys.clone(),
                        binding.command_id.clone(),
                    );
                    wired += 1;
                } else {
                    tracing::warn!(
                        mode = mode_str,
                        keys = binding.keys,
                        "Mode not found for keybinding"
                    );
                }
            }
        }
        tracing::info!(wired, "Wired keybindings");
    }

    // 4. Resolvers: ResolverRegistry → ResolverRegistry
    //    Same type - extract Arc<dyn ModeKeyResolver> by mode and re-register
    let resolver_registry = ResolverRegistry::new();
    if let Some(reg) = services.get::<ResolverRegistry>() {
        for mode_id in reg.modes() {
            if let Some(resolver) = reg.get(&mode_id) {
                resolver_registry.register_arc(resolver);
            }
        }
    }
    tracing::info!(count = resolver_registry.len(), "Extracted resolvers");

    (mode_registry, command_registry, keymap_registry, resolver_registry)
}

/// Extract ex-command handlers and create `ExCommandRegistry`.
///
/// Follows the same pattern as `extract_registries`:
/// - `ExCommandHandlerStore` → `ExCommandRegistry`
///
/// The registry is stored back in `ServiceRegistry` so the vim module's
/// `ExitCommandLineMode` can look it up at runtime.
fn extract_ex_command_registry(services: &Arc<ServiceRegistry>) {
    // 5. Ex-commands: ExCommandHandlerStore → ExCommandRegistry (#465)
    //    Ex-commands like :w, :q, :e are dispatched through this registry
    if let Some(store) = services.get::<ExCommandHandlerStore>() {
        let handlers = store.take_handlers();
        let registry = ExCommandRegistry::from_handlers(handlers);
        let count = registry.len();
        services.register(Arc::new(registry));
        tracing::info!(count, "Extracted ex-commands");
    } else {
        // No ex-commands registered - create empty registry
        services.register(Arc::new(ExCommandRegistry::new()));
        tracing::debug!("No ex-commands registered, created empty registry");
    }
}

/// Resolve a mode string like `"vim:normal"` to a `ModeId`.
///
/// Mode strings in `KeybindingRegistration.modes` use `"module:name"` format.
fn resolve_mode_str<'a>(mode_str: &str, mode_registry: &'a ModeRegistry) -> Option<&'a ModeId> {
    if let Some((module, name)) = mode_str.split_once(':') {
        mode_registry.find_by_name(module, name)
    } else {
        // Bare mode name without module prefix - shouldn't happen in practice
        tracing::warn!(mode = mode_str, "Mode string missing module prefix");
        None
    }
}

/// Trigger empty session handlers to create initial buffer.
///
/// If no buffers exist after module initialization, call registered
/// `EmptySessionHandler`s to create a scratch buffer.
fn trigger_empty_session_handlers(state: &mut SessionState, services: &Arc<ServiceRegistry>) {
    use reovim_driver_session::{
        EmptySessionAction, EmptySessionContext, SessionHandlerKey, SessionHandlerRegistry,
    };

    // Only trigger if no buffers exist
    if state.active_buffer().is_some() {
        return;
    }

    // Get the handler registry
    let Some(registry) = services.get::<SessionHandlerRegistry>() else {
        tracing::debug!("No SessionHandlerRegistry found, skipping empty session handling");
        return;
    };

    // Get the empty session handler
    let Some(handler) = registry.get(&SessionHandlerKey::Empty) else {
        tracing::debug!("No empty session handler registered");
        return;
    };

    // Create context for handlers
    let cwd = std::env::current_dir().unwrap_or_default();
    let ctx = EmptySessionContext {
        session_id: 0,
        file_args: &[],
        cwd: &cwd,
    };

    // Call handler and execute action
    match handler.handle(&ctx) {
        EmptySessionAction::CreateBuffer { content, .. } => {
            let id = state.create_buffer(&content);
            tracing::info!(?id, "Created scratch buffer from empty session handler");
        }
        EmptySessionAction::None => {
            tracing::debug!("Empty session handler returned None");
        }
    }
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

/// Configure syntax highlighting from `SyntaxFactoryStore`.
///
/// Extracts syntax factories registered by treesitter modules during `init()`
/// and configures the `SyntaxSessionState` with the first available factory.
///
/// This follows the same extraction pattern as other registries:
/// - Modules register into `SyntaxFactoryStore` during `init()`
/// - Bootstrap extracts and configures `SyntaxSessionState`
/// - NO direct imports of language-specific modules here!
fn configure_syntax_highlighting(state: &mut SessionState, services: &Arc<ServiceRegistry>) {
    // Get the SyntaxFactoryStore (populated by treesitter modules during init)
    let Some(store) = services.get::<SyntaxFactoryStore>() else {
        tracing::debug!("No SyntaxFactoryStore found, syntax highlighting disabled");
        return;
    };

    // Take all factories and use the first one
    // Future: aggregate into CompositeFactory for multiple languages
    let factories = store.take_factories();
    if factories.is_empty() {
        tracing::debug!("No syntax factories registered");
        return;
    }

    // For now, use first factory (TODO: CompositeFactory for multiple languages)
    let factory = factories.into_iter().next().unwrap();
    let languages = factory.supported_languages();

    tracing::info!(count = 1, ?languages, "Configured syntax highlighting from modules");

    // Configure SyntaxSessionState with the factory (#491: use app.extensions)
    let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();
    syntax_state.set_factory(factory);
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

        // #491: Use home_mode() instead of removed current_mode()
        let mode = state.home_mode();
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
