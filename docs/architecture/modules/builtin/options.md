# options Module

Editor settings and configuration (virtualedit, etc.).

## Source Location

`server/modules/options/src/`

## Purpose

Provides configurable editor settings following vim's `:set` option pattern.
Options are POLICY - they define how the editor behaves based on user preferences.
The kernel provides mechanisms; this module provides configuration policy.

## Key Types

```rust
/// When virtual edit is allowed
pub enum VirtualEditMode {
    None,      // Never (default)
    All,       // Always allow
    Block,     // Only in visual block mode (Ctrl-V)
    Insert,    // Only in insert mode
    OneMore,   // Cursor can be one past line end
}

/// Configuration for virtual edit behavior
pub struct VirtualEditConfig {
    modes: Vec<VirtualEditMode>,
}

impl VirtualEditConfig {
    pub fn new() -> Self;
    pub fn all() -> Self;
    pub fn block_only() -> Self;
    pub fn add_mode(&mut self, mode: VirtualEditMode);
    pub fn remove_mode(&mut self, mode: VirtualEditMode);
    pub fn allows_mode(&self, mode: VirtualEditMode) -> bool;
    pub fn is_enabled(&self) -> bool;
}

/// Editor-wide settings container
pub struct EditorSettings {
    pub virtual_edit: VirtualEditConfig,
}

/// Options module
pub struct OptionsModule {
    settings: EditorSettings,
}

impl Module for OptionsModule {
    fn id(&self) -> ModuleId { ModuleId::new("options") }
    fn name(&self) -> &'static str { "Editor Options" }
}
```

## Virtual Edit

Virtual edit allows the cursor to move beyond the end of a line, into "virtual"
space. This is useful for block selections and certain editing operations.

Corresponds to vim's `virtualedit` option.

## Example Usage

```rust
use reovim_module_options::{EditorSettings, VirtualEditMode};

let mut settings = EditorSettings::default();

// Allow virtual edit in block mode only
settings.virtual_edit.add_mode(VirtualEditMode::Block);

// Check if virtual edit is allowed for a specific mode
assert!(!settings.virtual_edit.allows_mode(VirtualEditMode::Insert));
assert!(settings.virtual_edit.allows_mode(VirtualEditMode::Block));
```

## Dependencies

- `reovim_kernel::api::v1` - Module trait

## Future Enhancements

Additional options to implement:
- `number` / `relativenumber` - Line number display
- `wrap` / `nowrap` - Line wrapping
- `tabstop` / `shiftwidth` - Indentation settings
- `ignorecase` / `smartcase` - Search case sensitivity

## Related Documents

- [Module System Overview](../overview.md)
- [User Configuration](../../../user-guide/configuration.md)
