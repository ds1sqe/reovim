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
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, OptionScope,
        OptionSpec, OptionValue, ProbeResult, Version, pr_info,
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
pub mod visual;

#[cfg(test)]
mod registry_integration;

// Re-export mode types (Epic #372 - Mode Ownership)
pub use modes::{VIM_MODULE, VimMode};

// Re-export provider types (Epic #415 - Module provider hooks)
pub use providers::{VimDefaultModeProvider, VimModuleProviderExt};

// Re-export session state (Epic #385 - Server Simplification)
pub use session_state::{LastFind, PendingCharOp, VimSessionState};

// Re-export OperatorId (Epic #385 - operators are vim policy, not kernel mechanism)
pub use ids::{CHANGE, DELETE, OperatorId, YANK};

// Re-export operators (Epic #385 - operators are vim-specific, merged from operators module)
pub use operators::{
    ChangeCommand, ChangeOperator, DeleteCommand, DeleteOperator, Operator, OperatorContext,
    OperatorError, Range, YankCommand, YankOperator, operator_commands,
};

// Re-export fallback handler (Epic #372 - Mode Ownership)
pub use fallback::VimFallbackHandler;

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
    VimNormalResolver, VimYankResolver,
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

impl Default for VimModule {
    fn default() -> Self {
        Self::new()
    }
}

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
                .with_scope(OptionScope::Window),
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
            .with_scope(OptionScope::Window),
        ) {
            return ProbeResult::Failed(ModuleError::InitFailed(format!(
                "Failed to register 'relativenumber' option: {e}"
            )));
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

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_module_id() {
        let module = VimModule::new();
        assert_eq!(module.id().as_str(), "vim");
    }

    #[test]
    fn test_vim_module_name() {
        let module = VimModule::new();
        assert_eq!(module.name(), "Vim");
    }

    #[test]
    fn test_vim_module_version() {
        let module = VimModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
    }

    #[test]
    fn test_vim_keybindings_not_empty() {
        let module = VimModule::new();
        let bindings = module.keybindings();
        assert!(!bindings.is_empty(), "Vim module should provide keybindings");
        // Should have at least 100 bindings across all modes
        assert!(bindings.len() > 100, "Vim module should have many keybindings");
    }

    #[test]
    fn test_normal_mode_bindings() {
        let bindings = bindings::normal::bindings();
        assert!(!bindings.is_empty(), "Normal mode should have bindings");
        // Verify basic navigation keys exist
        assert!(bindings.iter().any(|b| b.keys == "h"), "Normal mode should have 'h' binding");
        assert!(bindings.iter().any(|b| b.keys == "j"), "Normal mode should have 'j' binding");
        assert!(bindings.iter().any(|b| b.keys == "k"), "Normal mode should have 'k' binding");
        assert!(bindings.iter().any(|b| b.keys == "l"), "Normal mode should have 'l' binding");
    }

    #[test]
    fn test_insert_mode_bindings() {
        let bindings = bindings::insert::bindings();
        assert!(!bindings.is_empty(), "Insert mode should have bindings");
        // Verify escape key exists
        assert!(
            bindings.iter().any(|b| b.keys == "<Esc>"),
            "Insert mode should have Escape binding"
        );
    }

    #[test]
    fn test_visual_mode_bindings() {
        let bindings = bindings::visual::bindings();
        assert!(!bindings.is_empty(), "Visual mode should have bindings");
    }

    #[test]
    fn test_operator_modes_bindings() {
        let bindings = bindings::operator_modes::all_operator_bindings();
        assert!(!bindings.is_empty(), "Operator modes should have bindings");
    }

    #[test]
    fn test_commandline_mode_bindings() {
        let bindings = bindings::commandline::bindings();
        assert!(!bindings.is_empty(), "Commandline mode should have bindings");
    }

    #[test]
    fn test_all_bindings_aggregation() {
        let all = bindings::all();
        let normal = bindings::normal::bindings();
        let insert = bindings::insert::bindings();
        let visual = bindings::visual::bindings();
        let operator_modes = bindings::operator_modes::all_operator_bindings();
        let cmdline = bindings::commandline::bindings();
        let window = bindings::window::bindings();

        let expected_total = normal.len()
            + insert.len()
            + visual.len()
            + operator_modes.len()
            + cmdline.len()
            + window.len();
        assert_eq!(all.len(), expected_total, "all() should aggregate all mode bindings");
    }

    // ========================================================================
    // Epic #445: Line number option tests
    // ========================================================================

    #[test]
    fn test_line_number_options_registered() {
        let mut module = VimModule::new();
        let ctx = ModuleContext::default();

        let result = module.init(&ctx);
        assert!(
            matches!(result, ProbeResult::Success),
            "Vim module should initialize successfully"
        );

        // Verify 'number' option is registered
        assert!(ctx.kernel.options.contains("number"), "'number' option should be registered");

        // Verify 'relativenumber' option is registered
        assert!(
            ctx.kernel.options.contains("relativenumber"),
            "'relativenumber' option should be registered"
        );
    }

    #[test]
    fn test_line_number_option_aliases() {
        let mut module = VimModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Verify short alias 'nu' resolves to 'number'
        assert_eq!(
            ctx.kernel.options.resolve_name("nu"),
            Some("number".to_string()),
            "'nu' should be an alias for 'number'"
        );

        // Verify short alias 'rnu' resolves to 'relativenumber'
        assert_eq!(
            ctx.kernel.options.resolve_name("rnu"),
            Some("relativenumber".to_string()),
            "'rnu' should be an alias for 'relativenumber'"
        );
    }

    #[test]
    fn test_line_number_option_defaults() {
        let mut module = VimModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Both options should default to false
        let number_val = ctx.kernel.options.get_global("number");
        assert_eq!(number_val, Some(OptionValue::bool(false)), "'number' should default to false");

        let rnu_val = ctx.kernel.options.get_global("relativenumber");
        assert_eq!(
            rnu_val,
            Some(OptionValue::bool(false)),
            "'relativenumber' should default to false"
        );
    }

    #[test]
    fn test_line_number_option_scope() {
        let mut module = VimModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Both options should have Window scope
        let number_spec = ctx.kernel.options.get_spec("number").unwrap();
        assert_eq!(number_spec.scope, OptionScope::Window, "'number' should have Window scope");

        let rnu_spec = ctx.kernel.options.get_spec("relativenumber").unwrap();
        assert_eq!(
            rnu_spec.scope,
            OptionScope::Window,
            "'relativenumber' should have Window scope"
        );
    }

    // ========================================================================
    // Epic #458: GutterRenderer registration test
    // ========================================================================

    #[test]
    fn test_gutter_renderer_registered() {
        let mut module = VimModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Verify GutterRenderer is registered in ServiceRegistry
        let registry = ctx.services.get::<GutterRendererRegistry>();
        assert!(registry.is_some(), "GutterRendererRegistry should be created");

        let registry = registry.unwrap();
        let renderer = registry.get(&GutterRendererKey::Default);
        assert!(renderer.is_some(), "GutterRenderer should be registered with Default key");

        // Verify renderer has source and presenter
        let renderer = renderer.unwrap();
        assert_eq!(renderer.source_count(), 1, "Should have 1 source registered");
        assert_eq!(renderer.presenter_count(), 1, "Should have 1 presenter registered");
    }
}
