#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim policy module.
//!
//! This module provides Vim-style behavior for reovim:
//! - **Keybindings**: Standard Vim keys (hjkl, operators, modes)
//! - **Operators**: Vim operators (d, y, c) - delete, yank, change
//! - **Resolvers**: Mode-specific key interpretation (counts, registers)
//! - **Visual mode**: Entry, exit, manipulation, operators
//! - **Policy**: Vim lookup behavior (wait for longer sequences)
//!
//! # Architecture
//!
//! This is a **POLICY** module - it defines HOW the editor behaves.
//! The kernel and drivers provide the mechanisms (WHAT can be done).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  VIM POLICY MODULE (this module)           POLICY       │
//! │  → Vim keybindings (hjkl, dd, etc.)                     │
//! │  → Vim operators (d, y, c with motions)                 │
//! │  → Vim resolvers (count/register handling)              │
//! │  → Visual mode commands (selection operations)          │
//! │  → Vim behavior (wait for longer sequences)             │
//! ├─────────────────────────────────────────────────────────┤
//! │  MECHANISM MODULES                         CAPABILITIES │
//! │  editor/, keymap/, motions/                             │
//! │  → "What operations are possible"                       │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_vim::VimModule;
//!
//! let module = VimModule::new();
//! let bindings = module.keybindings();
//! // Returns ~180 Vim keybindings
//! ```

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_input::{
        KeybindingStore, LookupPolicyStore, ModeInfo, ModeInfoStore, ModeProviderKey,
        ModeProviderRegistry, ResolverRegistry,
    },
    reovim_driver_session::{InitialModeProvider, LeaderKeyProvider, bridges::BridgeProvider},
    reovim_kernel::api::v1::{
        KeybindingRegistration, ModeId, Module, ModuleContext, ModuleError, ModuleId, ProbeResult,
        Version, pr_info,
    },
    reovim_subsys_annotation::{AnnotationSourceKey, AnnotationSourceRegistry, LineNumberMode},
    reovim_subsys_manifest::ModeBridgeStore,
};

pub mod annotation;
pub mod bindings;
pub mod commands;
pub mod fallback;
pub mod ids;
pub mod macros;
pub mod modes;
pub mod operators;
pub mod providers;
pub mod resolvers;
pub mod session_state;
pub mod vim_lookup_policy;
pub mod visual;

#[cfg(test)]
mod ids_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod macros_tests;
#[cfg(test)]
mod modes_tests;
#[cfg(test)]
mod providers_tests;
#[cfg(test)]
mod registry_integration;
#[cfg(test)]
mod session_state_tests;
#[cfg(test)]
mod vim_lookup_policy_tests;

// Re-export mode types (Epic #372 - Mode Ownership)
pub use modes::{VIM_MODULE, VimMode};

// Re-export provider types (Epic #415 - Module provider hooks)
pub use providers::{VimDefaultModeProvider, VimModuleProviderExt};

// Re-export session state (Epic #385 - Server Simplification)
pub use session_state::{PendingCharOp, VimSessionState};

// Re-export OperatorId (Epic #385 - operators are vim policy, not kernel mechanism)
pub use ids::{CHANGE, DELETE, OperatorId, YANK};

// Re-export operators (Epic #385 - operators are vim-specific, merged from operators module)
pub use operators::{
    ChangeCommand, ChangeOperator, DeleteCommand, DeleteOperator, Operator, OperatorContext,
    OperatorError, Range, YankCommand, YankOperator, operator_commands,
};

// Re-export fallback handler (Epic #372 - Mode Ownership)
pub use fallback::VimFallbackHandler;

// Re-export Vim lookup policy (#542 - mechanism/policy separation)
pub use vim_lookup_policy::VimLookupPolicy;

// Re-export mode commands (Epic #372 - Mode Ownership)
pub use commands::{
    ChangeLine, ChangeToEndOfLine, EnterCommandLineMode, EnterInsertEndOfLine,
    EnterInsertFirstNonBlank, EnterInsertMode, EnterInsertModeAppend, EnterReplaceMode,
    EnterSearchBackward, EnterSearchForward, EnterWindowMode, ExecuteFindChar, ExitCommandLineMode,
    ExitToNormal, OpenLineAbove, OpenLineBelow, ReplaceBackspace,
};

// Re-export resolvers
pub use resolvers::{
    VimCaseResolver, VimChangeResolver, VimCommandLineResolver, VimDeleteResolver,
    VimInsertResolver, VimNormalResolver, VimReplaceResolver, VimVisualResolver, VimYankResolver,
};

// Re-export visual mode commands
pub use visual::{
    // Operators
    ChangeSelection,
    DedentSelection,
    DeleteSelection,
    // Entry
    EnterVisualBlockMode,
    EnterVisualLineMode,
    EnterVisualMode,
    // Exit
    ExitVisualMode,
    IndentSelection,
    LowercaseSelection,
    // Manipulation
    ReselectLast,
    SwapAnchor,
    ToggleCaseSelection,
    ToggleVisualBlock,
    ToggleVisualChar,
    ToggleVisualLine,
    UppercaseSelection,
    YankSelection,
    // Helper functions
    visual_commands,
    visual_entry_commands,
    visual_exit_commands,
    visual_operator_commands,
    visual_selection_commands,
};

/// Vim policy module.
///
/// Provides standard Vim keybindings and behavior.
/// This module is stateless - all state is managed by the kernel.
pub struct VimModule;

impl VimModule {
    /// Create a new Vim module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for VimModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Module for VimModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("vim")
    }

    fn name(&self) -> &'static str {
        "Vim"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("editor"), ModuleId::new("motions")]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register YankFlashBridge via BridgeProvider (#657)
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(operators::yank_flash::YankFlashBridge);

        // Register default mode provider with typed key (Epic #417)
        let mode_registry = ctx.services.get_or_create::<ModeProviderRegistry>();
        mode_registry.register(ModeProviderKey::Entry, Arc::new(VimDefaultModeProvider::new()));

        // Epic #417 Part 3: Self-register modes
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in VimMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Epic #417 Part 3: Self-register resolvers
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolvers::VimNormalResolver::new());
        resolver_registry.register(resolvers::VimInsertResolver::new());
        resolver_registry.register(resolvers::VimDeleteResolver::new());
        resolver_registry.register(resolvers::VimYankResolver::new());
        resolver_registry.register(resolvers::VimChangeResolver::new());
        resolver_registry.register(resolvers::VimCommandLineResolver::new());
        // Epic #438: Window mode resolver
        resolver_registry.register(resolvers::VimWindowResolver::new());
        // #465 Bug fix: Visual mode resolvers (character, line, block)
        resolver_registry.register(resolvers::VimVisualResolver::character_wise());
        resolver_registry.register(resolvers::VimVisualResolver::line_wise());
        resolver_registry.register(resolvers::VimVisualResolver::block_wise());

        // Replace mode resolver (#666)
        resolver_registry.register(resolvers::VimReplaceResolver::new());

        // Case operator resolvers (#666)
        resolver_registry.register(resolvers::VimCaseResolver::new(
            resolvers::operator_common::OperatorType::Lowercase,
        ));
        resolver_registry.register(resolvers::VimCaseResolver::new(
            resolvers::operator_common::OperatorType::Uppercase,
        ));
        resolver_registry.register(resolvers::VimCaseResolver::new(
            resolvers::operator_common::OperatorType::ToggleCase,
        ));

        // #620: Self-register lookup policy (decouples bootstrap from vim import)
        let policy_store = ctx.services.get_or_create::<LookupPolicyStore>();
        policy_store.set(Arc::new(VimLookupPolicy));

        // #623: Register initial mode for cross-personality support
        let initial_mode_provider = ctx.services.get_or_create::<InitialModeProvider>();
        if let Some(prev) = initial_mode_provider.set(ModeId::new(VIM_MODULE, "normal")) {
            tracing::warn!(previous = %prev, "VimModule overrode existing initial mode");
        }

        // #700: Register leader key for personality-aware binding expansion
        let leader_provider = ctx.services.get_or_create::<LeaderKeyProvider>();
        if let Some(prev) = leader_provider.set("<Space>") {
            tracing::warn!(previous = prev, "VimModule overrode existing leader key");
        }

        // Epic #417 Part 3: Self-register commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

        // Epic #417 Part 3: Self-register keybindings
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // #585/#610: Load personality manifest (keybindings, mode bridges, options)
        if let Err(e) = load_personality_manifest(ctx, &keybinding_store) {
            return ProbeResult::Failed(e);
        }

        // Register annotation source (display-side creates presenters)
        let source_registry = ctx.services.get_or_create::<AnnotationSourceRegistry>();
        source_registry.register(
            AnnotationSourceKey::new("builtin.line_number"),
            annotation::create_line_number_source(LineNumberMode::Absolute),
        );

        tracing::info!("VimModule: registered LineNumberSource in AnnotationSourceRegistry");

        pr_info!("Vim module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Vim module exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::MODE_MANAGEMENT]
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[operators::yank_flash::KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        bindings::all()
    }
}

impl CommandProvider for VimModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let mut handlers = Vec::new();
        handlers.extend(commands::mode_commands());
        handlers.extend(visual::visual_commands());
        handlers.extend(operators::operator_commands()); // Epic #415
        handlers
    }
}

// ============================================================================
// Personality manifest loading (#585)
// ============================================================================

/// Embedded vim personality manifest.
const VIM_MANIFEST_TOML: &str = include_str!("../data/vim.toml");

/// Load the vim personality manifest and register its keybindings and mode bridges.
///
/// Keybindings from the manifest are registered for all declared bindings.
/// Command availability is validated at key-dispatch time (feature modules may
/// not be loaded yet when `VimModule` initializes). Mode bridges are stored in
/// `ModeBridgeStore` for feature modules to query during their `init()`.
///
/// Startup validation (#585 Phase 6):
/// - Detects key conflicts within the manifest (same key + same mode)
/// - Logs non-fatal warnings — does not fail initialization
#[cfg_attr(coverage_nightly, coverage(off))]
fn load_personality_manifest(
    ctx: &ModuleContext,
    keybinding_store: &KeybindingStore,
) -> Result<(), ModuleError> {
    let manifest = match reovim_subsys_manifest::PersonalityManifest::parse(VIM_MANIFEST_TOML) {
        Ok(m) => m,
        Err(e) => {
            return Err(ModuleError::InitFailed(format!(
                "Failed to parse vim.toml personality manifest: {e}"
            )));
        }
    };

    // #585 Phase 6: Startup validation — detect key conflicts within the manifest
    let conflicts = manifest.detect_conflicts();
    for warning in &conflicts {
        tracing::warn!("vim.toml: {warning}");
    }

    // Register manifest keybindings (all bindings — command availability is
    // validated at key-dispatch time, not at registration time, because
    // feature modules may not have loaded yet)
    let registrations = manifest.to_keybinding_registrations();
    let count = registrations.len();
    keybinding_store.add_all(registrations);
    tracing::info!(count, "VimModule: loaded personality manifest keybindings");

    // #610: Register manifest-driven options (before mode_bridges move)
    let option_specs = manifest.to_option_specs(&VIM_MODULE);

    // Populate ModeBridgeStore for feature modules
    let bridge_store = ModeBridgeStore::new(manifest.mode_bridges);
    ctx.services.register(Arc::new(bridge_store));
    tracing::info!("VimModule: registered ModeBridgeStore");

    // #610: Register options (replaces hardcoded vim_option_specs)
    for spec in option_specs {
        ctx.kernel.options.register(spec).map_err(|e| {
            ModuleError::InitFailed(format!("Failed to register manifest option: {e}"))
        })?;
    }

    Ok(())
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimModule);
