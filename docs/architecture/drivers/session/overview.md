# Session Driver

The session driver (`lib/drivers/session/`) provides traits for session management.

## Overview

| Type | Purpose |
|------|---------|
| `Session` | Shared session infrastructure (`id` + `SessionShared`) |
| `SessionShared` | Truly shared state (compositor, active_buffer, home_mode) |
| `SessionRuntime` | Runtime that borrows Session + per-client state |
| `ClientId` | Client connection identifier |
| `EmptySessionHandler` | Handle empty sessions (no buffers) |
| `EmptySessionAction` | Action to take for empty session |
| `EmptySessionContext` | Context passed to handlers |

## Session Architecture (#491)

The session driver defines types and mechanisms. Per-client state ownership
lives in the server layer (`EditingState`).

### Session (Shared Infrastructure)

```rust
pub struct Session {
    pub id: ClientId,
    pub shared: SessionShared,
}
```

### SessionShared

| Field | Type | Description |
|-------|------|-------------|
| `compositor` | `Option<Box<dyn RootCompositor>>` | Window layout management |
| `active_buffer` | `Option<BufferId>` | Session-level active buffer |
| `terminal_size` | `(u16, u16)` | Default terminal dimensions (80x24) |
| `home_mode` | `ModeId` | Mode for initializing new clients |

### Per-Client State (server::EditingState)

Per-client state lives in `server/lib/server/src/session/client.rs`:

| Field | Type | Description |
|-------|------|-------------|
| `mode_stack` | `ModeStack` | Per-client editing mode |
| `pending_keys` | `KeySequence` | Per-client key accumulator |
| `windows` | `WindowLayout` | Per-client cursors (#471) |
| `viewport` | `Viewport` | Per-client terminal size |
| `selection` | `Option<ClientSelection>` | Per-client visual selection |
| `extensions` | `ExtensionMap` | Per-client module state (#477) |

### Why This Split?

1. **Mechanism vs Policy**: Driver defines WHAT (types), server decides HOW (ownership)
2. **Per-client isolation**: Client A's `5j` doesn't affect Client B
3. **Shared efficiency**: Compositor, buffer IDs shared across clients

### Access Patterns

```rust
// Shared state access
let home_mode = state.driver_session.shared.home_mode();
let active_buffer = state.driver_session.shared.active_buffer();

// Per-client state access (via server Session)
let editing_state = session.client_state(client_id);
let mode = editing_state.current_mode();
let cursor = editing_state.windows.focused().map(|w| w.cursor);
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
