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
    reovim_driver_display::{
        GutterRenderer, GutterRendererKey, GutterRendererRegistry, LineNumberMode,
    },
    reovim_driver_input::{
        KeybindingStore, ModeInfo, ModeInfoStore, ModeProviderKey, ModeProviderRegistry,
        ResolverRegistry,
    },
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, OptionConstraint,
        OptionScope, OptionSpec, OptionValue, ProbeResult, Version, pr_info,
    },
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
mod registry_integration;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod session_state_tests;
#[cfg(test)]
mod ids_tests;
#[cfg(test)]
mod macros_tests;
#[cfg(test)]
mod modes_tests;
#[cfg(test)]
mod vim_lookup_policy_tests;
#[cfg(test)]
mod providers_tests;

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
    EnterInsertFirstNonBlank, EnterInsertMode, EnterInsertModeAppend, EnterSearchBackward,
    EnterSearchForward, EnterWindowMode, ExecuteFindChar, ExitCommandLineMode, ExitToNormal,
    OpenLineAbove, OpenLineBelow,
};

// Re-export resolvers
pub use resolvers::{
    VimChangeResolver, VimCommandLineResolver, VimDeleteResolver, VimInsertResolver,
    VimNormalResolver, VimVisualResolver, VimYankResolver,
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
    // Manipulation
    ReselectLast,
    SwapAnchor,
    ToggleVisualBlock,
    ToggleVisualChar,
    ToggleVisualLine,
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

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
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

        // Epic #417 Part 3: Self-register commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

        // Epic #417 Part 3: Self-register keybindings
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // Epic #445: Register line number options
        if let Err(e) = ctx.kernel.options.register(
            OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
                .with_short("nu")
                .with_scope(OptionScope::Window)
                .with_owner(VIM_MODULE),
        ) {
            return ProbeResult::Failed(ModuleError::InitFailed(format!(
                "Failed to register 'number' option: {e}"
            )));
        }
        if let Err(e) = ctx.kernel.options.register(
            OptionSpec::new(
                "relativenumber",
                "Show relative line numbers",
                OptionValue::bool(false),
            )
            .with_short("rnu")
            .with_scope(OptionScope::Window)
            .with_owner(VIM_MODULE),
        ) {
            return ProbeResult::Failed(ModuleError::InitFailed(format!(
                "Failed to register 'relativenumber' option: {e}"
            )));
        }

        // Epic #570: Register vim behavior options (#573)
        for spec in vim_option_specs() {
            if let Err(e) = ctx.kernel.options.register(spec) {
                return ProbeResult::Failed(ModuleError::InitFailed(format!(
                    "Failed to register vim option: {e}"
                )));
            }
        }

        // Epic #458: Register GutterRenderer with LineNumberSource and LineNumberPresenter
        // Mode is dynamic - AnnotationContext will carry the actual mode from options
        let mut gutter_renderer = GutterRenderer::new();
        gutter_renderer
            .register_source(annotation::create_line_number_source(LineNumberMode::Absolute));
        gutter_renderer.register_presenter(annotation::create_line_number_presenter());

        tracing::info!(
            sources = gutter_renderer.source_count(),
            presenters = gutter_renderer.presenter_count(),
            "VimModule: created GutterRenderer"
        );

        let renderer_registry = ctx.services.get_or_create::<GutterRendererRegistry>();
        renderer_registry.register(GutterRendererKey::Default, Arc::new(gutter_renderer));

        tracing::info!("VimModule: registered GutterRenderer in ServiceRegistry");

        pr_info!("Vim module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Vim module exiting");
        Ok(())
    }

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
// Option specifications (#573)
// ============================================================================

/// Vim behavior option specifications.
///
/// These are standard vim options for search, scroll, and display behavior.
/// Registered during `VimModule::init()`.
fn vim_option_specs() -> Vec<OptionSpec> {
    vec![
        OptionSpec::new("scrolloff", "Minimum lines above/below cursor", OptionValue::int(0))
            .with_short("so")
            .with_constraint(OptionConstraint::min(0))
            .with_owner(VIM_MODULE),
        OptionSpec::new(
            "sidescrolloff",
            "Minimum columns left/right of cursor",
            OptionValue::int(0),
        )
        .with_short("siso")
        .with_constraint(OptionConstraint::min(0))
        .with_owner(VIM_MODULE),
        OptionSpec::new("ignorecase", "Ignore case in search patterns", OptionValue::bool(false))
            .with_short("ic")
            .with_owner(VIM_MODULE),
        OptionSpec::new(
            "smartcase",
            "Override ignorecase if pattern has uppercase",
            OptionValue::bool(false),
        )
        .with_short("scs")
        .with_owner(VIM_MODULE),
        OptionSpec::new("hlsearch", "Highlight search matches", OptionValue::bool(false))
            .with_short("hls")
            .with_owner(VIM_MODULE),
        OptionSpec::new("incsearch", "Show search matches incrementally", OptionValue::bool(false))
            .with_short("is")
            .with_owner(VIM_MODULE),
        OptionSpec::new("wrapscan", "Wrap search around end of file", OptionValue::bool(true))
            .with_short("ws")
            .with_owner(VIM_MODULE),
    ]
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimModule);

