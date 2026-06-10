# buffer-ops Module

Buffer lifecycle event subscriber for the kernel EventBus.

## Source Location

`ext/server/modules/buffer-ops/src/`

## Purpose

Handles buffer lifecycle events from the kernel `EventBus`. Subscribes to
`BufferCreated`, `TextBufferModified` (text-domain), `BufferClosed`, and `BufferSwitched`
events and provides coordinated buffer state management.

Following the kernel's "mechanism vs policy" principle:
- Kernel provides the buffer events (mechanism)
- This module decides how to react (policy)

## Key Types

```rust
pub struct BufferOps {
    /// Active subscriptions - stored to keep handlers active (RAII pattern)
    subscriptions: Vec<Subscription>,
}

impl Module for BufferOps {
    fn id(&self) -> ModuleId { ModuleId::new("buffer-ops") }
    fn name(&self) -> &'static str { "Buffer Operations" }
    fn version(&self) -> Version { Version::new(0, 1, 0) }
}
```

## Event Subscriptions

| Event | Priority | Purpose |
|-------|----------|---------|
| `BufferCreated` | CORE (10) | Initialize buffer-specific state |
| `TextBufferModified` (text-domain) | NORMAL (50) | Track dirty state, schedule reparse |
| `BufferClosed` | LOW (200) | Cleanup buffer-specific resources |
| `BufferSwitched` | CORE (10) | Update active buffer tracking |

## Dependencies

- `reovim_kernel::api::v1` - Module trait, EventBus, events

## Example Usage

```rust
use reovim_module_buffer_ops::BufferOps;

// Module is typically loaded via defaults bundle
let module = BufferOps::new();
registry.load(Box::new(module))?;
```

## Related Documents

- [Module System Overview](../overview.md)
- [EventBus Documentation](../../kernel/ipc/overview.md)
