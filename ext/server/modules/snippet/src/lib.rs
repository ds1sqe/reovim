#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Snippet expansion module for reovim (#136).
//!
//! Provides TextMate/LSP-compatible snippet expansion with intelligent
//! tab stop navigation, placeholder mirroring, variable resolution,
//! regex transforms, and choice support.
//!
//! # Architecture
//!
//! - `ast` - Snippet element types (full EBNF grammar coverage)
//! - `parser` - Recursive descent parser (full TextMate/LSP grammar)
//! - `engine` - Active snippet tracking with position updates
//! - `transform` - Regex transform execution with case modifiers
//! - `variables` - Built-in variable resolver (`TM_FILENAME`, etc.)
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
mod expander;
pub mod ids;
pub mod inheritance;
mod keybinding;
pub mod loader;
pub mod loader_friendly;
pub mod parser;
pub mod project;
pub mod provider;
pub mod resolver;
pub mod state;
pub mod transform;
pub mod variables;

use std::path::Path;

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_text_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_text_session::SnippetExpanderRegistry,
    reovim_kernel::api::v1::{
        CursorStyle, KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Version, pr_info,
    },
};

use crate::{
    loader::JsonSnippetProvider,
    loader_friendly::FriendlySnippetsProvider,
    provider::{SnippetRegistry, SnippetRegistryHandle},
};

/// Build a snippet registry from the module data directory.
///
/// Loads providers in priority order (first registered = highest priority):
/// 1. User snippets: `{data_dir}/user/` (or legacy `{data_dir}/snippets/`)
/// 2. Friendly-snippets: `{data_dir}/friendly-snippets/` (if present)
/// 3. Built-in snippets: `{data_dir}/built-in/`
#[must_use]
pub fn build_registry(data_dir: &Path) -> SnippetRegistry {
    let mut registry = SnippetRegistry::new();

    // 1. User snippets (highest priority)
    let user_dir = data_dir.join("user");
    let legacy_dir = data_dir.join("snippets");

    let user_source = if user_dir.exists() {
        &user_dir
    } else {
        // Backward compat: treat legacy `snippets/` as user source
        &legacy_dir
    };

    if let Ok(provider) = JsonSnippetProvider::load_directory(user_source) {
        registry.register(Box::new(provider));
    }

    // 2. Friendly-snippets (middle priority)
    let friendly_dir = data_dir.join("friendly-snippets");
    if friendly_dir.join("package.json").exists()
        && let Ok(provider) = FriendlySnippetsProvider::load(&friendly_dir)
    {
        registry.register(Box::new(provider));
    }

    // 3. Built-in snippets (lowest priority)
    let builtin_dir = data_dir.join("built-in");
    if let Ok(provider) = JsonSnippetProvider::load_directory(&builtin_dir) {
        registry.register(Box::new(provider));
    }

    registry
}

/// Snippet expansion module.
///
/// Manages snippet loading, parsing, expansion, and tab stop navigation.
/// Registers commands, mode resolver, and keybindings during init.
pub struct SnippetModule {
    /// Snippet registry handle (kept alive for command handler access and hot reload).
    handle: Option<SnippetRegistryHandle>,
    /// Data directory for reload support.
    data_dir: Option<std::path::PathBuf>,
}

impl SnippetModule {
    /// Create a new snippet module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            handle: None,
            data_dir: None,
        }
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

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![]
    }

    fn optional_dependencies(&self) -> Vec<ModuleId> {
        // Personality modules (e.g., vim) populate ModeBridgeStore before us.
        // Optional: snippet still works without a personality, just no parent mode bridging.
        vec![ModuleId::new("vim")]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // 1. Load snippet files from data directory (multi-source hierarchy)
        let handle = SnippetRegistryHandle::new(build_registry(&ctx.data_dir));
        self.handle = Some(handle.clone());
        self.data_dir = Some(ctx.data_dir.clone());

        // #585/#610: Read parent mode from ModeBridgeStore (manifest-driven).
        // Falls back to snippet's own mode if no personality loaded.
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        let parent_insert = resolve_snippet_parent(ctx, &modes);

        // Use resolved parent or fallback to snippet's own mode (reduced functionality)
        let effective_parent = parent_insert.unwrap_or(ids::NAVIGATING_MODE);

        // 2. Register command handlers with return mode
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        let commands =
            command::all_commands(handle, effective_parent.clone(), ctx.data_dir.clone());
        let command_count = commands.len();
        for cmd_handler in commands {
            store.add(cmd_handler);
        }

        // 3. Register snippet resolver with looked-up parent
        let resolvers = ctx.services.get_or_create::<ResolverRegistry>();
        resolvers.register(resolver::SnippetResolver::with_parent(effective_parent.clone()));

        // 4. Register mode info for display
        modes.add(ModeInfo {
            id: ids::NAVIGATING_MODE,
            display_name: "SNIPPET",
            cursor_style: CursorStyle::Bar,
            accepts_char_input: true,
            has_selection: true,
            inherits_from: Some(effective_parent),
            is_entry: false,
        });

        // 5. Register keybindings
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // 6. Register SnippetExpander implementation (#542: decouple completion from this module).
        let expander_registry = ctx.services.get_or_create::<SnippetExpanderRegistry>();
        expander_registry.register(Arc::new(expander::SnippetExpanderImpl));

        pr_info!("Snippet module initialized with {command_count} commands");
        ProbeResult::Success
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn exit(&mut self) -> Result<(), ModuleError> {
        self.handle = None;
        self.data_dir = None;
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::SNIPPET_PROVIDER]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        // Vim-mode bindings from personality adapter (#700)
        let mut bindings = keybinding::all();
        // Snippet-mode bindings (module-owned, already qualified)
        bindings.extend([
            // Snippet navigating mode: tab stop navigation
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
        ]);
        bindings
    }
}

impl CommandProvider for SnippetModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        let handle = self
            .handle
            .clone()
            .unwrap_or_else(|| SnippetRegistryHandle::new(SnippetRegistry::new()));
        // Fallback ModeId for CommandProvider (testing/FFI only).
        // In production, init() resolves the real mode from ModeInfoStore.
        let fallback_mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("editor"), "insert");
        let data_dir = self
            .data_dir
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/reovim-snippet-fallback"));
        command::all_commands(handle, fallback_mode, data_dir)
    }
}

/// Resolve the parent mode for snippet:navigating from `ModeBridgeStore`.
///
/// Returns `None` if no personality module is loaded (reduced functionality).
#[cfg_attr(coverage_nightly, coverage(off))]
fn resolve_snippet_parent(
    ctx: &ModuleContext,
    modes: &ModeInfoStore,
) -> Option<reovim_kernel::api::v1::ModeId> {
    use reovim_subsys_manifest::ModeBridgeStore;

    let bridge_store = ctx.services.get::<ModeBridgeStore>()?;
    let parent_str = bridge_store.find_parent("snippet:navigating")?;
    let (module, name) = parent_str.split_once(':')?;

    if let Some(mode_id) = modes.find_by_name(module, name) {
        return Some(mode_id);
    }
    tracing::warn!("Mode bridge parent '{parent_str}' not found in ModeInfoStore");
    None
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(SnippetModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
