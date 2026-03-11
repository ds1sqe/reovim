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
use {reovim_driver_command::CommandHandler, reovim_module_vim::Operator};

// Internal-only imports for create_modules() and helper functions
// Note: Client-side modules removed (Epic #465 Phase 11)
use {
    reovim_module_buffer_simple as buffer_simple, reovim_module_clipboard as clipboard,
    reovim_module_cmdline as cmdline, reovim_module_commands as commands,
    reovim_module_completion as completion, reovim_module_editor as editor,
    reovim_module_explorer as explorer, reovim_module_keymap as keymap, reovim_module_lsp as lsp,
    reovim_module_lsp_navigation as lsp_navigation, reovim_module_microscope as microscope,
    reovim_module_motions as motions, reovim_module_notification as notification,
    reovim_module_profiles as profiles, reovim_module_range_finder as range_finder,
    reovim_module_scratch_buffer as scratch_buffer, reovim_module_search as search,
    reovim_module_snippet as snippet, reovim_module_tetromino as tetromino,
    reovim_module_treesitter_markdown as treesitter_markdown,
    reovim_module_treesitter_rust as treesitter_rust, reovim_module_undo as undo,
    reovim_module_vfs_local as vfs_local, reovim_module_vim as vim,
    reovim_module_vim_completion as vim_completion, reovim_module_vim_explorer as vim_explorer,
    reovim_module_vim_lsp as vim_lsp, reovim_module_vim_microscope as vim_microscope,
    reovim_module_vim_range_finder as vim_range_finder, reovim_module_vim_snippet as vim_snippet,
    reovim_module_whichkey as whichkey, reovim_picker_buffers as picker_buffers,
    reovim_picker_commands as picker_commands, reovim_picker_files as picker_files,
    reovim_picker_grep as picker_grep, reovim_picker_options as picker_options,
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
            // Picker data providers (#522)
            Box::new(picker_files::PickerFilesModule::new()),
            Box::new(picker_buffers::PickerBuffersModule::new()),
            Box::new(picker_commands::PickerCommandsModule::new()),
            Box::new(picker_grep::PickerGrepModule::new()),
            Box::new(picker_options::PickerOptionsModule::new()),
            // Picker orchestration (#522)
            Box::new(microscope::MicroscopeModule::new()),
            // Vim-microscope adapter - bridges vim keybindings to picker commands
            Box::new(vim_microscope::VimMicroscopeModule::new()),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            Box::new(treesitter_rust::TreesitterRustModule::new()),
            Box::new(treesitter_markdown::TreesitterMarkdownModule::new()),
            // Code intelligence modules (#520, #532)
            Box::new(lsp::LspModule::new()),
            Box::new(lsp_navigation::LspNavigationModule::new()),
            // Vim-LSP adapter (#532) - bridges vim keybindings to LSP commands
            Box::new(vim_lsp::VimLspModule::new()),
            // Vim-snippet adapter (#532) - registers SnippetParentMode before snippet
            Box::new(vim_snippet::VimSnippetModule::new()),
            // Snippet expansion (#136) - reads SnippetParentMode from adapter
            Box::new(snippet::SnippetModule::new()),
            // Vim-range-finder adapter (#532) - registers JumpParentMode before range-finder
            Box::new(vim_range_finder::VimRangeFinderModule::new()),
            // Jump navigation and code folding (#524) - reads JumpParentMode from adapter
            Box::new(range_finder::RangeFinderModule::new()),
            // Completion engine (#521)
            Box::new(completion::CompletionModule::new()),
            // Vim-completion adapter - bridges vim keybindings to completion commands
            Box::new(vim_completion::VimCompletionModule::new()),
            // File explorer (#523)
            Box::new(explorer::ExplorerModule::new()),
            // Vim-explorer adapter - bridges vim keybindings to explorer commands
            Box::new(vim_explorer::VimExplorerModule::new()),
            // Tetromino game (#537)
            Box::new(tetromino::TetrominoModule::new()),
            // Configuration profiles (#593)
            Box::new(profiles::ProfilesModule::new()),
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
            // Picker data providers (#522)
            ModuleId::new("picker-files"),
            ModuleId::new("picker-buffers"),
            ModuleId::new("picker-commands"),
            ModuleId::new("picker-grep"),
            ModuleId::new("picker-options"),
            // Picker orchestration (#522)
            ModuleId::new("microscope"),
            // Syntax highlighting modules (Epic #465 Phase 12.1, 12.2)
            ModuleId::new("treesitter-rust"),
            ModuleId::new("treesitter-markdown"),
            // Code intelligence modules (#520, #532)
            ModuleId::new("lsp"),
            ModuleId::new("lsp-navigation"),
            ModuleId::new("vim-lsp"),
            // Snippet expansion (#136)
            ModuleId::new("snippet"),
            ModuleId::new("vim-snippet"),
            // Jump navigation and code folding (#524)
            ModuleId::new("range-finder"),
            ModuleId::new("vim-range-finder"),
            // Completion engine (#521)
            ModuleId::new("completion"),
            ModuleId::new("vim-completion"),
            // File explorer (#523)
            ModuleId::new("explorer"),
            ModuleId::new("vim-explorer"),
            // Picker vim adapter
            ModuleId::new("vim-microscope"),
            // Tetromino game (#537)
            ModuleId::new("tetromino"),
            // Configuration profiles (#593)
            ModuleId::new("profiles"),
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
