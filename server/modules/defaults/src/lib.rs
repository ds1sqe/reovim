//! Default modules bundle - POLICY aggregator.
//!
//! This module bundles the standard server-side modules:
//! - `vim` - Vim keybindings, operators (normal, insert, visual, operator-pending, d/y/c)
//! - `keymap` - Keymap utilities (mechanism)
//! - `commands` - Ex-commands (:w, :q, :wq)
//! - `editor` - Core editing operations
//! - `motions` - Cursor motion commands
//!
//! # Note on Client-Side Modules
//!
//! Client-side modules (layout, pair, cmdline, statusline, which-key, undotree)
//! were removed in Epic #465 Phase 11.
//! These will be reimplemented as client-side plugins.
//!
//! # Purpose
//!
//! Provides a single entry point to load all default server-side modules.
//! The runner can load this bundle to get core vim-like editor functionality.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_defaults::DefaultsModule;
//!
//! let defaults = DefaultsModule::new();
//! let modules = defaults.create_modules();
//! for module in modules {
//!     registry.load(module)?;
//! }
//! ```

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

// Epic #417 Part 2: Removed pub use re-exports.
// defaults is now ONLY a module list. Runner imports directly from module crates.

// Import traits from their respective modules (mechanism vs policy)
use {reovim_module_commands::ExCommandHandler, reovim_module_vim::Operator};

// Internal-only imports for create_modules() and helper functions
// Note: Client-side modules removed (Epic #465 Phase 11)
use {
    reovim_module_buffer_simple as buffer_simple, reovim_module_clipboard as clipboard,
    reovim_module_cmdline as cmdline, reovim_module_commands as commands,
    reovim_module_completion as completion, reovim_module_editor as editor,
    reovim_module_explorer as explorer, reovim_module_keymap as keymap, reovim_module_lsp as lsp,
    reovim_module_microscope as microscope, reovim_module_motions as motions,
    reovim_module_notification as notification, reovim_module_range_finder as range_finder,
    reovim_module_scratch_buffer as scratch_buffer, reovim_module_search as search,
    reovim_module_snippet as snippet, reovim_module_treesitter_markdown as treesitter_markdown,
    reovim_module_treesitter_rust as treesitter_rust, reovim_module_undo as undo,
    reovim_module_vfs_local as vfs_local, reovim_module_vim as vim,
    reovim_module_whichkey as whichkey,
};

/// Default modules bundle.
///
/// This module aggregates all default modules and provides their
/// keybindings, operators, and commands.
pub struct DefaultsModule;

impl DefaultsModule {
    /// Create a new defaults module bundle.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Create instances of all default server-side modules.
    ///
    /// Returns a vector of boxed module instances that can be loaded
    /// into the module registry.
    ///
    /// # Module Categories
    ///
    /// - **Service modules**: Provide services via `ServiceRegistry` (undo, buffer, search, vfs)
    /// - **Policy modules**: Define behavior (vim, editor, motions)
    ///
    /// All modules are initialized in order - service modules first so that
    /// policy modules can depend on their services.
    ///
    /// # Note
    ///
    /// Client-side modules (layout, pair, cmdline, statusline, which-key, undotree)
    /// were removed in Epic #465 Phase 11. They will be reimplemented as
    /// client-side plugins.
    #[must_use]
    pub fn create_modules() -> Vec<Box<dyn Module>> {
        vec![
            // Service modules (Epic #417) - register providers during init()
            Box::new(undo::UndoModule::new()),
            Box::new(buffer_simple::BufferSimpleModule::new()),
            Box::new(search::SearchModule::new()),
            Box::new(scratch_buffer::ScratchBufferModule::new()),
            Box::new(vfs_local::VfsLocalModule::new()),
            Box::new(clipboard::ClipboardModule::new()),
            // Utility modules
            Box::new(keymap::KeymapModule),
            Box::new(commands::CommandsModule),
            // Policy modules (Epic #417 Part 3) - register commands/keybindings during init()
            Box::new(editor::EditorModule),
            Box::new(motions::MotionsModule),
            Box::new(vim::VimModule::new()),
            // Extension bridge modules (#468, #443)
            Box::new(cmdline::CmdlineModule::new()),
            Box::new(whichkey::WhichKeyModule::new()),
            Box::new(notification::NotificationModule::new()),
            // Picker module (#522)
            Box::new(microscope::MicroscopeModule::new()),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            Box::new(treesitter_rust::TreesitterRustModule::new()),
            Box::new(treesitter_markdown::TreesitterMarkdownModule::new()),
            // Code intelligence modules (#520)
            Box::new(lsp::LspModule::new()),
            // Snippet expansion (#136)
            Box::new(snippet::SnippetModule::new()),
            // Jump navigation and code folding (#524)
            Box::new(range_finder::RangeFinderModule::new()),
            // Completion engine (#521)
            Box::new(completion::CompletionModule::new()),
            // File explorer (#523)
            Box::new(explorer::ExplorerModule::new()),
        ]
    }
}

impl Default for DefaultsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DefaultsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("defaults")
    }

    fn name(&self) -> &'static str {
        "Default Modules Bundle"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        // This module depends on its sub-modules
        // Note: operators merged into vim module (Epic #385)
        // Note: Client-side modules removed (Epic #465 Phase 11)
        vec![
            // Service modules (Epic #417)
            ModuleId::new("undo"),
            ModuleId::new("buffer-simple"),
            ModuleId::new("search"),
            ModuleId::new("scratch-buffer"),
            ModuleId::new("vfs-local"),
            ModuleId::new("clipboard"),
            // Utility modules
            ModuleId::new("keymap"),
            ModuleId::new("commands"),
            // Policy modules (Epic #417 Part 3)
            ModuleId::new("editor"),
            ModuleId::new("motions"),
            ModuleId::new("vim"),
            // Extension bridge modules (#468, #443)
            ModuleId::new("cmdline"),
            ModuleId::new("whichkey"),
            ModuleId::new("notification"),
            // Picker module (#522)
            ModuleId::new("microscope"),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            ModuleId::new("treesitter-rust"),
            ModuleId::new("treesitter-markdown"),
            // Code intelligence modules (#520)
            ModuleId::new("lsp"),
            // Snippet expansion (#136)
            ModuleId::new("snippet"),
            // Jump navigation and code folding (#524)
            ModuleId::new("range-finder"),
            // Completion engine (#521)
            ModuleId::new("completion"),
            // File explorer (#523)
            ModuleId::new("explorer"),
        ]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        // Aggregate keybindings from vim module
        vim::VimModule::new().keybindings()
    }
}

/// Get all default operators.
#[must_use]
pub fn operators() -> Vec<Box<dyn Operator>> {
    vim::operators::operators()
}

/// Get all default commands.
#[must_use]
pub fn commands() -> Vec<Box<dyn ExCommandHandler>> {
    commands::commands()
}

/// Get all default keybindings.
#[must_use]
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vim::bindings::all()
}

// Note: compositor() removed - layout module removed (Epic #465 Phase 11)
// Compositor is now a client-side concern.

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(DefaultsModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_module_id() {
        let module = DefaultsModule::new();
        assert_eq!(module.id().as_str(), "defaults");
    }

    #[test]
    fn test_defaults_module_name() {
        let module = DefaultsModule::new();
        assert_eq!(module.name(), "Default Modules Bundle");
    }

    #[test]
    fn test_defaults_has_dependencies() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        // Service modules (6): undo, buffer-simple, search, scratch-buffer, vfs-local, clipboard
        // Utility modules (2): keymap, commands
        // Policy modules (3): editor, motions, vim
        // Extension bridge modules (3): cmdline, whichkey, notification
        // Syntax modules (2): treesitter-rust, treesitter-markdown
        // Picker module (1): microscope
        // Code intelligence modules (1): lsp
        // Snippet (1): snippet
        // Range-finder (1): range-finder
        // Completion (1): completion
        // Explorer (1): explorer
        // Total: 22 modules
        assert_eq!(deps.len(), 22);
    }

    #[test]
    fn test_create_modules() {
        let modules = DefaultsModule::create_modules();
        // Service modules (6): undo, buffer-simple, search, scratch-buffer, vfs-local, clipboard
        // Utility modules (2): keymap, commands
        // Policy modules (3): editor, motions, vim
        // Extension bridge modules (3): cmdline, whichkey, notification
        // Syntax modules (2): treesitter-rust, treesitter-markdown
        // Picker module (1): microscope
        // Code intelligence modules (1): lsp
        // Snippet (1): snippet
        // Range-finder (1): range-finder
        // Completion (1): completion
        // Explorer (1): explorer
        // Total: 22 modules
        assert_eq!(modules.len(), 22);
    }

    #[test]
    fn test_operators_not_empty() {
        let ops = operators();
        assert!(!ops.is_empty());
    }

    #[test]
    fn test_commands_not_empty() {
        let cmds = commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_keybindings_not_empty() {
        let bindings = keybindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_defaults_module_version() {
        let module = DefaultsModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_defaults_module_default() {
        fn create_default<T: Default>() -> T {
            T::default()
        }
        let from_default: DefaultsModule = create_default();
        let from_new = DefaultsModule::new();
        assert_eq!(from_new.id(), from_default.id());
        assert_eq!(from_new.version(), from_default.version());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = DefaultsModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_dependencies_contain_service_modules() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

        assert!(dep_strs.contains(&"undo"));
        assert!(dep_strs.contains(&"buffer-simple"));
        assert!(dep_strs.contains(&"search"));
        assert!(dep_strs.contains(&"scratch-buffer"));
        assert!(dep_strs.contains(&"vfs-local"));
        assert!(dep_strs.contains(&"clipboard"));
    }

    #[test]
    fn test_dependencies_contain_utility_modules() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

        assert!(dep_strs.contains(&"keymap"));
        assert!(dep_strs.contains(&"commands"));
    }

    #[test]
    fn test_dependencies_contain_policy_modules() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

        assert!(dep_strs.contains(&"editor"));
        assert!(dep_strs.contains(&"motions"));
        assert!(dep_strs.contains(&"vim"));
    }

    #[test]
    fn test_dependencies_contain_syntax_modules() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

        assert!(dep_strs.contains(&"treesitter-rust"));
        assert!(dep_strs.contains(&"treesitter-markdown"));
    }

    #[test]
    fn test_dependencies_contain_lsp_module() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();

        assert!(deps.iter().map(ModuleId::as_str).any(|x| x == "lsp"));
    }

    #[test]
    fn test_create_modules_has_unique_ids() {
        let modules = DefaultsModule::create_modules();
        let ids: Vec<String> = modules
            .iter()
            .map(|m| m.id().as_str().to_string())
            .collect();

        // Check all IDs are unique
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "Module IDs should all be unique, found duplicates");
    }

    #[test]
    fn test_keybindings_from_module_trait() {
        let module = DefaultsModule::new();
        let bindings = module.keybindings();
        // Should be same as the free function
        assert_eq!(bindings.len(), keybindings().len());
    }

    #[test]
    fn test_init_returns_success() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = DefaultsModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
    }
}
