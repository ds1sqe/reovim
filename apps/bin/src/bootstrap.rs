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
    reovim_driver_command::{CommandHandlerStore, CommandQueryService},
    reovim_driver_input::{
        BindingLayer, KeySequence, KeybindingStore, ModeInfoStore, ResolverRegistry,
    },
    reovim_driver_syntax::{
        CompositeFactory, DefaultLanguageRegistry, LanguageInfoStore, SyntaxDriverFactory,
        SyntaxFactoryStore,
    },
    reovim_driver_vfs::VfsInstance,
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, Module, ModuleContext, ModuleId, MotionEngine,
        OptionRegistry, ProbeResult, ServiceRegistry, TextObjectEngine,
    },
    reovim_module_defaults::DefaultsModule,
    reovim_server::{
        CommandQuerySnapshot, CommandRegistry, KeymapRegistry, ModeEntry, ModeRegistry,
        SessionState, SyntaxSessionState,
    },
};

/// Collect extension bridges from modules via `BridgeProvider`.
///
/// Initializes modules in a temporary `ServiceRegistry` to collect bridges.
/// Bridges are stateless trait objects — they can be collected once and reused
/// across all sessions. The actual session state is created separately by
/// the session factory.
///
/// This keeps bootstrap decoupled from individual modules: zero module-specific
/// imports needed. Modules self-register their bridges during `init()`.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn collect_bridges() -> reovim_driver_session::bridges::BridgeRegistry {
    use reovim_driver_session::bridges::{BridgeProvider, BridgeRegistry};

    // Initialize modules in a temporary ServiceRegistry just for bridge collection.
    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    let module_ctx = create_module_context(kernel, Arc::clone(&services));
    initialize_modules(&module_ctx);

    let mut registry = BridgeRegistry::new();
    if let Some(provider) = services.get::<BridgeProvider>() {
        for bridge in provider.take_bridges() {
            registry.register_boxed(bridge);
        }
    }

    registry
}

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
#[cfg_attr(coverage_nightly, coverage(off))]
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

    // Register CommandQuerySnapshot for module command queries (#453)
    let command_query_snapshot = Arc::new(CommandQuerySnapshot::from_registry(&command_registry));
    // Register CommandQueryProvider for module-level access (#522)
    let command_query_provider = Arc::new(reovim_driver_command::CommandQueryProvider::new(
        command_query_snapshot.list_all(),
    ));
    services.register(command_query_snapshot);
    services.register(command_query_provider);

    // Build CommandNameIndex for vim dispatch (#547)
    let name_index = Arc::new(command_registry.build_name_index());
    tracing::info!(count = name_index.count(), "Built command name index");
    services.register(name_index);

    // Create session state with populated registries
    let initial_mode = ModeId::new(ModuleId::new("vim"), "normal");
    // Use StandardVfs for real file system operations (required for :e command)
    let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> =
        Arc::new(reovim_driver_vfs::StandardVfs::new());

    // Register VFS in ServiceRegistry so ex-commands can access it (#465)
    services.register(Arc::new(VfsInstance::new(Arc::clone(&vfs))));

    // Register theme system (#541): SharedThemeManager + ThemeLoader
    {
        use reovim_driver_display::style::{BuiltinTheme, SharedThemeManager, ThemeLoader};
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        services.register(Arc::new(theme_manager));
        let theme_loader = ThemeLoader::new();
        services.register(Arc::new(theme_loader));
    }

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
#[cfg_attr(coverage_nightly, coverage(off))]
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
    // Wire Vim-style lookup policy (#542): wait for longer sequences (dd after d)
    keymap_registry.set_default_policy(Arc::new(reovim_module_vim::VimLookupPolicy));
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
                        binding.description,
                        binding.category,
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

/// Resolve a mode string like `"vim:normal"` to a `ModeId`.
///
/// Mode strings in `KeybindingRegistration.modes` use `"module:name"` format.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn trigger_empty_session_handlers(state: &mut SessionState, services: &Arc<ServiceRegistry>) {
    use reovim_driver_session::{
        EmptySessionAction, EmptySessionContext, SessionHandlerKey, SessionHandlerRegistry,
    };

    // Only trigger if no buffers exist (check kernel buffer list)
    if !state.app.kernel.buffers.list().is_empty() {
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn create_kernel_context(services: Arc<ServiceRegistry>) -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        services,
    )
}

/// Create a module context for module initialization.
#[cfg_attr(coverage_nightly, coverage(off))]
fn create_module_context(kernel: KernelContext, services: Arc<ServiceRegistry>) -> ModuleContext {
    // Use default paths - modules can create their own subdirectories
    let data_dir = default_data_dir();
    let cache_dir = default_cache_dir();

    ModuleContext::new(kernel, services, data_dir, cache_dir)
}

/// Initialize all default modules plus any extra modules from environment.
///
/// Modules self-register their services during `init()`:
/// - Resolvers → `ResolverRegistry`
/// - Commands → `CommandHandlerStore`
/// - Keybindings → `KeybindingStore`
/// - Mode info → `ModeInfoStore`
#[cfg_attr(coverage_nightly, coverage(off))]
fn initialize_modules(ctx: &ModuleContext) {
    let modules = DefaultsModule::create_modules();

    tracing::info!(count = modules.len(), "Initializing default modules");

    for mut module in modules {
        init_single_module(&mut *module, ctx);
    }

    // Load extra modules from REOVIM_EXTRA_MODULES env var
    initialize_extra_modules(ctx);

    tracing::info!("Module initialization complete");
}

/// Initialize a single module, logging the result.
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_single_module(module: &mut dyn Module, ctx: &ModuleContext) {
    let id = module.id();
    let name = module.name();

    tracing::debug!(%id, name, "Initializing module");

    match module.init(ctx) {
        ProbeResult::Success => {
            tracing::info!(%id, name, "Module initialized successfully");
        }
        ProbeResult::Defer(msg) => {
            tracing::warn!(%id, name, %msg, "Module deferred initialization");
        }
        ProbeResult::Failed(err) => {
            tracing::error!(%id, name, ?err, "Module initialization failed");
        }
    }
}

/// Initialize extra modules from `REOVIM_EXTRA_MODULES` environment variable.
///
/// Parses a comma-separated list of module names and initializes each.
/// Unknown names are logged as warnings and skipped.
///
/// # Example
///
/// ```bash
/// REOVIM_EXTRA_MODULES=textobjects cargo run -- server --grpc 0
/// ```
#[cfg_attr(coverage_nightly, coverage(off))]
fn initialize_extra_modules(ctx: &ModuleContext) {
    let Ok(extra) = std::env::var("REOVIM_EXTRA_MODULES") else {
        return;
    };

    let names: Vec<&str> = extra
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return;
    }

    tracing::info!(count = names.len(), ?names, "Loading extra modules");

    for name in names {
        if let Some(mut module) = create_extra_module(name) {
            init_single_module(&mut *module, ctx);
        } else {
            tracing::warn!(name, "Unknown extra module, skipping");
        }
    }
}

/// Create an extra module by name.
///
/// Returns `None` for unknown module names.
fn create_extra_module(name: &str) -> Option<Box<dyn Module>> {
    match name {
        "textobjects" => Some(Box::new(reovim_module_textobjects::TextObjectsModule::new())),
        _ => None,
    }
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn configure_syntax_highlighting(state: &mut SessionState, services: &Arc<ServiceRegistry>) {
    // Get the SyntaxFactoryStore (populated by treesitter modules during init)
    let Some(store) = services.get::<SyntaxFactoryStore>() else {
        tracing::debug!("No SyntaxFactoryStore found, syntax highlighting disabled");
        return;
    };

    // Take all factories and build a CompositeFactory that routes by language ID
    let factories = store.take_factories();
    if factories.is_empty() {
        tracing::debug!("No syntax factories registered");
        return;
    }

    let composite = CompositeFactory::new(factories);
    let count = composite.factory_count();
    let factory = Arc::new(composite);
    let languages = factory.supported_languages();

    tracing::info!(count, ?languages, "Configured syntax highlighting from modules");

    // Configure SyntaxSessionState with the factory (#491: use app.extensions)
    let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();
    syntax_state.set_factory(factory);

    // Build language registry from LanguageInfoStore (populated by treesitter modules)
    if let Some(lang_store) = services.get::<LanguageInfoStore>() {
        let lang_infos = lang_store.take_all();
        if !lang_infos.is_empty() {
            let registry = DefaultLanguageRegistry::new(lang_infos);
            tracing::info!(?registry, "Built language registry");
            syntax_state.set_registry(Arc::new(registry));
        }
    }
}

/// Get the default data directory for modules.
///
/// Returns `~/.local/share/reovim/modules/` on Unix,
/// or equivalent on other platforms.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "bootstrap_tests.rs"]
mod tests;
