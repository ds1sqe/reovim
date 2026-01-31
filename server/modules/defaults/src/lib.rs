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
    reovim_module_commands as commands, reovim_module_editor as editor,
    reovim_module_keymap as keymap, reovim_module_motions as motions,
    reovim_module_scratch_buffer as scratch_buffer, reovim_module_search as search,
    reovim_module_treesitter_rust as treesitter_rust, reovim_module_undo as undo,
    reovim_module_vfs_local as vfs_local, reovim_module_vim as vim,
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
            // Syntax highlighting modules (Epic #465 Phase 12.1)
            Box::new(treesitter_rust::TreesitterRustModule::new()),
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
            // Syntax highlighting modules (Epic #465 Phase 12.1)
            ModuleId::new("treesitter-rust"),
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
        // Syntax modules (1): treesitter-rust
        // Total: 12 modules
        assert_eq!(deps.len(), 12);
    }

    #[test]
    fn test_create_modules() {
        let modules = DefaultsModule::create_modules();
        // Service modules (6): undo, buffer-simple, search, scratch-buffer, vfs-local, clipboard
        // Utility modules (2): keymap, commands
        // Policy modules (3): editor, motions, vim
        // Syntax modules (1): treesitter-rust
        // Total: 12 modules
        assert_eq!(modules.len(), 12);
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
}
