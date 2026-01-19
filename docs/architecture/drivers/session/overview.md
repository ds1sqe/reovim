# Session Driver

The session driver (`lib/drivers/session/`) provides traits for session management.

## Overview

| Type | Purpose |
|------|---------|
| `EmptySessionHandler` | Handle empty sessions (no buffers) |
| `EmptySessionAction` | Action to take for empty session |
| `EmptySessionContext` | Context passed to handlers |

## Empty Session Handling

When a session starts with no buffers, the runner calls registered `EmptySessionHandler` implementations to determine what should happen.

### Mechanism vs Policy

- **Mechanism**: `EmptySessionHandler` trait defines the interface
- **Policy**: Modules implement handlers (e.g., `ScratchBufferHandler`)

### Priority Convention

Handlers are called in priority order (lower = first):

| Priority | Use Case |
|----------|----------|
| 0-50 | Core handlers (system-level) |
| 100 | Default module priority |
| 200+ | Late/fallback handlers |

### Flow

```
Session Start
     │
     ▼
┌────────────────────┐
│buffers.is_empty()? │──No──► Normal startup
└────────────────────┘
     │ Yes
     ▼
┌────────────────────────────────┐
│ EmptySessionHandlerRegistry    │
│ .resolve(ctx)                  │
│                                │
│ Handlers (sorted by priority): │
│ 1. ScratchBufferHandler (100)  │
└────────────────────────────────┘
     │
     ▼ First non-None action wins
┌────────────────────────────────┐
│EmptySessionAction::CreateBuffer│
│ { name: None, content: "" }    │
└────────────────────────────────┘
     │
     ▼
Buffer created, session ready
```

## Module Structure

The empty session handling follows a layered architecture:

```
runner/
└── Calls handlers during session creation

modules/scratch-buffer/
└── ScratchBufferHandler (creates empty buffer)

lib/drivers/session/
└── EmptySessionHandler trait (mechanism)
```

## Example: Implementing a Handler

```rust
use reovim_driver_session::{
    EmptySessionAction, EmptySessionContext, EmptySessionHandler
};

pub struct WelcomeHandler;

impl EmptySessionHandler for WelcomeHandler {
    fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
        // Don't show welcome if files were specified
        if !ctx.file_args.is_empty() {
            return EmptySessionAction::None;
        }

        EmptySessionAction::CreateBuffer {
            name: Some("Welcome".to_string()),
            content: "Welcome to reovim!\n".to_string(),
        }
    }

    fn priority(&self) -> u32 {
        50 // Higher priority than scratch buffer
    }

    fn id(&self) -> &'static str {
        "welcome:handler"
    }

    fn description(&self) -> &'static str {
        "Show welcome message"
    }
}
```

## Related

- [Driver Overview](../overview.md)
- [Module System](../../modules/overview.md)
- Issue #369: Empty session handler mechanism
- Issue #381: Expand modules/defaults
