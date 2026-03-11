#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Core Editor Module for Reovim
//!
//! This module provides basic editor commands (cursor movement, text operations)
//! that form the foundation of vim-style editing.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (kernel/drivers): Mode/Command traits, registries
//! - **Policy** (this module): Command implementations
//!
//! # Components
//!
//! - [`command`]: Cursor movement, delete, yank, paste, and other text operations
//! - [`ResolverRegistry`]: Motion and text object resolvers (re-exported from driver)
//!
//! # Note
//!
//! Mode definitions and mode-specific commands (enter insert, exit to normal,
//! etc.) have been moved to `reovim_module_vim`. This module provides
//! policy-agnostic editor commands; mode identities and transitions are in vim.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_editor::command::{CursorDown, DeleteLine};
//!
//! // Register editor commands in your application
//! for cmd in reovim_module_editor::command::all_commands() {
//!     command_registry.register(cmd);
//! }
//! ```

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, OptionConstraint, OptionScope, OptionSpec,
        OptionValue, ProbeResult, Version,
    },
};

pub mod command;
pub mod display_lines;
pub mod ids;
pub mod resolver;

pub use resolver::ResolverRegistry;

/// Module identifier for editor module.
pub const EDITOR_MODULE: ModuleId = ids::MODULE;

/// Editor module instance.
///
/// Provides core editor commands: cursor movement, text operations,
/// undo/redo, yank/paste, etc. These commands are policy-agnostic;
/// mode-specific behavior (vim modes) is in the vim module.
pub struct EditorModule;

impl EditorModule {
    /// Create a new editor module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EditorModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EditorModule {
    fn id(&self) -> ModuleId {
        EDITOR_MODULE
    }

    fn name(&self) -> &'static str {
        "Editor"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Epic #417 Part 3: Self-register commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

        // Epic #570: Register editor options (#573)
        for spec in editor_option_specs() {
            if let Err(e) = ctx.kernel.options.register(spec) {
                return ProbeResult::Failed(ModuleError::InitFailed(format!(
                    "Failed to register editor option: {e}"
                )));
            }
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl CommandProvider for EditorModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        command::all_commands()
    }
}

// ============================================================================
// Option specifications (#573)
// ============================================================================

/// Editor option specifications.
///
/// These are standard editing options for indentation, tab behavior, and text width.
/// Registered during `EditorModule::init()`.
fn editor_option_specs() -> Vec<OptionSpec> {
    vec![
        OptionSpec::new("tabstop", "Number of spaces per tab", OptionValue::int(4))
            .with_short("ts")
            .with_scope(OptionScope::Buffer)
            .with_constraint(OptionConstraint::range(1, 32))
            .with_owner(EDITOR_MODULE),
        OptionSpec::new("shiftwidth", "Spaces for auto-indent", OptionValue::int(4))
            .with_short("sw")
            .with_scope(OptionScope::Buffer)
            .with_constraint(OptionConstraint::range(1, 32))
            .with_owner(EDITOR_MODULE),
        OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
            .with_short("et")
            .with_scope(OptionScope::Buffer)
            .with_owner(EDITOR_MODULE),
        OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
            .with_short("ai")
            .with_scope(OptionScope::Buffer)
            .with_owner(EDITOR_MODULE),
        OptionSpec::new("textwidth", "Maximum text width (0 = no limit)", OptionValue::int(0))
            .with_short("tw")
            .with_scope(OptionScope::Buffer)
            .with_constraint(OptionConstraint::min(0))
            .with_owner(EDITOR_MODULE),
    ]
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(EditorModule);

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{KernelContext, Module},
        std::sync::Arc,
    };

    #[test]
    fn test_editor_module_new() {
        let module = EditorModule::new();
        assert_eq!(module.name(), "Editor");
    }

    #[test]
    fn test_editor_module_default() {
        let module = EditorModule;
        assert_eq!(module.name(), "Editor");
    }

    #[test]
    fn test_editor_module_default_trait() {
        let module = <EditorModule as Default>::default();
        assert_eq!(module.name(), "Editor");
        assert_eq!(module.id().as_str(), "editor");
    }

    #[test]
    fn test_editor_module_id() {
        let module = EditorModule::new();
        assert_eq!(module.id().as_str(), "editor");
    }

    #[test]
    fn test_editor_module_version() {
        let module = EditorModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_editor_module_exit() {
        let mut module = EditorModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_editor_module_constant() {
        assert_eq!(EDITOR_MODULE.as_str(), "editor");
    }

    #[test]
    fn test_editor_module_command_provider() {
        let module = EditorModule::new();
        let handlers = module.command_handlers();
        assert!(!handlers.is_empty());
        // Should match all_commands() count
        assert_eq!(handlers.len(), command::all_commands().len());
    }

    #[test]
    fn test_editor_module_init() {
        use reovim_kernel::api::ServiceRegistry;

        let mut module = EditorModule::new();
        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services,
            std::path::PathBuf::from("/tmp/test-data"),
            std::path::PathBuf::from("/tmp/test-cache"),
        );
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
    }

    // ========================================================================
    // Epic #570: Editor options (#573)
    // ========================================================================

    #[test]
    fn test_editor_option_specs_count() {
        let specs = editor_option_specs();
        assert_eq!(specs.len(), 5);
    }

    #[test]
    fn test_editor_options_registered_after_init() {
        let mut module = EditorModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        let expected = [
            "tabstop",
            "shiftwidth",
            "expandtab",
            "autoindent",
            "textwidth",
        ];
        for name in &expected {
            assert!(ctx.kernel.options.contains(name), "'{name}' should be registered");
        }
    }

    #[test]
    fn test_editor_options_aliases() {
        let mut module = EditorModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        let aliases = [
            ("ts", "tabstop"),
            ("sw", "shiftwidth"),
            ("et", "expandtab"),
            ("ai", "autoindent"),
            ("tw", "textwidth"),
        ];
        for (short, full) in &aliases {
            assert_eq!(
                ctx.kernel.options.resolve_name(short),
                Some(full.to_string()),
                "'{short}' should resolve to '{full}'"
            );
        }
    }

    #[test]
    fn test_editor_options_defaults() {
        let mut module = EditorModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        assert_eq!(ctx.kernel.options.get_global("tabstop"), Some(OptionValue::int(4)));
        assert_eq!(ctx.kernel.options.get_global("shiftwidth"), Some(OptionValue::int(4)));
        assert_eq!(ctx.kernel.options.get_global("expandtab"), Some(OptionValue::bool(true)));
        assert_eq!(ctx.kernel.options.get_global("autoindent"), Some(OptionValue::bool(true)));
        assert_eq!(ctx.kernel.options.get_global("textwidth"), Some(OptionValue::int(0)));
    }

    #[test]
    fn test_editor_options_ownership() {
        let mut module = EditorModule::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        let editor_options = ctx.kernel.options.list_by_module(&EDITOR_MODULE);
        assert_eq!(editor_options.len(), 5);
    }

    #[test]
    fn test_editor_options_scopes() {
        let specs = editor_option_specs();
        for spec in &specs {
            assert_eq!(
                spec.scope,
                OptionScope::Buffer,
                "'{name}' should have Buffer scope",
                name = spec.name
            );
        }
    }

    #[test]
    fn test_editor_init_fails_on_duplicate_option() {
        let ctx = ModuleContext::default();

        // Pre-register one of our options to trigger a conflict
        let _ = ctx.kernel.options.register(OptionSpec::new(
            "tabstop",
            "Already taken",
            OptionValue::int(1),
        ));

        let mut module = EditorModule::new();
        let result = module.init(&ctx);
        assert!(
            matches!(result, ProbeResult::Failed(_)),
            "init should fail on duplicate option"
        );
    }
}
