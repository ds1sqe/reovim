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

use std::collections::HashMap;

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

// Epic #417 Part 2: Removed pub use re-exports.
// defaults is now ONLY a module list. Runner imports directly from module crates.

// Import traits from their respective modules (mechanism vs policy)
use {reovim_driver_command::CommandHandler, reovim_module_vim::Operator};

// Internal-only imports for create_modules() and helper functions
// Note: Client-side modules removed (Epic #465 Phase 11)
use {
    reovim_module_buffer_simple as buffer_simple, reovim_module_clipboard as clipboard,
    reovim_module_cmdline as cmdline, reovim_module_commands as commands,
    reovim_module_completion as completion, reovim_module_editor as editor,
    reovim_module_explorer as explorer, reovim_module_git as git,
    reovim_module_git_blame as git_blame, reovim_module_git_signs as git_signs,
    reovim_module_git_statusline as git_statusline, reovim_module_health_check as health_check,
    reovim_module_keymap as keymap, reovim_module_lsp as lsp,
    reovim_module_lsp_navigation as lsp_navigation, reovim_module_microscope as microscope,
    reovim_module_motions as motions, reovim_module_notification as notification,
    reovim_module_profiles as profiles, reovim_module_range_finder as range_finder,
    reovim_module_scratch_buffer as scratch_buffer, reovim_module_search as search,
    reovim_module_snippet as snippet, reovim_module_tetromino as tetromino,
    reovim_module_treesitter_markdown as treesitter_markdown,
    reovim_module_treesitter_rust as treesitter_rust, reovim_module_undo as undo,
    reovim_module_vfs_local as vfs_local, reovim_module_vim as vim,
    reovim_module_whichkey as whichkey, reovim_picker_buffers as picker_buffers,
    reovim_picker_commands as picker_commands, reovim_picker_files as picker_files,
    reovim_picker_git_branches as picker_git_branches, reovim_picker_git_log as picker_git_log,
    reovim_picker_git_stash as picker_git_stash, reovim_picker_git_status as picker_git_status,
    reovim_picker_grep as picker_grep, reovim_picker_options as picker_options,
};

/// Type alias for module factory functions.
///
/// Each factory creates a boxed module instance. Used by [`DefaultsModule::builtin_registry`]
/// to map module IDs to their constructors.
pub type ModuleFactory = fn() -> Box<dyn Module>;

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
            // Git provider module (#530) - must init before consumers and pickers
            Box::new(git::GitModule::new()),
            // Git consumer modules (#530) - after vim and git provider
            Box::new(git_signs::GitSignsModule::new()),
            Box::new(git_statusline::GitStatuslineModule::new()),
            Box::new(git_blame::GitBlameModule::new()),
            // Extension bridge modules (#468, #443)
            Box::new(cmdline::CmdlineModule::new()),
            Box::new(whichkey::WhichKeyModule::new()),
            Box::new(notification::NotificationModule::new()),
            // Picker data providers (#522)
            Box::new(picker_files::PickerFilesModule::new()),
            Box::new(picker_buffers::PickerBuffersModule::new()),
            Box::new(picker_commands::PickerCommandsModule::new()),
            Box::new(picker_grep::PickerGrepModule::new()),
            Box::new(picker_options::PickerOptionsModule::new()),
            // Git picker data providers (#530)
            Box::new(picker_git_branches::PickerGitBranchesModule::new()),
            Box::new(picker_git_log::PickerGitLogModule::new()),
            Box::new(picker_git_stash::PickerGitStashModule::new()),
            Box::new(picker_git_status::PickerGitStatusModule::new()),
            // Picker orchestration (#522)
            Box::new(microscope::MicroscopeModule::new()),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            Box::new(treesitter_rust::TreesitterRustModule::new()),
            Box::new(treesitter_markdown::TreesitterMarkdownModule::new()),
            // Code intelligence modules (#520, #532)
            Box::new(lsp::LspModule::new()),
            Box::new(lsp_navigation::LspNavigationModule::new()),
            // Snippet expansion (#136) - reads ModeBridgeStore from personality manifest
            Box::new(snippet::SnippetModule::new()),
            // Jump navigation and code folding (#524) - reads ModeBridgeStore from personality manifest
            Box::new(range_finder::RangeFinderModule::new()),
            // Completion engine (#521)
            Box::new(completion::CompletionModule::new()),
            // File explorer (#523)
            Box::new(explorer::ExplorerModule::new()),
            // Tetromino game (#537)
            Box::new(tetromino::TetrominoModule::new()),
            // Configuration profiles (#593)
            Box::new(profiles::ProfilesModule::new()),
            // Health-check diagnostic command (#594)
            Box::new(health_check::HealthCheckModule::new()),
        ]
    }

    /// Get the builtin module registry: a map from module ID to factory function.
    ///
    /// This is the canonical list of all builtin server modules. Bootstrap
    /// iterates this registry and calls factories for enabled modules only.
    #[must_use]
    pub fn builtin_registry() -> HashMap<&'static str, ModuleFactory> {
        let mut map: HashMap<&'static str, ModuleFactory> =
            HashMap::with_capacity(Self::builtin_order().len());
        // Service modules
        map.insert("undo", || Box::new(undo::UndoModule::new()));
        map.insert("buffer-simple", || Box::new(buffer_simple::BufferSimpleModule::new()));
        map.insert("search", || Box::new(search::SearchModule::new()));
        map.insert("scratch-buffer", || Box::new(scratch_buffer::ScratchBufferModule::new()));
        map.insert("vfs-local", || Box::new(vfs_local::VfsLocalModule::new()));
        map.insert("clipboard", || Box::new(clipboard::ClipboardModule::new()));
        // Utility modules
        map.insert("keymap", || Box::new(keymap::KeymapModule));
        map.insert("commands", || Box::new(commands::CommandsModule));
        // Policy modules
        map.insert("editor", || Box::new(editor::EditorModule));
        map.insert("motions", || Box::new(motions::MotionsModule));
        map.insert("vim", || Box::new(vim::VimModule::new()));
        // Git provider (#530) - must init before consumers and pickers
        map.insert("git", || Box::new(git::GitModule::new()));
        // Git consumer modules (#530)
        map.insert("git-signs", || Box::new(git_signs::GitSignsModule::new()));
        map.insert("git-statusline", || Box::new(git_statusline::GitStatuslineModule::new()));
        map.insert("git-blame", || Box::new(git_blame::GitBlameModule::new()));
        // Extension bridge modules
        map.insert("cmdline", || Box::new(cmdline::CmdlineModule::new()));
        map.insert("whichkey", || Box::new(whichkey::WhichKeyModule::new()));
        map.insert("notification", || Box::new(notification::NotificationModule::new()));
        // Picker data providers
        map.insert("picker-files", || Box::new(picker_files::PickerFilesModule::new()));
        map.insert("picker-buffers", || Box::new(picker_buffers::PickerBuffersModule::new()));
        map.insert("picker-commands", || Box::new(picker_commands::PickerCommandsModule::new()));
        map.insert("picker-grep", || Box::new(picker_grep::PickerGrepModule::new()));
        map.insert("picker-options", || Box::new(picker_options::PickerOptionsModule::new()));
        // Git picker providers (#530)
        map.insert("picker-git-branches", || {
            Box::new(picker_git_branches::PickerGitBranchesModule::new())
        });
        map.insert("picker-git-log", || Box::new(picker_git_log::PickerGitLogModule::new()));
        map.insert("picker-git-stash", || Box::new(picker_git_stash::PickerGitStashModule::new()));
        map.insert("picker-git-status", || {
            Box::new(picker_git_status::PickerGitStatusModule::new())
        });
        // Picker orchestration
        map.insert("microscope", || Box::new(microscope::MicroscopeModule::new()));
        // Syntax highlighting modules
        map.insert("treesitter-rust", || Box::new(treesitter_rust::TreesitterRustModule::new()));
        map.insert("treesitter-markdown", || {
            Box::new(treesitter_markdown::TreesitterMarkdownModule::new())
        });
        // Code intelligence modules
        map.insert("lsp", || Box::new(lsp::LspModule::new()));
        map.insert("lsp-navigation", || Box::new(lsp_navigation::LspNavigationModule::new()));
        // Snippet expansion
        map.insert("snippet", || Box::new(snippet::SnippetModule::new()));
        // Jump navigation and code folding
        map.insert("range-finder", || Box::new(range_finder::RangeFinderModule::new()));
        // Completion engine
        map.insert("completion", || Box::new(completion::CompletionModule::new()));
        // File explorer
        map.insert("explorer", || Box::new(explorer::ExplorerModule::new()));
        // Tetromino game
        map.insert("tetromino", || Box::new(tetromino::TetrominoModule::new()));
        // Configuration profiles (#593)
        map.insert("profiles", || Box::new(profiles::ProfilesModule::new()));
        // Health-check diagnostic command (#594)
        map.insert("health-check", || Box::new(health_check::HealthCheckModule::new()));
        map
    }

    /// Get the ordered list of builtin module IDs.
    ///
    /// This preserves the canonical initialization order when no dependency
    /// graph reordering is applied. Used as the key iteration order for
    /// [`builtin_registry()`].
    #[must_use]
    pub const fn builtin_order() -> &'static [&'static str] {
        &[
            // Service modules
            "undo",
            "buffer-simple",
            "search",
            "scratch-buffer",
            "vfs-local",
            "clipboard",
            // Utility modules
            "keymap",
            "commands",
            // Policy modules
            "editor",
            "motions",
            "vim",
            // Git provider (#530) - must init before consumers and pickers
            "git",
            // Git consumer modules (#530) - after vim and git provider
            "git-signs",
            "git-statusline",
            "git-blame",
            // Extension bridge modules
            "cmdline",
            "whichkey",
            "notification",
            // Picker data providers
            "picker-files",
            "picker-buffers",
            "picker-commands",
            "picker-grep",
            "picker-options",
            // Git picker providers (#530)
            "picker-git-branches",
            "picker-git-log",
            "picker-git-stash",
            "picker-git-status",
            // Picker orchestration
            "microscope",
            // Syntax highlighting modules
            "treesitter-rust",
            "treesitter-markdown",
            // Code intelligence modules
            "lsp",
            "lsp-navigation",
            // Snippet expansion
            "snippet",
            // Jump navigation and code folding
            "range-finder",
            // Completion engine
            "completion",
            // File explorer
            "explorer",
            // Tetromino game
            "tetromino",
            // Configuration profiles (#593)
            "profiles",
            // Health-check diagnostic command (#594)
            "health-check",
        ]
    }

    /// Create modules filtered by a predicate.
    ///
    /// Instantiates only modules for which `is_enabled(module_id)` returns `true`.
    /// Maintains canonical ordering from [`builtin_order()`].
    #[must_use]
    pub fn create_modules_filtered<F>(is_enabled: F) -> Vec<Box<dyn Module>>
    where
        F: Fn(&str) -> bool,
    {
        let registry = Self::builtin_registry();
        Self::builtin_order()
            .iter()
            .filter(|id| is_enabled(id))
            .filter_map(|id| registry.get(id).map(|factory| factory()))
            .collect()
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
            // Git provider and consumer modules (#530)
            ModuleId::new("git"),
            ModuleId::new("git-signs"),
            ModuleId::new("git-statusline"),
            ModuleId::new("git-blame"),
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
            // Picker data providers (#522)
            ModuleId::new("picker-files"),
            ModuleId::new("picker-buffers"),
            ModuleId::new("picker-commands"),
            ModuleId::new("picker-grep"),
            ModuleId::new("picker-options"),
            // Git picker data providers (#530)
            ModuleId::new("picker-git-branches"),
            ModuleId::new("picker-git-log"),
            ModuleId::new("picker-git-stash"),
            ModuleId::new("picker-git-status"),
            // Picker orchestration (#522)
            ModuleId::new("microscope"),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            ModuleId::new("treesitter-rust"),
            ModuleId::new("treesitter-markdown"),
            // Code intelligence modules (#520, #532)
            ModuleId::new("lsp"),
            ModuleId::new("lsp-navigation"),
            // Snippet expansion (#136)
            ModuleId::new("snippet"),
            // Jump navigation and code folding (#524)
            ModuleId::new("range-finder"),
            // Completion engine (#521)
            ModuleId::new("completion"),
            // File explorer (#523)
            ModuleId::new("explorer"),
            // Tetromino game (#537)
            ModuleId::new("tetromino"),
            // Configuration profiles (#593)
            ModuleId::new("profiles"),
            // Health-check diagnostic command (#594)
            ModuleId::new("health-check"),
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

/// Get all default command handlers.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    commands::command_handlers()
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
#[path = "lib_tests.rs"]
mod tests;
