# Built-in Modules

This directory contains documentation for reovim's built-in policy modules.

## Module Index

| Module | Crate | Purpose | Status |
|--------|-------|---------|--------|
| [editor](./editor.md) | `reovim-module-editor` | Core editing fallback handler | Documented |
| [keymap](./keymap.md) | `reovim-module-keymap` | Key sequence mapping | Documented |
| [operators](./operators.md) | `reovim-module-operators` | Vim operators (d, y, c) | Documented |
| buffer-ops | `reovim-module-buffer-ops` | Buffer operations | TODO |
| commands | `reovim-module-commands` | Ex commands (:w, :q) | TODO |
| defaults | `reovim-module-defaults` | Default keybindings | TODO |
| layout | — | Window layout policy | Archived (client-side in Phase 10) |
| mode-manager | `reovim-module-mode-manager` | Mode state management | TODO |
| options | `reovim-module-options` | Editor options (:set) | TODO |
| window-ops | `reovim-module-window-ops` | Window operations (`<C-w>` commands) | ✅ Implemented (Phase 11) |

## Example Modules

| Module | Purpose |
|--------|---------|
| example | Template for new modules |
| hot-reload-demo | Hot reload demonstration |

## Module Architecture

All modules implement the `Module` trait from `reovim_kernel::api::v1`:

```rust
pub trait Module: Send + Sync {
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
