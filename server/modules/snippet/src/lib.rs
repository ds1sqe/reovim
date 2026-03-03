#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Snippet expansion module for reovim (#136).
//!
//! Provides TextMate/LSP-compatible snippet expansion with intelligent
//! tab stop navigation. Designed for future extensibility (LSP providers,
//! completion integration, transforms).
//!
//! # Architecture
//!
//! - `ast` - Snippet element types (full EBNF grammar coverage)
//! - `parser` - Recursive descent parser (Phase 1: tabstops + placeholders)
//! - `engine` - Active snippet tracking with position updates
//! - `provider` - Snippet provider trait and registry
//! - `loader` - JSON file loader (VSCode-compatible format)
//! - `resolver` - Mode key resolver for snippet navigation mode
//! - `state` - Per-client session extension state
//! - `command` - Command handlers (expand, jump next/prev, cancel)
//!
//! # Commands
//!
//! - `snippet:expand` (`<C-s>` in insert mode) - Expand snippet at cursor
//! - `snippet:jump-next` (`<Tab>` in snippet mode) - Next tab stop
//! - `snippet:jump-prev` (`<S-Tab>` in snippet mode) - Previous tab stop
//! - `snippet:cancel` (`<Esc>` in snippet mode) - Cancel snippet navigation

pub mod ast;
pub mod command;
pub mod engine;
pub mod ids;
pub mod loader;
pub mod parser;
pub mod provider;
pub mod resolver;
pub mod state;

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_kernel::api::v1::{
        CursorStyle, KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Version, pr_info,
    },
};

use crate::{loader::JsonSnippetProvider, provider::SnippetRegistry};

/// Snippet expansion module.
///
/// Manages snippet loading, parsing, expansion, and tab stop navigation.
/// Registers commands, mode resolver, and keybindings during init.
pub struct SnippetModule {
    /// Snippet registry (kept alive for command handler access).
    registry: Option<Arc<SnippetRegistry>>,
}

impl SnippetModule {
    /// Create a new snippet module.
    #[must_use]
    pub const fn new() -> Self {
        Self { registry: None }
    }
}

impl Default for SnippetModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SnippetModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Snippet"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // 1. Load snippet files from data directory
        let data_dir = ctx.data_dir.join("snippets");
        let json_provider = JsonSnippetProvider::load_directory(&data_dir).unwrap_or_default();
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(json_provider));
        let registry = Arc::new(registry);
        self.registry = Some(Arc::clone(&registry));

        // 2. Register command handlers
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in command::all_commands(Arc::clone(&registry)) {
            store.add(handler);
        }

        // 3. Register snippet resolver in ResolverRegistry
        let resolvers = ctx.services.get_or_create::<ResolverRegistry>();
        resolvers.register(resolver::SnippetResolver::new());

        // 4. Register mode info for display
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        modes.add(ModeInfo {
            id: ids::NAVIGATING_MODE,
            display_name: "SNIPPET",
            cursor_style: CursorStyle::Bar,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: Some(ids::VIM_INSERT_MODE),
            is_entry: false,
        });

        // 5. Register keybindings
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        pr_info!(
            "Snippet module initialized with {} commands",
            command::all_commands(registry).len()
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        self.registry = None;
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Expand snippet in insert mode
            KeybindingRegistration::new("<C-s>", ids::EXPAND)
                .with_modes(&["vim:insert"])
                .with_description("Expand snippet at cursor")
                .with_category("snippet"),
            // Tab navigates to next tab stop in snippet mode
            KeybindingRegistration::new("<Tab>", ids::JUMP_NEXT)
                .with_modes(&["snippet:navigating"])
                .with_description("Jump to next tab stop")
                .with_category("snippet"),
            // S-Tab navigates to previous tab stop
            KeybindingRegistration::new("<S-Tab>", ids::JUMP_PREV)
                .with_modes(&["snippet:navigating"])
                .with_description("Jump to previous tab stop")
                .with_category("snippet"),
            // Esc cancels snippet navigation
            KeybindingRegistration::new("<Esc>", ids::CANCEL)
                .with_modes(&["snippet:navigating"])
                .with_description("Cancel snippet navigation")
                .with_category("snippet"),
        ]
    }
}

impl CommandProvider for SnippetModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let registry = self
            .registry
            .clone()
            .unwrap_or_else(|| Arc::new(SnippetRegistry::new()));
        command::all_commands(registry)
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(SnippetModule);

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SnippetModule construction
    // =========================================================================

    #[test]
    fn test_new_creates_module() {
        let module = SnippetModule::new();
        assert!(module.registry.is_none());
    }

    #[test]
    fn test_default_creates_same_as_new() {
        let from_new = SnippetModule::new();
        let from_default = SnippetModule::default();
        assert!(from_new.registry.is_none());
        assert!(from_default.registry.is_none());
    }

    // =========================================================================
    // Module trait
    // =========================================================================

    #[test]
    fn test_module_id() {
        let module = SnippetModule::new();
        assert_eq!(module.id().as_str(), "snippet");
    }

    #[test]
    fn test_module_name() {
        let module = SnippetModule::new();
        assert_eq!(module.name(), "Snippet");
    }

    #[test]
    fn test_module_version() {
        let module = SnippetModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    // =========================================================================
    // Exit
    // =========================================================================

    #[test]
    fn test_exit_clears_registry() {
        let mut module = SnippetModule::new();
        module.registry = Some(Arc::new(SnippetRegistry::new()));
        assert!(module.exit().is_ok());
        assert!(module.registry.is_none());
    }

    #[test]
    fn test_exit_ok_when_no_registry() {
        let mut module = SnippetModule::new();
        assert!(module.exit().is_ok());
    }

    // =========================================================================
    // Keybindings
    // =========================================================================

    #[test]
    fn test_keybindings_count() {
        let module = SnippetModule::new();
        assert_eq!(module.keybindings().len(), 4);
    }

    #[test]
    fn test_keybindings_expand() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let expand = &bindings[0];
        assert_eq!(expand.keys, "<C-s>");
        assert_eq!(expand.command_id, ids::EXPAND);
    }

    #[test]
    fn test_keybindings_jump_next() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let next = &bindings[1];
        assert_eq!(next.keys, "<Tab>");
        assert_eq!(next.command_id, ids::JUMP_NEXT);
    }

    #[test]
    fn test_keybindings_jump_prev() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let prev = &bindings[2];
        assert_eq!(prev.keys, "<S-Tab>");
        assert_eq!(prev.command_id, ids::JUMP_PREV);
    }

    #[test]
    fn test_keybindings_cancel() {
        let module = SnippetModule::new();
        let bindings = module.keybindings();
        let cancel = &bindings[3];
        assert_eq!(cancel.keys, "<Esc>");
        assert_eq!(cancel.command_id, ids::CANCEL);
    }

    // =========================================================================
    // CommandProvider
    // =========================================================================

    #[test]
    fn test_command_provider_returns_handlers() {
        let module = SnippetModule::new();
        let handlers = module.command_handlers();
        assert_eq!(handlers.len(), 4);
    }

    #[test]
    fn test_command_provider_matches_all_commands() {
        let registry = Arc::new(SnippetRegistry::new());
        let module_handlers = SnippetModule::new().command_handlers();
        let all = command::all_commands(registry);
        assert_eq!(module_handlers.len(), all.len());
    }

    // =========================================================================
    // Dependencies
    // =========================================================================

    #[test]
    fn test_dependencies_default_empty() {
        let module = SnippetModule::new();
        assert!(module.dependencies().is_empty());
    }

    // =========================================================================
    // Init with real context
    // =========================================================================

    #[test]
    fn test_init_registers_commands() {
        use {
            reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
            std::path::PathBuf,
        };

        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());
        let kernel = KernelContext::with_event_bus_services_and_options(
            event_bus,
            Arc::clone(&services),
            Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
        );
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/reovim-snippet-test/data"),
            PathBuf::from("/tmp/reovim-snippet-test/cache"),
        );

        let mut module = SnippetModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
        assert!(module.registry.is_some());

        // Verify CommandHandlerStore is populated
        let store = ctx.services.get::<CommandHandlerStore>();
        assert!(store.is_some());

        // Verify ResolverRegistry is populated
        let resolvers = ctx.services.get::<ResolverRegistry>();
        assert!(resolvers.is_some());

        // Verify ModeInfoStore is populated
        let modes = ctx.services.get::<ModeInfoStore>();
        assert!(modes.is_some());
    }

    #[test]
    fn test_init_then_exit() {
        use {
            reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
            std::path::PathBuf,
        };

        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());
        let kernel = KernelContext::with_event_bus_services_and_options(
            event_bus,
            Arc::clone(&services),
            Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
        );
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/reovim-snippet-test/data"),
            PathBuf::from("/tmp/reovim-snippet-test/cache"),
        );

        let mut module = SnippetModule::new();
        module.init(&ctx);
        assert!(module.registry.is_some());
        module.exit().unwrap();
        assert!(module.registry.is_none());
    }
}
