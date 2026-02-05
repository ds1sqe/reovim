# mode-manager Module

Mode transition tracker that handles ModeChanged events.

## Source Location

`server/modules/mode-manager/src/`

## Purpose

Handles mode change events from the kernel `EventBus`. Subscribes to
`ModeChanged` events and coordinates mode-related state across the editor.

Following the kernel's "mechanism vs policy" principle:
- Kernel provides the mode change events (mechanism)
- This module decides how to react (policy)

The kernel uses string-based modes (from/to) intentionally to keep mode policy
in modules. The mode strings are opaque to the kernel. Examples: "Normal",
"Insert", "Visual", "Command", etc.

## Key Types

```rust
pub struct ModeManager {
    /// Active subscriptions - stored to keep handlers active (RAII pattern)
    subscriptions: Vec<Subscription>,
}

impl Module for ModeManager {
    fn id(&self) -> ModuleId { ModuleId::new("mode-manager") }
    fn name(&self) -> &'static str { "Mode State Manager" }
    fn version(&self) -> Version { Version::new(0, 1, 0) }
}
```

## Event Subscriptions

| Event | Priority | Purpose |
|-------|----------|---------|
| `ModeChanged` | CORE (10) | Track mode transitions, update UI state |

## Mode Semantics

Mode strings are policy-defined by modules:

| Mode | Description |
|------|-------------|
| `"Normal"` | Normal mode (default) |
| `"Insert"` | Insert mode for text input |
| `"Visual"` | Visual selection mode |
| `"VisualLine"` | Line-wise visual selection |
| `"VisualBlock"` | Block visual selection |
| `"Command"` | Command-line mode |
| `"Replace"` | Replace mode |

## Dependencies

- `reovim_kernel::api::v1` - Module trait, EventBus, ModeChanged event

## Future Enhancements

When mode changes occur, this module can:
- Update cursor style (block vs line)
- Update status line display
- Update available keybindings
- Request render updates via `ctx.request_render()`

## Related Documents

- [Module System Overview](../overview.md)
- [Mode Inheritance](../mode-inheritance.md)
