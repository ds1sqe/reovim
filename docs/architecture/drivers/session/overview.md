# Session Driver

The session driver (`lib/drivers/session/`) provides traits for session management.

## Overview

| Type | Purpose |
|------|---------|
| `driver::Session` | SSOT for per-session editing state |
| `ClientId` | Client connection identifier |
| `EmptySessionHandler` | Handle empty sessions (no buffers) |
| `EmptySessionAction` | Action to take for empty session |
| `EmptySessionContext` | Context passed to handlers |

## Session State (SSOT)

`driver::Session` is the Single Source of Truth for per-session editing state:

| Field | Type | Description |
|-------|------|-------------|
| `mode_stack` | `ModeStack` | Current editing mode hierarchy |
| `pending_keys` | `Vec<KeyEvent>` | Accumulated key sequence |
| `extensions` | `ExtensionMap` | Module-provided state |
| `active_buffer` | `Option<BufferId>` | Default active buffer |
| `terminal_size` | `(u16, u16)` | Session-level terminal size |

### Why SSOT?

Previously, session state was duplicated between `AppState` and `driver::Session`.
This caused:
- Two sources of truth requiring synchronization
- Potential for state drift
- Unclear ownership

Now, `driver::Session` is the authoritative source. `SessionState` in runner
delegates to it via accessors:

```rust
// SessionState delegates to driver_session
pub fn mode_stack(&self) -> &ModeStack {
    &self.driver_session.mode_stack
}
```

## ClientId Type

Client connections are identified by `ClientId`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(usize);
```

- **Not** a session ID - sessions use `SessionId(Arc<str>)`
- Display format: `"client-{id}"`
- Auto-generated via atomic counter

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
