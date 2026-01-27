//! Registry initialization.
//!
//! Contains the "generation position" functions that wire up modules,
//! registries, and session defaults.

use std::{path::PathBuf, sync::Arc};

use {
    reovim_driver_buffer::{BufferManagerKey, BufferManagerRegistry},
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_display::{
        OverlayContentKey, OverlayContentRegistry,
        layout::{CompositorKey, CompositorRegistry, RootCompositor},
    },
    reovim_driver_input::{
        KeybindingStore, ModeInfoStore, ModeProviderKey, ModeProviderRegistry, ResolverRegistry,
    },
    reovim_driver_search::{SearchKey, SearchProviderRegistry},
    reovim_driver_session::{EmptySessionContext, SessionHandlerKey, SessionHandlerRegistry},
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        EventBus, KernelContext, Module, ModuleContext, Service, ServiceRegistry,
    },
};

/// Holder for defaults bundle modules (#440).
///
/// Modules that register event subscriptions must be kept alive for the
/// subscriptions to remain active (RAII pattern). This holder stores
/// initialized modules and is registered in `ServiceRegistry`.
struct DefaultsModuleHolder {
    /// Modules kept alive for their event subscriptions. Never read, only held.
    #[allow(dead_code)]
    modules: Vec<Box<dyn Module>>,
}

impl Service for DefaultsModuleHolder {}

use crate::server::capture::CaptureTracker;

use crate::server::{
    config,
    module::{
        ModuleConfig, ModuleLoader, ModuleManager, load_from_config, wire_module_keybindings,
    },
    registry::{self, CommandRegistry, EmptySessionHandlerRegistry, KeymapRegistry, ModeRegistry},
};

/// Build the empty session handler registry.
///
/// Queries handlers from `ServiceRegistry` (Epic #417 Phase 5).
/// Modules register their handlers during `init()` using typed keys.
pub fn build_empty_session_registry(services: &ServiceRegistry) -> EmptySessionHandlerRegistry {
    let mut registry = EmptySessionHandlerRegistry::new();

    // Get handler from ServiceRegistry using typed key (Epic #417)
    // ScratchBufferModule registers its handler during init()
    if let Some(handler_registry) = services.get::<SessionHandlerRegistry>() {
        if let Some(handler) = handler_registry.get(&SessionHandlerKey::Empty) {
            registry.register(handler);
            tracing::debug!("got empty session handler from ServiceRegistry (Epic #417)");
        } else {
            tracing::warn!("no empty session handler registered with SessionHandlerKey::Empty");
        }
    } else {
        tracing::warn!("no SessionHandlerRegistry in ServiceRegistry");
    }

    tracing::debug!(handlers = registry.len(), "built empty session handler registry");

    registry
}

/// Handle empty session by calling registered handlers.
///
/// If no buffers exist and a handler returns `CreateBuffer`, creates
/// the buffer in the kernel context. This runs synchronously at session
/// startup.
pub fn handle_empty_session(kernel: &KernelContext, registry: &EmptySessionHandlerRegistry) {
    use reovim_driver_session::EmptySessionAction;

    // Check if session already has buffers
    if kernel.buffers.count() > 0 {
        return;
    }

    // No file args for now (CLI file opening is separate)
    let file_args: Vec<String> = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();

    let ctx = EmptySessionContext {
        session_id: 0, // Session ID not used in current handlers
        file_args: &file_args,
        cwd: &cwd,
    };

    if let Some(EmptySessionAction::CreateBuffer { name: _, content }) = registry.resolve(&ctx) {
        let buffer_id = kernel.buffers.create();
        if !content.is_empty()
            && let Some(buffer) = kernel.buffers.get(buffer_id)
        {
            buffer.write().set_content(&content);
        }
        tracing::info!(
            buffer_id = buffer_id.as_usize(),
            "Created initial buffer for empty session"
        );
    }
}

/// Build default registries with keybindings wired from default modules.
///
/// This is the "generation position" where modules are registered and
/// their keybindings are wired to the session registries.
///
/// Module loading follows this precedence:
/// 1. Dynamic modules from XDG paths (`~/.local/share/reovim/modules/`)
/// 2. Static fallback for modules not found in paths
///
/// Returns the registries plus resolver registry, compositor, provider registries,
/// and kernel context (for sharing event bus with session).
///
/// # Panics
///
/// Panics if no VFS provider or default mode provider is registered.
/// This is the "panic fast" behavior for essential providers.
#[allow(clippy::too_many_lines)]
#[allow(clippy::type_complexity)]
pub fn build_default_registries() -> (
    ModeRegistry,
    CommandRegistry,
    KeymapRegistry,
    ModuleManager,
    ResolverRegistry,
    Option<Box<dyn RootCompositor>>,
    registry::DefaultModeProviderRegistry,
    Arc<ServiceRegistry>,
    KernelContext,
) {
    let mut mode_registry = ModeRegistry::new();
    let mut command_registry = CommandRegistry::new();
    let mut keymap_registry = KeymapRegistry::new();
    let mut mode_providers = registry::DefaultModeProviderRegistry::new();

    // Create shared ServiceRegistry for Epic #417 typed key pattern
    let services = Arc::new(ServiceRegistry::new());

    // Register CaptureTracker for TUI capture relay (#447)
    // This is server-specific, not module-provided, so we register it directly.
    services.register(Arc::new(CaptureTracker::new()));
    tracing::debug!("registered CaptureTracker in ServiceRegistry (#447)");

    // Register OverlayContentRegistry with OverlayContentStorage provider (#457)
    // This provides storage for overlay popup content (which-key, diagnostics, etc.)
    // The storage is also registered standalone for direct write access from modules.
    {
        use reovim_driver_display::OverlayContentStorage;
        let overlay_registry = OverlayContentRegistry::new();
        let provider = Arc::new(OverlayContentStorage::new());

        // Register storage standalone for direct module access (writes)
        services.register(Arc::clone(&provider));

        // Register in registry for renderer access (reads)
        overlay_registry.register(OverlayContentKey::new("which-key"), provider);
        services.register(Arc::new(overlay_registry));
        tracing::debug!("registered OverlayContentRegistry in ServiceRegistry (#457)");
    }

    // Register SharedThemeManager with default Dark theme (#439)
    // Theme is server-specific (policy), ThemeManager is driver mechanism.
    {
        use reovim_driver_display::style::{BuiltinTheme, SharedThemeManager};
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        services.register(Arc::new(theme_manager));
        tracing::debug!("registered SharedThemeManager in ServiceRegistry (#439)");
    }

    // Create shared EventBus FIRST - before any module initialization (#440)
    // This bus will be used by both modules (for subscriptions) and session (for emitting)
    let shared_event_bus = Arc::new(EventBus::new());

    // Create shared OptionRegistry (#458) - modules register options during init()
    // and the session uses the same registry for :set commands and rendering.
    let shared_options = Arc::new(reovim_kernel::api::v1::OptionRegistry::new());
    super::kernel::register_default_options(&shared_options);

    // Create minimal kernel context for module initialization
    // Uses the shared event bus and options so modules can subscribe to events
    // and register options (#458)
    let module_init_kernel = KernelContext::with_event_bus_services_and_options(
        Arc::clone(&shared_event_bus),
        Arc::clone(&services),
        Arc::clone(&shared_options),
    );

    // Initialize defaults bundle modules (Epic #417 Phase 5)
    // Modules register their services (undo, buffer, search, scratch-buffer) during init()
    // Use the shared event bus so modules can subscribe to events (#440)
    let defaults_init_ctx = ModuleContext::new(
        module_init_kernel,
        services.clone(),
        PathBuf::from("/tmp/reovim-init/defaults/data"),
        PathBuf::from("/tmp/reovim-init/defaults/cache"),
    );

    // CRITICAL (#440): Collect and store modules to keep event subscriptions alive.
    // Modules use RAII for subscriptions - when the module drops, subscriptions are canceled.
    let mut defaults_modules: Vec<Box<dyn Module>> = Vec::new();
    for mut module in reovim_module_defaults::DefaultsModule::create_modules() {
        let module_id = module.id();
        match module.init(&defaults_init_ctx) {
            reovim_kernel::api::v1::ProbeResult::Success => {
                tracing::debug!(module = %module_id, "initialized defaults module");
            }
            result => {
                tracing::warn!(module = %module_id, ?result, "defaults module init returned non-success");
            }
        }
        defaults_modules.push(module);
    }

    // Store modules in ServiceRegistry to keep them alive for session lifetime (#440)
    let module_count = defaults_modules.len();
    services.register(Arc::new(DefaultsModuleHolder {
        modules: defaults_modules,
    }));
    tracing::info!(count = module_count, "stored defaults modules in ServiceRegistry (#440)");

    // Create real kernel context AFTER modules are initialized (#440, #458)
    // BufferManager is now available in ServiceRegistry
    // Uses the same event bus that modules subscribed to
    // Uses the same options registry that modules registered options in (#458)
    let kernel = super::real_kernel_context_with_options(
        Arc::clone(&services),
        shared_event_bus,
        shared_options,
    );

    // VFS provider now comes from vfs-local module via ServiceRegistry (Epic #417)

    // Load module configuration from ~/.config/reovim/config.toml
    let module_config = ModuleConfig::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load module config, using defaults");
        ModuleConfig::default()
    });

    // Create module loader and load modules from config
    // Epic #417 Part 3: No static module factory - all built-in modules come from defaults bundle
    let mut loader = ModuleLoader::new();
    let load_stats = load_from_config(&mut loader, &module_config, None);

    tracing::info!(
        dynamic = load_stats.dynamic_loaded.len(),
        static_ = load_stats.static_loaded.len(),
        not_found = load_stats.not_found.len(),
        failed = load_stats.failed.len(),
        "loaded modules from config"
    );

    // Create ModuleManager from the loader
    let module_manager = ModuleManager::with_loader(loader);

    // Initialize dynamically loaded modules (#457)
    // Modules need init() called to register their services in ServiceRegistry
    // Skip modules that might conflict with defaults bundle (editor, motions, vim)
    // Only init non-core modules like which-key
    let core_modules = [
        "editor",
        "motions",
        "vim",
        "layout",
        "operators",
        "keymap",
        "commands",
    ];
    for module_id in &load_stats.dynamic_loaded {
        // Skip core modules that are already initialized via defaults bundle
        if core_modules.contains(&module_id.as_str()) {
            tracing::debug!(module = %module_id, "skipping dynamic init (already in defaults)");
            continue;
        }
        match module_manager.init_module(module_id, &defaults_init_ctx) {
            Ok(()) => {
                tracing::info!(module = %module_id, "initialized dynamic module (#457)");
            }
            Err(e) => {
                tracing::warn!(module = %module_id, error = %e, "failed to initialize dynamic module");
            }
        }
    }

    // Epic #417 Part 3: Extract modes from ModeInfoStore (modules registered during init)
    if let Some(mode_store) = services.get::<ModeInfoStore>() {
        for mode_info in mode_store.take_modes() {
            mode_registry.register(registry::ModeEntry::from_info(mode_info));
        }
        tracing::info!(count = mode_registry.len(), "registered modes from ModeInfoStore");
    } else {
        tracing::warn!("no ModeInfoStore in ServiceRegistry - no modes registered");
    }

    // Epic #417 Part 3: Extract commands from CommandHandlerStore (modules registered during init)
    if let Some(command_store) = services.get::<CommandHandlerStore>() {
        let handlers = command_store.take_handlers();
        let count = handlers.len();
        for handler in handlers {
            command_registry.register(handler);
        }
        tracing::info!(count, "registered commands from CommandHandlerStore");
    } else {
        tracing::warn!("no CommandHandlerStore in ServiceRegistry - no commands registered");
    }

    // Register CommandQueryService for module access (#453)
    // Snapshot captures all command metadata for tab completion, help, etc.
    let command_snapshot = registry::CommandQuerySnapshot::from_registry(&command_registry);
    services.register(Arc::new(command_snapshot));
    tracing::info!(
        count = command_registry.len(),
        "registered CommandQueryService in ServiceRegistry (#453)"
    );

    // Epic #417 Part 3: Extract keybindings from KeybindingStore (modules registered during init)
    if let Some(keybinding_store) = services.get::<KeybindingStore>() {
        let keybindings = keybinding_store.take_keybindings();
        let module_id = reovim_kernel::api::v1::ModuleId::new("defaults");
        match wire_module_keybindings(
            &module_id,
            &keybindings,
            &mut keymap_registry,
            &mode_registry,
        ) {
            Ok(stats) => {
                tracing::info!(
                    wired = stats.keybindings_wired,
                    skipped = stats.keybindings_skipped,
                    "wired keybindings from KeybindingStore"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to wire keybindings from KeybindingStore");
            }
        }
    } else {
        tracing::warn!("no KeybindingStore in ServiceRegistry - no keybindings wired");
    }

    // Epic #417 Part 3: Get resolvers from ServiceRegistry (VimModule registered during init)
    // Clone the inner data since we need an owned ResolverRegistry for the return tuple.
    // We need to access the underlying data and clone it.
    let resolver_registry =
        services
            .get::<ResolverRegistry>()
            .map_or_else(ResolverRegistry::new, |arc| {
                // Create a new registry and copy resolvers from the ServiceRegistry one
                let new_registry = ResolverRegistry::new();
                for mode in arc.modes() {
                    if let Some(resolver) = arc.get(&mode) {
                        new_registry.register_arc(resolver);
                    }
                }
                new_registry
            });
    tracing::info!(count = resolver_registry.len(), "using resolvers from ServiceRegistry");

    // Epic #417 Part 3: Extract default mode provider from ServiceRegistry
    // VimModule registers VimDefaultModeProvider during init()
    if let Some(provider_registry) = services.get::<ModeProviderRegistry>() {
        if let Some(provider) = provider_registry.get(&ModeProviderKey::Entry) {
            mode_providers.register(provider);
            tracing::info!("extracted default mode provider from ServiceRegistry");
        } else {
            tracing::warn!("no entry mode provider in ModeProviderRegistry");
        }
    } else {
        tracing::warn!("no ModeProviderRegistry in ServiceRegistry");
    }

    // Load user keymap configuration from ~/.config/reovim/keymap.toml
    // User bindings are registered at the User layer (highest priority)
    match config::KeymapConfig::load() {
        Ok(user_config) => {
            if !user_config.is_empty() {
                // Validate config and report warnings for any issues
                let validation_errors = user_config.validate();
                for err in &validation_errors {
                    tracing::warn!(error = %err, "keymap.toml validation warning");
                }

                // Try to apply - will fail on first invalid entry
                match user_config.apply(&mut keymap_registry) {
                    Ok(stats) => {
                        tracing::info!(
                            bindings_added = stats.bindings_added,
                            bindings_removed = stats.bindings_removed,
                            warnings = validation_errors.len(),
                            "applied user keymap configuration"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to apply keymap configuration");
                    }
                }
            }
        }
        Err(config::KeymapConfigError::NoConfigDir) => {
            // Silent - no config directory is fine
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load keymap.toml");
        }
    }

    // Note: LayoutModule is now initialized in the defaults bundle loop above.
    // Compositor is registered via CompositorRegistry during init().

    // Validate services are registered in ServiceRegistry (Epic #417)
    // Modules registered their services during init() via typed key pattern

    // Compositor service
    if services
        .get::<CompositorRegistry>()
        .is_some_and(|r| r.get(&CompositorKey::Root).is_some())
    {
        tracing::info!("compositor registered in ServiceRegistry (Epic #417)");
    }

    // Undo service (Epic #417 Phase 6)
    if services
        .get::<UndoProviderRegistry>()
        .is_some_and(|r| r.get(&UndoKey::Buffer).is_some())
    {
        tracing::info!("undo provider registered in ServiceRegistry (Epic #417)");
    }

    // Buffer manager service (Epic #417 Phase 6)
    if services
        .get::<BufferManagerRegistry>()
        .is_some_and(|r| r.get(&BufferManagerKey::Simple).is_some())
    {
        tracing::info!("buffer manager registered in ServiceRegistry (Epic #417)");
    }

    // Search service (Epic #417 Phase 6)
    if services
        .get::<SearchProviderRegistry>()
        .is_some_and(|r| r.get(&SearchKey::Regex).is_some())
    {
        tracing::info!("search provider registered in ServiceRegistry (Epic #417)");
    }

    // Extract compositor from ServiceRegistry using typed key (Epic #417 Phase 5)
    // Replaces old Module::compositor() + CompositorBox downcast pattern
    let compositor: Option<Box<dyn RootCompositor>> = services
        .get::<CompositorRegistry>()
        .and_then(|registry| registry.get(&CompositorKey::Root))
        .map(|arc_compositor| arc_compositor.boxed_clone());

    if compositor.is_some() {
        tracing::info!("extracted compositor from ServiceRegistry (Epic #417 Phase 5)");
    }

    // PANIC FAST: Validate essential providers exist (Epic #415)
    // VFS now validated via ServiceRegistry (Epic #417)
    mode_providers.validate();

    // ========================================================================
    // Which-Key Saturator Spawning (#457)
    // ========================================================================
    //
    // The saturator is a background task that filters keybindings and updates
    // the which-key cache. It needs:
    // - Cache handle (from WhichKeyCacheHandle in ServiceRegistry)
    // - KeymapQuery (from keymap_registry)
    // - CommandDescriptionProvider (wrapper around command_registry)
    //
    // The sender is registered in ServiceRegistry for input handler access.
    {
        use {
            crate::server::registry::WhichKeySaturatorSender,
            reovim_module_which_key::{
                CommandDescriptionProvider, WhichKeyCacheHandle, spawn_saturator,
            },
        };

        // Wrapper for CommandRegistry that implements CommandDescriptionProvider
        struct CommandRegistryDescriptions {
            registry: Arc<CommandRegistry>,
        }

        impl CommandDescriptionProvider for CommandRegistryDescriptions {
            fn description(&self, id: &reovim_kernel::api::v1::CommandId) -> Option<&str> {
                self.registry.get(id).map(|h| h.description())
            }
        }

        // Spawn saturator if which-key cache is available
        if let Some(cache_handle) = services.get::<WhichKeyCacheHandle>() {
            let cache = cache_handle.clone_inner();

            // Create KeymapQuery wrapper (KeymapRegistry implements KeymapQuery)
            let keymap_query: Arc<KeymapRegistry> = Arc::new(keymap_registry.clone());

            // Create CommandDescriptionProvider wrapper
            let descriptions = Arc::new(CommandRegistryDescriptions {
                registry: Arc::new(command_registry.clone()),
            });

            // Spawn the saturator background task
            let saturator_handle = spawn_saturator(cache, keymap_query, descriptions);

            // Register the sender for input handler access
            services.register(Arc::new(WhichKeySaturatorSender::new(saturator_handle.tx)));

            tracing::info!("which-key: saturator spawned and sender registered (#457)");
        } else {
            tracing::debug!("which-key: cache not available, skipping saturator spawn");
        }
    }

    (
        mode_registry,
        command_registry,
        keymap_registry,
        module_manager,
        resolver_registry,
        compositor,
        mode_providers,
        services,
        kernel,
    )
}
