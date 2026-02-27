# Built-in Modules

This directory contains documentation for reovim's built-in policy modules.

## Module Index

| Module | Crate | Purpose | Status |
|--------|-------|---------|--------|
| [editor](./editor.md) | `reovim-module-editor` | Core editing commands | Documented |
| [keymap](./keymap.md) | `reovim-module-keymap` | Key sequence mapping | Documented |
| [buffer-ops](./buffer-ops.md) | `reovim-module-buffer-ops` | Buffer lifecycle events | Documented |
| [buffer-simple](./buffer-simple.md) | `reovim-module-buffer-simple` | SimpleBufferManager | Documented |
| [commands](./commands.md) | `reovim-module-commands` | Ex commands (:w, :q, :e) | Documented |
| [defaults](./defaults.md) | `reovim-module-defaults` | Default modules bundle | Documented |
| [mode-manager](./mode-manager.md) | `reovim-module-mode-manager` | Mode state management | Documented |
| [options](./options.md) | `reovim-module-options` | Editor options (virtualedit) | Documented |
| [scratch-buffer](./scratch-buffer.md) | `reovim-module-scratch-buffer` | Empty buffer on startup | Documented |
| [cmdline](./cmdline.md) | `reovim-module-cmdline` | Command-line mode input (#468) | Documented |
| [whichkey](./whichkey.md) | `reovim-module-whichkey` | Which-key hints (#468) | Documented |
| window-ops | `reovim-module-window-ops` | Window operations (`<C-w>` commands) | Implemented |
| vim | `reovim-module-vim` | Core Vim-like behavior (includes operators) | Implemented |
| motions | `reovim-module-motions` | Movement commands | Implemented |
| textobjects | `reovim-module-textobjects` | Text object definitions | Implemented |
| clipboard | `reovim-module-clipboard` | Clipboard operations | Implemented |
| search | `reovim-module-search` | Search provider | Implemented |
| undo | `reovim-module-undo` | Undo provider | Implemented |
| vfs-local | `reovim-module-vfs-local` | Local filesystem VFS | Implemented |
| treesitter-rust | `reovim-module-treesitter-rust` | Rust syntax | Implemented |
| treesitter-markdown | `reovim-module-treesitter-markdown` | Markdown syntax | Implemented |

**Total: 21 modules** (10 documented + 11 implemented)

## Module Architecture

All modules implement the `Module` trait from `reovim_kernel::api::v1`:

```rust
pub trait Module: Send + Sync + 'static {
    fn id(&self) -> ModuleId;
    fn name(&self) -> &'static str;
    fn version(&self) -> Version;
    fn dependencies(&self) -> Vec<ModuleId>;

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;
    fn exit(&mut self) -> Result<(), ModuleError>;

    fn commands(&self) -> Vec<CommandRegistration>;
    fn keybindings(&self) -> Vec<KeybindingRegistration>;
    fn event_handlers(&self) -> Vec<EventHandlerRegistration>;
}
```

See [Module System Overview](../overview.md) for architecture details.
