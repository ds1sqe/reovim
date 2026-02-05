# scratch-buffer Module

Creates an empty buffer when a session starts with no buffers.

## Source Location

`server/modules/scratch-buffer/src/`

## Purpose

Provides `ScratchBufferHandler` which creates an empty buffer when a session
starts with no buffers. Like a Linux driver's `probe()` function that initializes
hardware, this handler initializes the editor with a usable state when no files
are specified.

Following the mechanism/policy separation:
- **Mechanism**: `EmptySessionHandler` trait (in `reovim-driver-session`)
- **Policy**: `ScratchBufferHandler` (this module) decides to create a buffer

## Key Types

```rust
/// Handler that creates an empty scratch buffer
pub struct ScratchBufferHandler;

impl EmptySessionHandler for ScratchBufferHandler {
    fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
        // If files were specified on command line, don't create scratch buffer
        if !ctx.file_args.is_empty() {
            return EmptySessionAction::None;
        }

        // Create an empty scratch buffer
        EmptySessionAction::CreateBuffer {
            name: None,  // Unnamed scratch buffer
            content: String::new(),
        }
    }

    fn priority(&self) -> u32 {
        100  // Default module priority
    }

    fn id(&self) -> &'static str {
        "scratch-buffer:handler"
    }
}

/// Module instance
pub struct ScratchBufferModule;

impl Module for ScratchBufferModule {
    fn id(&self) -> ModuleId { ModuleId::new("scratch-buffer") }
    fn name(&self) -> &'static str { "Scratch Buffer" }
}
```

## Behavior

| Scenario | Action |
|----------|--------|
| No files specified | Creates empty scratch buffer |
| Files on command line | Returns `None` (lets file opener handle them) |

## Registration

During `init()`, the module registers itself with the `SessionHandlerRegistry`:

```rust
fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
    let handler_registry = ctx.services.get_or_create::<SessionHandlerRegistry>();
    handler_registry.register(
        SessionHandlerKey::Empty,
        Arc::new(ScratchBufferHandler)
    );
    ProbeResult::Success
}
```

## Priority

Uses the default priority (100), allowing core handlers (0-50) to take precedence
if needed.

## Dependencies

- `reovim_kernel::api::v1` - Module trait
- `reovim_driver_session` - EmptySessionHandler trait, registry types

## Related Documents

- [Module System Overview](../overview.md)
- [session Driver](../../drivers/session/overview.md)
