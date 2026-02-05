# defaults Module

Meta-module that aggregates 13 default modules for standard editor functionality.

## Source Location

`server/modules/defaults/src/`

## Purpose

Provides a single entry point to load all default server-side modules. The runner
can load this bundle to get core vim-like editor functionality.

This module aggregates:

**Service modules (6):**
- `undo` - Undo/redo provider
- `buffer-simple` - Buffer manager implementation
- `search` - Search provider
- `scratch-buffer` - Empty buffer on startup
- `vfs-local` - Local filesystem VFS
- `clipboard` - Clipboard operations

**Utility modules (2):**
- `keymap` - Keymap utilities
- `commands` - Ex-commands

**Policy modules (3):**
- `editor` - Core editing operations
- `motions` - Movement commands
- `vim` - Vim-like behavior

**Syntax modules (2):**
- `treesitter-rust` - Rust syntax highlighting
- `treesitter-markdown` - Markdown syntax highlighting

## Key Types

```rust
pub struct DefaultsModule;

impl DefaultsModule {
    /// Create instances of all default server-side modules
    pub fn create_modules() -> Vec<Box<dyn Module>> {
        vec![
            // Service modules
            Box::new(undo::UndoModule::new()),
            Box::new(buffer_simple::BufferSimpleModule::new()),
            // ... 11 more modules
        ]
    }
}

impl Module for DefaultsModule {
    fn id(&self) -> ModuleId { ModuleId::new("defaults") }
    fn name(&self) -> &'static str { "Default Modules Bundle" }

    fn dependencies(&self) -> Vec<ModuleId> {
        // Returns all 13 module IDs
    }
}
```

## Helper Functions

```rust
/// Get all default operators (from vim module)
pub fn operators() -> Vec<Box<dyn Operator>>;

/// Get all default commands (from commands module)
pub fn commands() -> Vec<Box<dyn ExCommandHandler>>;

/// Get all default keybindings (from vim module)
pub fn keybindings() -> Vec<KeybindingRegistration>;
```

## Dependencies

All 13 sub-modules plus:
- `reovim_kernel::api::v1` - Module trait

## Example Usage

```rust
use reovim_module_defaults::DefaultsModule;

// Load all default modules
let modules = DefaultsModule::create_modules();
for module in modules {
    registry.load(module)?;
}

// Or just the bundle (declares dependencies)
let defaults = DefaultsModule::new();
registry.load(Box::new(defaults))?;
```

## Note on Client-Side Modules

Client-side modules (layout, pair, cmdline, statusline, which-key, undotree)
were removed in Epic #465 Phase 11. These will be reimplemented as client-side
plugins.

## Related Documents

- [Module System Overview](../overview.md)
- [vim Module](./vim.md)
