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
//! 3. Load default modules (static factories or dynamic `.so` loading)
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

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use {
    reovim_depgraph::{DepEntry, DependencyOrder, check_version_constraints, resolve_dependencies},
    reovim_driver_command::{CommandHandlerStore, CommandQueryService},
    reovim_driver_text_input::{
        KeySequence, KeybindingStore, KeymapQuery as DriverKeymapQuery, LookupPolicyStore,
        ModeInfoStore, ResolverRegistry, key_sequence_to_input_sequence,
    },
    reovim_driver_text_session::LeaderKeyProvider,
    reovim_driver_text_syntax::{
        CompositeFactory, DefaultLanguageRegistry, LanguageInfoStore, SyntaxDriverFactory,
        SyntaxFactoryStore, SyntaxSessionState,
    },
    reovim_kernel::api::v1::{
        ConfigPaths, EventBus, KernelContext, ModeId, Module, ModuleContext, ModuleId, ModuleState,
        OptionRegistry, ServiceRegistry,
    },
    reovim_server::{
        CommandQuerySnapshot, CommandRegistry, KeymapRegistry, ModeEntry, ModeRegistry,
        SessionState,
    },
    reovim_subsys_input::{
        BindingLayer, DefaultInputCodecRegistry, EagerLookupPolicy, LookupPolicy, LookupResult,
        LookupState,
    },
    reovim_subsys_module_config::{BuiltinManifest, ModulesConfig},
    reovim_subsys_module_loader::{
        handle::ModuleHandle, loader::ModuleLoader, registry::ModuleRegistry,
    },
    reovim_subsys_vfs::VfsInstance,
};

// #620: Static module factories — only available when static-modules feature is on.
// Replaces the DefaultsModule god-crate with direct, feature-gated imports.
#[cfg(feature = "static-modules")]
#[path = "static_modules.rs"]
mod static_modules;

// ============================================================================
// Composition-root adapters (#753 E6)
// These bridge driver-tier types to the domain-neutral subsys contracts.
// They live here because the composition root (not the kernel or drivers)
// is the correct site for cross-layer wiring.
// ============================================================================

/// Adapts a driver-tier `KeyLookupPolicy` to the domain-neutral subsys
/// `LookupPolicy<CommandId>` trait. Used by the composition root to wire a
/// module-registered policy (e.g. `VimLookupPolicy`) into the new registry.
#[cfg_attr(coverage_nightly, coverage(off))]
struct KeyPolicyAsLookupPolicy(Arc<dyn reovim_driver_text_input::KeyLookupPolicy>);

#[cfg_attr(coverage_nightly, coverage(off))]
impl LookupPolicy<reovim_kernel::api::v1::CommandId> for KeyPolicyAsLookupPolicy {
    fn resolve(
        &self,
        state: LookupState<reovim_kernel::api::v1::CommandId>,
    ) -> LookupResult<reovim_kernel::api::v1::CommandId> {
        use reovim_driver_text_input::{KeyLookupResult, KeyLookupState};
        let key_state = match state {
            LookupState::ExactOnly(c) => KeyLookupState::ExactOnly(c),
            LookupState::ExactWithLonger { exact } => KeyLookupState::ExactWithLonger { exact },
            LookupState::PrefixOnly => KeyLookupState::PrefixOnly,
            LookupState::NotFound => KeyLookupState::NotFound,
        };
        match self.0.resolve(key_state) {
            KeyLookupResult::Found(c) => LookupResult::Found(c),
            KeyLookupResult::Prefix => LookupResult::Prefix,
            KeyLookupResult::NotFound => LookupResult::NotFound,
        }
    }
}

/// Bridges a `CommandHandler` (from driver-command) to `CommandHandle` (from
/// text-session api). Restored from pre-#753-E6 shape per the E6 comment
/// directing bootstrap to own this adapter.
#[cfg_attr(coverage_nightly, coverage(off))]
struct HandlerBridge(Arc<dyn reovim_driver_command::CommandHandler>);

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_text_session::api::CommandHandle for HandlerBridge {
    fn execute(
        &self,
        runtime: &mut reovim_driver_text_session::SessionRuntime<'_>,
        ctx: &reovim_driver_command::CommandContext,
    ) -> reovim_driver_command::CommandResult {
        self.0.execute(runtime, ctx)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
struct CommandHandlerExecutor(
    HashMap<reovim_kernel::api::v1::CommandId, Arc<dyn reovim_driver_command::CommandHandler>>,
);

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_text_session::api::CommandExecutor for CommandHandlerExecutor {
    fn get_handle(
        &self,
        id: &reovim_kernel::api::v1::CommandId,
    ) -> Option<Arc<dyn reovim_driver_text_session::api::CommandHandle>> {
        self.0.get(id).map(|h| {
            Arc::new(HandlerBridge(Arc::clone(h)))
                as Arc<dyn reovim_driver_text_session::api::CommandHandle>
        })
    }
}

/// Adapts `KeymapRegistry` (subsys, `InputSequence`-based) to the driver-tier
/// `KeymapQuery` trait (which uses `KeySequence` string tokens).
///
/// Key sequences are converted via `key_sequence_to_input_sequence` using the
/// TUI codec from the `DefaultInputCodecRegistry`. If conversion fails (codec
/// not registered or unknown token) the query returns `NotFound`/`false`/`None`.
#[cfg_attr(coverage_nightly, coverage(off))]
struct KeymapRegistryAdapter {
    inner: KeymapRegistry,
    codec_registry: Arc<DefaultInputCodecRegistry>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DriverKeymapQuery for KeymapRegistryAdapter {
    fn query(
        &self,
        mode: &reovim_kernel::api::v1::ModeId,
        keys: &KeySequence,
    ) -> reovim_driver_text_input::KeyLookupState {
        use reovim_driver_text_input::KeyLookupState;
        let Ok(input_seq) = key_sequence_to_input_sequence(keys, self.codec_registry.as_ref())
        else {
            return KeyLookupState::NotFound;
        };
        match self.inner.query(mode, &input_seq) {
            LookupState::ExactOnly(c) => KeyLookupState::ExactOnly(c),
            LookupState::ExactWithLonger { exact } => KeyLookupState::ExactWithLonger { exact },
            LookupState::PrefixOnly => KeyLookupState::PrefixOnly,
            LookupState::NotFound => KeyLookupState::NotFound,
        }
    }

    fn has_longer_bindings(
        &self,
        mode: &reovim_kernel::api::v1::ModeId,
        keys: &KeySequence,
    ) -> bool {
        let Ok(input_seq) = key_sequence_to_input_sequence(keys, self.codec_registry.as_ref())
        else {
            return false;
        };
        self.inner.has_longer_bindings(mode, &input_seq)
    }

    fn get_exact(
        &self,
        mode: &reovim_kernel::api::v1::ModeId,
        keys: &KeySequence,
    ) -> Option<reovim_kernel::api::v1::CommandId> {
        let Ok(input_seq) = key_sequence_to_input_sequence(keys, self.codec_registry.as_ref())
        else {
            return None;
        };
        match self.inner.query(mode, &input_seq) {
            LookupState::ExactOnly(c) | LookupState::ExactWithLonger { exact: c } => Some(c),
            LookupState::PrefixOnly | LookupState::NotFound => None,
        }
    }

    fn bindings_with_prefix(
        &self,
        _mode: &reovim_kernel::api::v1::ModeId,
        _prefix: &KeySequence,
    ) -> Vec<(KeySequence, reovim_driver_text_input::BindingInfo)> {
        // Reverse-conversion from InputSequence to KeySequence is not
        // implemented; return empty (which-key display degrades gracefully).
        Vec::new()
    }
}

/// Embedded builtin module manifest (canonical module list and ordering).
const BUILTINS_TOML: &str = include_str!("../builtins.toml");

/// Parse the embedded builtin module manifest.
///
/// Panics if the embedded TOML is malformed (compile-time guarantee).
fn parse_builtin_manifest() -> BuiltinManifest {
    BuiltinManifest::parse(BUILTINS_TOML).expect("embedded builtins.toml must be valid")
}

/// Module with tracked lifecycle state (#582, #620).
///
/// Wraps a [`ModuleHandle`] with `ModuleState` FSM tracking.
/// Used during bootstrap to track init results and enable
/// `on_all_loaded()` to skip failed modules.
///
/// `ModuleHandle` unifies static (`Box<dyn Module>`) and dynamic
/// (FFI trampoline) modules behind a single interface.
struct TrackedModule {
    handle: ModuleHandle,
    state: ModuleState,
}

struct InitializedModules {
    tracked: Vec<TrackedModule>,
    dependents: HashMap<ModuleId, HashSet<ModuleId>>,
}

/// Result of a single authoritative bootstrap pass.
pub struct BootstrapResult {
    /// Session state ready for the default server session.
    pub session_state: SessionState,
    /// Live module registry used by runner-side gRPC control-plane wiring.
    pub module_registry: Arc<ModuleRegistry>,
    /// Shared module context paired with the live registry.
    pub module_ctx: Arc<ModuleContext>,
    /// Extension bridges collected during the same bootstrap pass.
    pub bridges: reovim_driver_text_session::bridges::BridgeRegistry,
    /// Domain driver for the default session (#753).
    ///
    /// Wired into the server's default session at startup.
    /// Dispatch routes through the domain driver when wired.
    pub domain_driver: Option<Arc<dyn reovim_subsys_session::DomainDriver>>,
}

/// Load user module configuration from `~/.config/reovim/modules.toml`.
///
/// Returns the official preset (all enabled) if:
/// - The config file does not exist (normal case)
/// - The config directory cannot be determined
/// - The config file has parse errors (logged as warning)
#[cfg_attr(coverage_nightly, coverage(off))]
fn load_module_config() -> ModulesConfig {
    let Ok(config_dir) = ConfigPaths::config_dir() else {
        tracing::debug!("Cannot determine config directory, using official preset");
        return ModulesConfig::official();
    };
    let config_path = config_dir.join("modules.toml");

    match ModulesConfig::load(&config_path) {
        Ok(config) => {
            let disabled = config.disabled_modules();
            if !disabled.is_empty() {
                tracing::info!(?disabled, "User module config: disabled modules");
            }
            let disabled_ext = config.disabled_extensions();
            if !disabled_ext.is_empty() {
                tracing::info!(?disabled_ext, "User module config: disabled extensions");
            }
            // Validate against known builtin module IDs (#620: from manifest)
            let manifest = parse_builtin_manifest();
            let known = manifest.module_ids();
            for warning in config.validate_modules(&known) {
                tracing::warn!(%warning, "Module config validation");
            }
            config
        }
        Err(e) => {
            tracing::warn!(%e, "Failed to load modules.toml, using official preset");
            ModulesConfig::official()
        }
    }
}

/// Register `ModuleConfigStore` in `ServiceRegistry` from config settings.
///
/// Extracts per-module settings from `ModulesConfig` and registers them
/// as a `ModuleConfigStore` service so modules can query their config
/// during `init()`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn register_module_config_store(config: &ModulesConfig, services: &Arc<ServiceRegistry>) {
    let store = config.build_config_store();
    if !store.is_empty() {
        tracing::info!("Registering ModuleConfigStore with per-module settings");
        services.register(Arc::new(store));
    }
}

/// Compute the set of extension kinds that should be disabled on the client.
///
/// For each disabled server module, looks up its `extension_kinds()` and
/// collects them into a `HashSet`. This set is passed to the TUI so it
/// can skip loading extensions for disabled server modules.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn compute_disabled_extension_kinds() -> std::collections::HashSet<String> {
    #[cfg(feature = "static-modules")]
    {
        let config = load_module_config();
        let mut disabled_kinds = std::collections::HashSet::new();

        // #620: Use static factory map instead of DefaultsModule
        let registry = static_modules::builtin_registry();
        for (id, factory) in &registry {
            if !config.is_module_enabled(id) {
                let module = factory();
                for kind in module.extension_kinds() {
                    disabled_kinds.insert((*kind).to_string());
                }
            }
        }

        if !disabled_kinds.is_empty() {
            tracing::info!(?disabled_kinds, "Computed disabled extension kinds from module config");
        }
        disabled_kinds
    }

    #[cfg(not(feature = "static-modules"))]
    {
        // Dynamic path: cannot query extension_kinds without loading .so files.
        // Extension filtering requires static modules or dynamic loader (Phase 5).
        tracing::debug!("Static modules disabled, extension kind filtering unavailable");
        std::collections::HashSet::new()
    }
}

/// Collect extension bridges from modules via `BridgeProvider`.
///
/// Initializes modules in a temporary `ServiceRegistry` to collect bridges.
/// Bridges are stateless trait objects — they can be collected once and reused
/// across all sessions. The actual session state is created separately by
/// the session factory.
///
/// This keeps bootstrap decoupled from individual modules: zero module-specific
/// imports needed. Modules self-register their bridges during `init()`.
/// Perform one authoritative bootstrap pass for the runner.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_lines)] // orchestration function; splitting would obscure intent
pub fn bootstrap_runtime() -> BootstrapResult {
    // Load user config (#586)
    let config = load_module_config();

    // Create shared service registry
    let services = Arc::new(ServiceRegistry::new());

    // Register per-module settings before module init (#586)
    register_module_config_store(&config, &services);

    // Create kernel context with service registry
    let kernel = create_kernel_context(Arc::clone(&services));

    // Register TextBufferRegistry for session-layer text access (#740).
    let text_registry = Arc::new(reovim_provider_text::TextBufferRegistry::new());
    services.register(Arc::clone(&text_registry));

    // Register BufferReadAccess so bridges can read buffer content in tick() (#664).
    services.register(Arc::new(reovim_driver_text_session::BufferReadAccess::new(text_registry)));

    // Create module context for initialization
    let module_ctx = Arc::new(create_module_context(kernel.clone(), Arc::clone(&services)));

    // Initialize enabled modules in dependency order (#582, #586, #587).
    let mut initialized = initialize_modules_with_dependents(&config, module_ctx.as_ref());

    // Wire on_all_loaded lifecycle hook (#582, #725)
    call_on_all_loaded(&mut initialized.tracked, module_ctx.as_ref());

    // Check modules.lock staleness (#587) — warning only, missing lock is fine
    check_lockfile_staleness();

    // #610: Construct and register ModuleLoadReport for health-check diagnostics
    build_and_register_load_report(&config, &initialized.tracked, &services);

    // Collect bridges from the same bootstrap pass used for the session and registry.
    let bridges = collect_bridges_from_services(&services, &initialized.tracked);

    let module_registry = build_live_module_registry(initialized);

    // Extract registries from ServiceRegistry (populated by modules during init)
    let (mode_registry, command_registry, keymap_registry, resolver_registry, command_handlers) =
        extract_registries(&services);

    // Register CommandQuerySnapshot for module command queries (#453)
    let command_query_snapshot = Arc::new(CommandQuerySnapshot::from_registry(&command_registry));
    let command_query_provider = Arc::new(reovim_driver_command::CommandQueryProvider::new(
        command_query_snapshot.list_all(),
    ));
    services.register(command_query_snapshot);
    services.register(command_query_provider);

    // Build CommandNameIndex for vim dispatch (#547)
    let name_index = Arc::new(command_registry.build_name_index());
    tracing::info!(count = name_index.count(), "Built command name index");
    services.register(name_index);

    // #623: Read initial mode from personality module, fallback to vim:normal
    let initial_mode = services
        .get::<reovim_driver_text_session::InitialModeProvider>()
        .and_then(|p| p.get())
        .unwrap_or_else(|| ModeId::new(ModuleId::new("vim"), "normal"));
    tracing::info!(mode = %initial_mode, "Selected initial mode from personality module");
    let vfs: Arc<dyn reovim_subsys_vfs::VfsDriver> =
        Arc::new(reovim_subsys_vfs::StandardVfs::new());

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

    // Build GutterRenderer from registered annotation sources + display presenters.
    {
        use reovim_driver_display::{
            AnnotationSourceRegistry, BlamePresenter, DiagnosticPresenter, GitSignsPresenter,
            GutterRenderer, GutterRendererKey, GutterRendererRegistry, LineNumberPresenter,
        };
        if let Some(source_registry) = services.get::<AnnotationSourceRegistry>() {
            let renderer = GutterRenderer::new();
            for source in source_registry.values() {
                renderer.register_source(source);
            }
            renderer.register_presenter(Arc::new(LineNumberPresenter::new()));
            renderer.register_presenter(Arc::new(GitSignsPresenter::new()));
            renderer.register_presenter(Arc::new(BlamePresenter::new()));
            renderer.register_presenter(Arc::new(DiagnosticPresenter::new()));

            let renderer_registry = services.get_or_create::<GutterRendererRegistry>();
            renderer_registry.register(GutterRendererKey::Default, Arc::new(renderer));
            tracing::info!("Built GutterRenderer from annotation sources");
        }
    }

    // Wrap server-side ComponentDataProviders into display-side ComponentProviders.
    {
        use reovim_driver_display::statusline::{
            ComponentDataProviderRegistry, ComponentProviderKey, ComponentProviderRegistry,
            DataProviderAdapter,
        };
        if let Some(data_registry) = services.get::<ComponentDataProviderRegistry>() {
            let provider_registry = services.get_or_create::<ComponentProviderRegistry>();
            for data_provider in data_registry.values() {
                let key = ComponentProviderKey::new(data_provider.id());
                provider_registry.register(key, Arc::new(DataProviderAdapter::new(data_provider)));
            }
            tracing::info!(
                "Wrapped {} data providers into ComponentProviders",
                data_registry.len()
            );
        }
    }

    // Extract compositor from CompositorRegistry (populated by layout module)
    let compositor: Option<Box<dyn reovim_subsys_layout::RootCompositor>> = services
        .get::<reovim_subsys_layout::CompositorRegistry>()
        .and_then(|reg| reg.get(&reovim_subsys_layout::CompositorKey::Root))
        .map(|arc| arc.boxed_clone());

    let mut session_state = SessionState::with_registries(
        kernel,
        initial_mode,
        vfs,
        mode_registry,
        command_registry,
        keymap_registry,
        compositor,
    );

    configure_syntax_highlighting(&mut session_state, &services);
    trigger_empty_session_handlers(&session_state, &services);
    session_state.ensure_initial_compositor_window();

    // Build TextDomainDriver for domain-neutral dispatch (#753).
    let domain_driver =
        build_text_domain_driver(&session_state, &services, resolver_registry, command_handlers);

    BootstrapResult {
        session_state,
        module_registry,
        module_ctx,
        bridges,
        domain_driver,
    }
}

/// Create a session state with fully-initialized module registries.
///
/// Used by tests that need a complete session state without wiring a full
/// server. Production code uses `bootstrap_runtime()` directly.
#[cfg(test)]
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn create_session_state() -> SessionState {
    bootstrap_runtime().session_state
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
#[allow(clippy::too_many_lines)] // extraction logic; splitting would obscure the registry mapping
fn extract_registries(
    services: &Arc<ServiceRegistry>,
) -> (
    ModeRegistry,
    CommandRegistry,
    KeymapRegistry,
    ResolverRegistry,
    HashMap<reovim_kernel::api::v1::CommandId, Arc<dyn reovim_driver_command::CommandHandler>>,
) {
    // 1. Modes: ModeInfoStore → ModeRegistry
    //    ModeEntry::from_fields() converts driver-side ModeInfo to server-side ModeEntry
    let mut mode_registry = ModeRegistry::new();
    if let Some(store) = services.get::<ModeInfoStore>() {
        for info in store.take_modes() {
            mode_registry.register(ModeEntry::from_fields(
                info.id,
                info.display_name,
                info.cursor_style,
                info.accepts_char_input,
                info.has_selection,
                info.inherits_from,
                info.is_entry,
            ));
        }
    }
    tracing::info!(count = mode_registry.len(), "Extracted modes");

    // 2. Commands: CommandHandlerStore → CommandRegistry
    //    Arc<dyn CommandHandler> is shared; we also keep a handlers map for
    //    CommandHandlerExecutor used by the domain driver (#753 E6).
    let mut command_registry = CommandRegistry::new();
    let mut command_handlers: HashMap<
        reovim_kernel::api::v1::CommandId,
        Arc<dyn reovim_driver_command::CommandHandler>,
    > = HashMap::new();
    if let Some(store) = services.get::<CommandHandlerStore>() {
        for handler in store.take_handlers() {
            command_handlers.insert(handler.id(), Arc::clone(&handler));
            command_registry.register(handler);
        }
    }
    tracing::info!(count = command_registry.len(), "Extracted commands");

    // 3. Keybindings: KeybindingStore → KeymapRegistry
    //    Each KeybindingRegistration declares modes as "module:name" strings
    //    (e.g., "vim:normal"). We resolve these to ModeId via mode_registry.
    //    KeySequence (notation tokens) are converted to InputSequence (opaque)
    //    via the TUI codec in DefaultInputCodecRegistry.
    let mut keymap_registry = KeymapRegistry::new();
    // #620: Extract lookup policy from ServiceRegistry (registered by VimModule during init).
    // Falls back to EagerLookupPolicy if no module registered a policy.
    if let Some(policy_store) = services.get::<LookupPolicyStore>() {
        if let Some(driver_policy) = policy_store.take() {
            keymap_registry.set_default_policy(Arc::new(KeyPolicyAsLookupPolicy(driver_policy)));
        } else {
            keymap_registry.set_default_policy(Arc::new(EagerLookupPolicy));
        }
    } else {
        keymap_registry.set_default_policy(Arc::new(EagerLookupPolicy));
    }
    // #700: Retrieve leader key provider for <leader> expansion before parsing.
    let leader_provider = services.get::<LeaderKeyProvider>();

    // Obtain (or lazily create) the input codec registry so we can convert
    // KeySequence → InputSequence. Modules register TUI codecs here during init.
    let codec_registry = services.get_or_create::<DefaultInputCodecRegistry>();

    if let Some(store) = services.get::<KeybindingStore>() {
        let mut wired = 0usize;
        for binding in store.take_keybindings() {
            if !binding.enabled {
                continue;
            }
            // #700: Expand <leader> tokens before KeySequence::parse().
            let expanded_keys = match leader_provider {
                Some(ref provider) => provider.expand(binding.keys),
                None => binding.keys.to_owned(),
            };
            let Some(keys) = KeySequence::parse(&expanded_keys) else {
                tracing::warn!(keys = binding.keys, "Failed to parse keybinding");
                continue;
            };

            // Convert notation-token KeySequence to opaque InputSequence for the registry.
            let input_seq = match key_sequence_to_input_sequence(&keys, codec_registry.as_ref()) {
                Ok(seq) => seq,
                Err(e) => {
                    tracing::warn!(
                        keys = binding.keys,
                        error = %e,
                        "Failed to encode key sequence to InputSequence, skipping"
                    );
                    continue;
                }
            };

            for mode_str in binding.modes {
                if let Some(mode_id) = resolve_mode_str(mode_str, &mode_registry) {
                    keymap_registry.register_at_layer(
                        BindingLayer::Policy,
                        mode_id,
                        input_seq.clone(),
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

    (
        mode_registry,
        command_registry,
        keymap_registry,
        resolver_registry,
        command_handlers,
    )
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

/// Build a `TextDomainDriver` for domain-neutral dispatch (#753 E1).
///
/// Constructs the driver from registries already in `session_state` and the
/// `TextBufferRegistry` from `services`. The driver is wired with a resolver
/// dispatch provider so it can handle key dispatch when activated.
///
/// The driver is returned as `Some(Arc<dyn DomainDriver>)`. The server wires
/// it into the default session for dispatch and state queries.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_text_domain_driver(
    session_state: &SessionState,
    services: &Arc<ServiceRegistry>,
    resolver_registry: reovim_driver_text_input::ResolverRegistry,
    command_handlers: HashMap<
        reovim_kernel::api::v1::CommandId,
        Arc<dyn reovim_driver_command::CommandHandler>,
    >,
) -> Option<Arc<dyn reovim_subsys_session::DomainDriver>> {
    use {
        reovim_driver_text_input::ResolverDispatchProvider,
        reovim_driver_text_session::TextDomainDriver,
    };

    // Get TextBufferRegistry from services (registered earlier in bootstrap).
    let text_buffers = services.get::<reovim_provider_text::TextBufferRegistry>()?;

    // Wrap the command handlers map in the composition-root executor adapter.
    let command_executor: Arc<dyn reovim_driver_text_session::api::CommandExecutor> =
        Arc::new(CommandHandlerExecutor(command_handlers));

    // Clone the kernel context from session_state (KernelContext: Clone).
    let kernel = session_state.app.kernel.clone();

    // Clone the home mode from session_state.
    let home_mode = session_state.home_mode().clone();

    // Get (or lazily create) the input codec registry for the keymap adapter.
    let codec_registry = services.get_or_create::<DefaultInputCodecRegistry>();

    // Wrap the server's KeymapRegistry with a driver-tier adapter so
    // ResolverDispatchProvider can query it via the string-token KeymapQuery trait.
    let keymap_arc: Arc<dyn DriverKeymapQuery> = Arc::new(KeymapRegistryAdapter {
        inner: session_state.keymap_registry.clone(),
        codec_registry,
    });

    // Construct TextDomainDriver with domain_id=1 (canonical text domain).
    let mut driver =
        TextDomainDriver::new(1, home_mode, Arc::new(kernel), command_executor, text_buffers);

    // Wire dispatch provider.
    let provider = Arc::new(ResolverDispatchProvider::new(resolver_registry, keymap_arc));
    driver.set_dispatch_provider(provider);

    tracing::info!("TextDomainDriver constructed for default session (#753 E1)");

    Some(Arc::new(driver) as Arc<dyn reovim_subsys_session::DomainDriver>)
}

/// Trigger empty session handlers to create initial buffer.
///
/// If no buffers exist after module initialization, call registered
/// `EmptySessionHandler`s to create a scratch buffer.
#[cfg_attr(coverage_nightly, coverage(off))]
fn trigger_empty_session_handlers(state: &SessionState, services: &Arc<ServiceRegistry>) {
    use reovim_driver_text_session::{
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
            // Register the buffer in both the kernel BufferManager (for ID
            // assignment) and the TextBufferRegistry (for text-layer access).
            let Some(text_registry) = services.get::<reovim_provider_text::TextBufferRegistry>()
            else {
                tracing::warn!("TextBufferRegistry not found; skipping scratch buffer creation");
                return;
            };
            // Step 1: Register in TextBufferRegistry as `dyn BufferOps`; this
            //         assigns the canonical BufferId from the buffer's own ID.
            let buffer = reovim_provider_text::Buffer::from_string(&content);
            let buf_id = buffer.id();
            let text_arc: Arc<reovim_kernel::api::v1::RwLock<dyn reovim_provider_text::BufferOps>> =
                Arc::new(reovim_kernel::api::v1::RwLock::new(buffer));
            let id = text_registry.register(text_arc);
            // Step 2: Register in kernel BufferManager as `dyn KernelBuffer` using
            //         the same BufferId (via Buffer::with_id). An empty-content buffer
            //         is sufficient for the kernel — text content is in TextBufferRegistry.
            let kernel_buf = reovim_provider_text::Buffer::with_id(buf_id);
            let kernel_arc: Arc<
                reovim_kernel::api::v1::RwLock<dyn reovim_kernel::api::KernelBuffer>,
            > = Arc::new(reovim_kernel::api::v1::RwLock::new(kernel_buf));
            state.app.kernel.buffers.register(kernel_arc);
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
        Arc::new(reovim_driver_text_buffer::TestBufferManager::new()),
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

/// Initialize enabled modules in dependency-resolved order (#582, #586).
///
/// Collects default + extra modules (filtered by user config), resolves
/// dependencies via Kahn's topological sort, and initializes in dependency
/// order. Returns tracked modules for the `on_all_loaded()` lifecycle hook.
///
/// Modules self-register their services during `init()`:
/// - Resolvers → `ResolverRegistry`
/// - Commands → `CommandHandlerStore`
/// - Keybindings → `KeybindingStore`
/// - Mode info → `ModeInfoStore`
// Length justification: this function is the single orchestration point for
// builtin + external + registry module loading and dependency resolution.
// Splitting it further would obscure the 1:1 correspondence between phases
// (#582 depgraph, #587 externals, #725 registry + unified init).
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn initialize_modules(config: &ModulesConfig, ctx: &ModuleContext) -> Vec<TrackedModule> {
    initialize_modules_with_dependents(config, ctx).tracked
}

#[allow(clippy::too_many_lines)] // module init fan-out; splitting would scatter the registry dispatch
fn initialize_modules_with_dependents(
    config: &ModulesConfig,
    ctx: &ModuleContext,
) -> InitializedModules {
    // #620: Create builtin modules from static factory map (or empty for dynamic path)
    #[cfg(feature = "static-modules")]
    let all_modules: Vec<Box<dyn Module>> = {
        let manifest = parse_builtin_manifest();
        let registry = static_modules::builtin_registry();
        manifest
            .module_ids()
            .into_iter()
            .filter(|id| config.is_module_enabled(id))
            .filter_map(|id| registry.get(id).map(|factory| factory()))
            .collect()
    };

    #[cfg(not(feature = "static-modules"))]
    let all_modules: Vec<Box<dyn Module>> = Vec::new();

    // Convert Box<dyn Module> → ModuleHandle for unified static/dynamic interface (#620)
    let all_handles: Vec<ModuleHandle> = all_modules
        .into_iter()
        .map(ModuleHandle::from_boxed)
        .collect();

    // Collect builtin IDs before external discovery (for dedup)
    let builtin_ids: Vec<ModuleId> = all_handles.iter().map(|h| h.id().clone()).collect();

    // Discover and load external .so modules (#587)
    let mut external = discover_and_load_externals(config, &builtin_ids);

    // Load registry-installed modules (#725) into the same loader so they
    // participate in dependency resolution + unified init alongside builtins
    // and filesystem-discovered externals.
    load_registry_modules(
        &mut external,
        config,
        &builtin_ids,
        &reovim_subsys_module_registry::workflow::RegistryPaths::default_paths(),
    );

    tracing::info!(builtin = all_handles.len(), external = external.len(), "Initializing modules");

    // Save hardcoded order for shadow-mode comparison (reuse builtin_ids)
    let hardcoded_order = builtin_ids;

    // Build dependency entries from module handles
    let mut entries: Vec<DepEntry<ModuleId>> = all_handles
        .iter()
        .map(|h| DepEntry {
            key: h.id().clone(),
            required: h.dependencies(),
            optional: h.optional_dependencies(),
            provides_caps: h.provides().to_vec(),
            requires_caps: h.requires().to_vec(),
        })
        .collect();

    // Add dependency entries from external modules (#587)
    {
        let ext_ids: Vec<ModuleId> = external.loaded_ids().cloned().collect();
        for id in &ext_ids {
            if let Some(handle) = external.get(id) {
                entries.push(DepEntry {
                    key: handle.id().clone(),
                    required: handle.dependencies(),
                    optional: handle.optional_dependencies(),
                    provides_caps: handle.provides().to_vec(),
                    requires_caps: handle.requires().to_vec(),
                });
            }
        }
    }

    // Resolve dependency order via Kahn's topological sort
    let dep_order = match resolve_dependencies(&entries) {
        Ok(order) => {
            tracing::info!(count = order.order.len(), "Resolved module dependency order");
            order
        }
        Err(e) => {
            // TRANSITION SAFETY: Fallback to input order. This is UNSAFE for
            // Tier 4 modules (vim-snippet, vim-range-finder, snippet,
            // range-finder) which call .expect() during init and will panic
            // if their deps aren't init'd first. Remove this fallback after
            // integration tests verify toposort (#582).
            tracing::error!(%e, "Dependency resolution failed, using input order");
            DependencyOrder {
                order: hardcoded_order.clone(),
                dependents: HashMap::new(),
            }
        }
    };

    // Shadow-mode: compare relative ordering of dependent pairs (#582)
    log_shadow_comparison(&hardcoded_order, &dep_order.order, &entries);

    // Check version constraints (#619)
    check_module_version_constraints(&all_handles, &external);

    // Index handles by ID for O(1) lookup during ordered init
    let mut module_map: HashMap<ModuleId, ModuleHandle> = all_handles
        .into_iter()
        .map(|h| {
            let id = h.id().clone();
            (id, h)
        })
        .collect();

    // Capture external count BEFORE the take-loop drains the loader, so the
    // post-init trace still reports the number of external modules that
    // participated in initialization (flight-director S1).
    let external_count_before_init = external.len();

    // Initialize in dependency order. External module handles are `take()`n
    // out of the loader and joined into the unified `tracked` list so they
    // go through the same init path as builtins (#725 Phase 3).
    let mut tracked = Vec::with_capacity(dep_order.order.len());
    for id in &dep_order.order {
        let handle = if let Some(handle) = module_map.remove(id) {
            handle
        } else if let Some(handle) = external.take(id) {
            handle
        } else {
            continue;
        };

        let mut tm = TrackedModule {
            handle,
            state: ModuleState::Loaded,
        };
        tm.state = ModuleState::Initializing;
        let success = init_single_handle(&mut tm.handle, ctx);
        tm.state = if success {
            ModuleState::Running
        } else {
            ModuleState::Failed("init returned non-success".into())
        };
        tracked.push(tm);
    }

    let running = tracked
        .iter()
        .filter(|t| t.state == ModuleState::Running)
        .count();
    tracing::info!(
        total = tracked.len(),
        running,
        external = external_count_before_init,
        "Module initialization complete"
    );

    InitializedModules {
        tracked,
        dependents: dep_order.dependents,
    }
}

fn collect_bridges_from_services(
    services: &Arc<ServiceRegistry>,
    tracked: &[TrackedModule],
) -> reovim_driver_text_session::bridges::BridgeRegistry {
    use reovim_driver_text_session::bridges::BridgeRegistry;

    #[cfg(feature = "static-modules")]
    {
        use reovim_driver_text_session::bridges::BridgeProvider;

        let mut registry = BridgeRegistry::new();
        if let Some(provider) = services.get::<BridgeProvider>() {
            for bridge in provider.take_bridges() {
                registry.register_boxed(bridge);
            }
        }

        let available_kinds = collect_available_kinds(tracked);
        validate_extension_contracts(tracked, &registry);
        registry.set_available_kinds(available_kinds);
        registry
    }

    #[cfg(not(feature = "static-modules"))]
    {
        let _ = tracked;
        tracing::debug!("Static modules disabled, bridge collection unavailable");
        BridgeRegistry::new()
    }
}

fn build_live_module_registry(initialized: InitializedModules) -> Arc<ModuleRegistry> {
    let InitializedModules {
        tracked,
        dependents,
    } = initialized;
    let init_order: Vec<ModuleId> = tracked.iter().map(|tm| tm.handle.id().clone()).collect();
    let mut loader = ModuleLoader::new();
    let mut states = HashMap::new();

    for tracked_module in tracked {
        let id = tracked_module.handle.id().clone();
        states.insert(id, tracked_module.state.clone());
        loader
            .insert_handle(tracked_module.handle)
            .expect("bootstrap module IDs must be unique");
    }

    ModuleRegistry::from_loader(loader, states, init_order, dependents).into_arc()
}

/// Initialize a single module handle, logging the result.
///
/// Works with both static and dynamic modules via `ModuleHandle`.
/// Returns `true` if the module initialized successfully.
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_single_handle(handle: &mut ModuleHandle, ctx: &ModuleContext) -> bool {
    let id = handle.id().clone();
    let name = handle.name().to_owned();

    tracing::debug!(%id, name, "Initializing module");

    match handle.init(ctx) {
        Ok(reovim_subsys_module_loader::handle::InitResult::Success) => {
            tracing::info!(%id, name, "Module initialized successfully");
            true
        }
        Ok(reovim_subsys_module_loader::handle::InitResult::Defer(msg)) => {
            tracing::warn!(%id, name, %msg, "Module deferred initialization");
            false
        }
        Err(err) => {
            tracing::error!(%id, name, ?err, "Module initialization failed");
            false
        }
    }
}

/// Call `on_all_loaded()` on all running modules (#582).
///
/// Invoked after all modules have completed `init()`. Currently a no-op
/// for all modules (none override this hook), but wires the mechanism
/// so future modules can use it for cross-module queries.
#[cfg_attr(coverage_nightly, coverage(off))]
fn call_on_all_loaded(modules: &mut [TrackedModule], ctx: &ModuleContext) {
    // Since #725, `tracked` contains BOTH builtin and external modules in
    // dependency order, so a single pass dispatches `on_all_loaded` to
    // everything. External modules dispatch through the new
    // `reovim_module_on_all_loaded` FFI trampoline.
    for tm in modules.iter_mut() {
        if tm.state == ModuleState::Running {
            tm.handle.on_all_loaded(ctx);
        }
    }
    tracing::info!(count = modules.len(), "on_all_loaded complete");
}

/// Discover and load external `.so` modules from search paths (#587).
///
/// Scans default search paths (`$REOVIM_MODULE_PATH`, XDG dirs, system dirs)
/// for module shared libraries, probes each one for metadata and API version
/// compatibility, and loads enabled modules.
///
/// External modules that duplicate a builtin module ID are skipped — builtins
/// always take priority. Modules disabled by user config are also skipped.
///
/// This function never fails — individual module load failures are logged
/// as warnings but don't prevent other modules from loading.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unsafe_code)]
fn discover_and_load_externals(config: &ModulesConfig, builtin_ids: &[ModuleId]) -> ModuleLoader {
    let mut loader = ModuleLoader::new();

    let discovered = loader.discover();
    if discovered.is_empty() {
        return loader;
    }

    tracing::info!(count = discovered.len(), "Discovered external module files");

    for path in &discovered {
        // SAFETY: We trust .so files found in system search paths (XDG user dir,
        // /usr/local/lib, /usr/lib). Users control which paths are searched via
        // the REOVIM_MODULE_PATH environment variable.
        match unsafe { loader.load_dynamic(path) } {
            Ok(id) => {
                // Builtins always take priority — skip external duplicates
                if builtin_ids.iter().any(|b| b == &id) {
                    tracing::debug!(
                        %id,
                        "External module duplicates builtin, skipping"
                    );
                    loader.unload(&id).ok();
                } else if config.is_module_enabled(id.as_str()) {
                    tracing::info!(%id, ?path, "Loaded external module");
                } else {
                    tracing::info!(
                        %id,
                        "External module disabled by user config, unloading"
                    );
                    loader.unload(&id).ok();
                }
            }
            Err(e) => {
                tracing::warn!(?path, %e, "Failed to load external module, skipping");
            }
        }
    }

    let ext_count = loader.len();
    if ext_count > 0 {
        tracing::info!(count = ext_count, "External modules loaded");
    }

    loader
}

/// Load registry-installed modules into an existing loader (#725).
///
/// Reads `installed.json` via `module-registry::workflow::list`, applies the
/// same filters as [`discover_and_load_externals`] (builtin-conflict,
/// disabled-kind, missing/nonexistent library), and loads each enabled
/// module via `loader.load_dynamic`. Registry failures are non-fatal —
/// missing registry means "no installed modules," not an error.
///
/// The `paths` argument is injected so tests can point at a tempdir
/// `RegistryPaths` instead of the user's real install location (#725
/// countdown addendum Phase 2 test strategy).
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unsafe_code)]
fn load_registry_modules(
    loader: &mut ModuleLoader,
    config: &ModulesConfig,
    builtin_ids: &[ModuleId],
    paths: &reovim_subsys_module_registry::workflow::RegistryPaths,
) {
    use reovim_subsys_module_registry::workflow;

    let installed = match workflow::list(paths) {
        Ok(modules) => modules,
        Err(e) => {
            // Not fatal — registry might not exist yet.
            tracing::debug!(%e, "Could not read module registry, skipping");
            return;
        }
    };

    if installed.is_empty() {
        return;
    }

    tracing::info!(count = installed.len(), "Found registry-installed modules");

    for entry in &installed {
        let id_str = entry.id.as_str();

        // Builtin-wins: check BEFORE dlopen to avoid wasted work.
        if builtin_ids.iter().any(|b| b.as_str() == id_str) {
            tracing::debug!(
                module = %id_str,
                "Registry module duplicates builtin, skipping"
            );
            continue;
        }

        // Disabled-in-config check — skip before dlopen.
        if !config.is_module_enabled(id_str) {
            tracing::info!(
                module = %id_str,
                "Registry module disabled by user config, skipping"
            );
            continue;
        }

        // Must have a built library path.
        let Some(ref lib_path) = entry.library_path else {
            tracing::warn!(
                module = %id_str,
                "Registry module has no compiled library, skipping"
            );
            continue;
        };

        if !lib_path.exists() {
            tracing::warn!(
                module = %id_str,
                path = %lib_path.display(),
                "Registry module library not found on disk, skipping"
            );
            continue;
        }

        // SAFETY: We trust registry-installed `.so` files because the user
        // explicitly ran `reovim module install <source>` to place them.
        // Same trust model as `discover_and_load_externals` for files under
        // `$REOVIM_MODULE_PATH` and XDG directories.
        match unsafe { loader.load_dynamic(lib_path) } {
            Ok(loaded_id) => {
                tracing::info!(
                    module = %loaded_id,
                    path = %lib_path.display(),
                    "Loaded registry-installed module"
                );
            }
            Err(e) => {
                tracing::warn!(
                    module = %id_str,
                    %e,
                    "Failed to load registry-installed module, skipping"
                );
            }
        }
    }
}

/// Compare relative ordering of dependent pairs between hardcoded and
/// Check version constraints across all modules (#619).
///
/// Collects `version_constraints()` from each module and validates them
/// against actual module versions. Violations are logged as warnings.
/// Non-fatal — modules still load but constraints are surfaced.
// Runtime-only validation with tracing — not unit testable.
#[cfg_attr(coverage_nightly, coverage(off))]
fn check_module_version_constraints(builtins: &[ModuleHandle], external: &ModuleLoader) {
    // Build version map: module_id -> (major, minor, patch)
    let mut versions: Vec<(ModuleId, (u32, u32, u32))> = builtins
        .iter()
        .map(|h| {
            let v = h.version();
            (h.id().clone(), (v.major, v.minor, v.patch))
        })
        .collect();

    let ext_ids: Vec<ModuleId> = external.loaded_ids().cloned().collect();
    for id in &ext_ids {
        if let Some(handle) = external.get(id) {
            let v = handle.version();
            versions.push((handle.id().clone(), (v.major, v.minor, v.patch)));
        }
    }

    // Collect all constraints: (source_id, target_id, range_str)
    let mut constraints: Vec<(ModuleId, ModuleId, &str)> = Vec::new();
    for h in builtins {
        for (target, range_str) in h.version_constraints() {
            constraints.push((h.id().clone(), target, range_str));
        }
    }

    if constraints.is_empty() {
        return;
    }

    let violations = check_version_constraints(&constraints, &versions);
    for v in &violations {
        tracing::warn!(
            source = %v.source,
            target = %v.target,
            required = %v.required,
            actual = format_args!("{}.{}.{}", v.actual.0, v.actual.1, v.actual.2),
            "Version constraint violation"
        );
    }

    if !violations.is_empty() {
        tracing::warn!(
            count = violations.len(),
            "Version constraint violations detected — modules may not work correctly"
        );
    }
}

/// computed orderings (#582 shadow-mode).
///
/// Only logs violations — does NOT compare absolute positions (which
/// produces false positives from valid toposort reorderings of independent
/// modules). Will be removed after transition is verified.
#[cfg_attr(coverage_nightly, coverage(off))]
fn log_shadow_comparison(
    hardcoded: &[ModuleId],
    computed: &[ModuleId],
    entries: &[DepEntry<ModuleId>],
) {
    let hardcoded_pos: HashMap<&ModuleId, usize> = hardcoded
        .iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    let computed_pos: HashMap<&ModuleId, usize> =
        computed.iter().enumerate().map(|(i, id)| (id, i)).collect();

    for entry in entries {
        for dep in &entry.required {
            if let (Some(&h_dep), Some(&h_mod), Some(&c_dep), Some(&c_mod)) = (
                hardcoded_pos.get(dep),
                hardcoded_pos.get(&entry.key),
                computed_pos.get(dep),
                computed_pos.get(&entry.key),
            ) {
                if c_dep >= c_mod {
                    tracing::error!(
                        dep = %dep,
                        module = %entry.key,
                        "ORDERING BUG: dep not before module in computed order"
                    );
                }
                if h_dep < h_mod && c_dep >= c_mod {
                    tracing::warn!(
                        dep = %dep,
                        module = %entry.key,
                        "Regression: hardcoded order was correct, computed is wrong"
                    );
                }
            }
        }
    }
}

/// Collect the union of all `extension_kinds()` from running modules (#584).
///
/// Returns a sorted, deduplicated list of extension kind identifiers
/// declared by loaded server modules. Used to populate `BridgeRegistry`
/// and expose to clients via the `ListExtensions` RPC.
#[cfg(feature = "static-modules")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn collect_available_kinds(modules: &[TrackedModule]) -> Vec<&'static str> {
    use std::collections::BTreeSet;

    let kinds: BTreeSet<&'static str> = modules
        .iter()
        .filter(|tm| tm.state == ModuleState::Running)
        .flat_map(|tm| tm.handle.extension_kinds().iter().copied())
        .collect();

    kinds.into_iter().collect()
}

/// Validate that bridge registry kinds match module `extension_kinds()` declarations (#584).
///
/// Logs warnings for:
/// - Orphaned bridges: bridge kind registered but no module declares it
/// - Orphaned module kinds: module declares a kind but no bridge matches
///
/// This is non-fatal — the system continues with graceful degradation.
#[cfg(feature = "static-modules")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_extension_contracts(
    modules: &[TrackedModule],
    bridge_registry: &reovim_driver_text_session::bridges::BridgeRegistry,
) {
    use std::collections::HashSet;

    let module_kinds: HashSet<&str> = modules
        .iter()
        .filter(|tm| tm.state == ModuleState::Running)
        .flat_map(|tm| tm.handle.extension_kinds().iter().copied())
        .collect();

    let bridge_kinds: HashSet<&str> = bridge_registry.kinds().into_iter().collect();

    for kind in &bridge_kinds {
        if !module_kinds.contains(kind) {
            tracing::warn!(
                kind,
                "Orphaned bridge: registered but no module declares this extension_kind"
            );
        }
    }

    for kind in &module_kinds {
        if !bridge_kinds.contains(kind) {
            tracing::warn!(
                kind,
                "Orphaned module kind: declared in extension_kinds() but no bridge registered"
            );
        }
    }

    let matched = module_kinds.intersection(&bridge_kinds).count();
    tracing::info!(
        matched,
        module_kinds = module_kinds.len(),
        bridge_kinds = bridge_kinds.len(),
        "Extension contract validation complete"
    );
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

    // Re-add factories to the store so runtime consumers (e.g., microscope
    // preview highlighting) can still look up drivers via `find()`.
    for factory in &factories {
        store.add(Arc::clone(factory));
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
/// Uses `ConfigPaths::data_dir()` with `/modules/` subdirectory.
/// Respects `$REOVIM_DATA_DIR` override (#610).
#[cfg_attr(coverage_nightly, coverage(off))]
fn default_data_dir() -> std::path::PathBuf {
    ConfigPaths::data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from(".").join("reovim"))
        .join("modules")
}

/// Get the default cache directory for modules.
///
/// Uses `ConfigPaths::cache_dir()` with `/modules/` subdirectory.
/// Respects `$REOVIM_CACHE_DIR` override (#610).
#[cfg_attr(coverage_nightly, coverage(off))]
fn default_cache_dir() -> std::path::PathBuf {
    ConfigPaths::cache_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from(".").join("reovim"))
        .join("modules")
}

/// Build and register a `ModuleLoadReport` for health-check diagnostics (#610).
///
/// Populates the report from tracked module states and user config,
/// then registers it in `ServiceRegistry` for `collect_modules()` etc.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_and_register_load_report(
    config: &ModulesConfig,
    tracked: &[TrackedModule],
    services: &Arc<ServiceRegistry>,
) {
    use reovim_subsys_module_loader::report::ModuleLoadReport;

    let mut report = ModuleLoadReport::new();

    for tm in tracked {
        let id = tm.handle.id().clone();
        match &tm.state {
            ModuleState::Running => report.loaded.push(id),
            ModuleState::Failed(reason) => report.failed.push((id, reason.clone())),
            _ => {}
        }
    }

    // Record disabled modules from config
    for name in config.disabled_modules() {
        report
            .disabled
            .push(ModuleId::from_string(name.to_string()));
    }

    // Check required deps: warn if a loaded module's required dep is not loaded
    let loaded_ids: Vec<&str> = report.loaded.iter().map(ModuleId::as_str).collect();
    for tm in tracked {
        if tm.state != ModuleState::Running {
            continue;
        }
        for dep in tm.handle.dependencies() {
            if !loaded_ids.contains(&dep.as_str()) {
                report.missing_deps.push((tm.handle.id().clone(), dep));
            }
        }
    }

    // Record config path
    if let Ok(config_dir) = ConfigPaths::config_dir() {
        let config_path = config_dir.join("modules.toml");
        if config_path.exists() {
            report.config_path = Some(config_path);
        }
    }

    // Record module search paths
    report.search_paths = reovim_subsys_module_loader::discovery::default_search_paths();

    // Record isolation status
    report.isolation_active =
        std::env::var("REOVIM_DATA_DIR").is_ok() || std::env::var("REOVIM_CONFIG_DIR").is_ok();

    services.register(Arc::new(report));
}

/// Check `modules.lock` staleness at startup (#587).
///
/// Warns if the lock file was generated by a different reovim version.
/// Missing lock file is normal (first run or no external modules) and
/// is silently ignored.
#[cfg_attr(coverage_nightly, coverage(off))]
fn check_lockfile_staleness() {
    use reovim_subsys_module_loader::lockfile::ModulesLock;

    // Lock file lives alongside module data: ~/.local/share/reovim/modules.lock
    let lock_path = default_data_dir()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default()
        .join("modules.lock");

    match ModulesLock::load(&lock_path) {
        Ok(lock) => {
            let current_version = env!("CARGO_PKG_VERSION");
            if lock.is_stale(current_version) {
                tracing::warn!(
                    lock_version = %lock.meta.reovim_version,
                    current_version,
                    "modules.lock is stale — run `reovim module resolve` to regenerate"
                );
            } else {
                tracing::debug!("modules.lock is up to date");
            }
        }
        Err(_) => {
            // Missing lock file is expected on first run or when no
            // external modules are installed.
            tracing::debug!("No modules.lock found (normal for first run)");
        }
    }
}

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
