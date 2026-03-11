#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim text objects module - POLICY.
//!
//! This module implements text object commands for operator-pending mode:
//! - Word objects: `iw`, `aw`, `iW`, `aW`
//! - Quote objects: `i"`, `a"`, `i'`, `a'`, `` i` ``, `` a` ``
//! - Bracket objects: `i(`, `a(`, `i[`, `a[`, `i{`, `a{`, `i<`, `a<`
//! - Paragraph objects: `ip`, `ap`
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: `TextObjectEngine::range()` computes text ranges
//! - **Policy (This Module)**: Commands wire keys to specific text objects
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_textobjects::word;
//!
//! // Get all word text object commands
//! let cmds = word::all_commands();
//! for cmd in &cmds {
//!     println!("{}: {}", cmd.id(), cmd.description());
//! }
//! ```

pub mod bracket;
pub mod ids;
pub mod paragraph;
pub mod quote;
pub mod word;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Module identifier for text objects module.
pub const TEXTOBJECTS_MODULE: ModuleId = ModuleId::new("textobjects");

/// Text objects module instance.
pub struct TextObjectsModule;

impl TextObjectsModule {
    /// Create a new text objects module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TextObjectsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TextObjectsModule {
    fn id(&self) -> ModuleId {
        TEXTOBJECTS_MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Text Objects"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Self-register commands (same pattern as EditorModule)
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl CommandProvider for TextObjectsModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        all_commands()
    }
}

/// Get all text object commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds = Vec::new();
    cmds.extend(word::all_commands());
    cmds.extend(quote::all_commands());
    cmds.extend(bracket::all_commands());
    cmds.extend(paragraph::all_commands());
    cmds
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TextObjectsModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_trait() {
        let module = TextObjectsModule;
        assert_eq!(module.id().as_str(), "textobjects");
        assert_eq!(module.name(), "Vim Text Objects");
    }

    #[test]
    fn test_module_version() {
        let module = TextObjectsModule;
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_init() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services.clone(),
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = TextObjectsModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify commands were registered in CommandHandlerStore
        let store = services.get::<CommandHandlerStore>();
        assert!(store.is_some(), "CommandHandlerStore should be registered");
        let handlers = store.unwrap().take_handlers();
        assert_eq!(handlers.len(), 20, "All 20 text object commands should be registered");
    }

    #[test]
    fn test_command_provider_trait() {
        let module = TextObjectsModule::new();
        let handlers = module.command_handlers();
        assert_eq!(handlers.len(), all_commands().len());
    }

    #[test]
    fn test_module_exit() {
        let mut module = TextObjectsModule::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_default() {
        let module = TextObjectsModule;
        assert_eq!(module.name(), "Vim Text Objects");
    }

    #[test]
    fn test_module_new() {
        let _module = TextObjectsModule::new();
    }

    #[test]
    fn test_textobjects_module_id_constant() {
        assert_eq!(TEXTOBJECTS_MODULE.as_str(), "textobjects");
    }

    #[test]
    fn test_word_commands_count() {
        let cmds = word::all_commands();
        assert_eq!(cmds.len(), 4); // iw, aw, iW, aW
    }

    #[test]
    fn test_quote_commands_count() {
        let cmds = quote::all_commands();
        assert_eq!(cmds.len(), 6); // i", a", i', a', i`, a`
    }

    #[test]
    fn test_bracket_commands_count() {
        let cmds = bracket::all_commands();
        assert_eq!(cmds.len(), 8); // i(, a(, i[, a[, i{, a{, i<, a<
    }

    #[test]
    fn test_paragraph_commands_count() {
        let cmds = paragraph::all_commands();
        assert_eq!(cmds.len(), 2); // ip, ap
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 20); // 4 + 6 + 8 + 2
    }

    #[test]
    fn test_module_default_impl() {
        fn make_default<T: Default>() -> T {
            T::default()
        }
        let module: TextObjectsModule = make_default();
        assert_eq!(module.id().as_str(), "textobjects");
    }

    #[test]
    fn test_all_commands_have_valid_ids() {
        let cmds = all_commands();
        for cmd in &cmds {
            assert_eq!(cmd.id().module().as_str(), "textobjects");
            assert!(!cmd.id().name().is_empty());
        }
    }

    #[test]
    fn test_all_commands_have_descriptions() {
        let cmds = all_commands();
        for cmd in &cmds {
            assert!(!cmd.description().is_empty());
        }
    }

    #[test]
    fn test_all_commands_have_args() {
        let cmds = all_commands();
        for cmd in &cmds {
            let args = cmd.args();
            assert!(!args.is_empty(), "Command {} should have args", cmd.id());
        }
    }
}
