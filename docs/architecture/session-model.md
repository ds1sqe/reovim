# tmux-like Session Architecture

reovim follows a tmux-inspired session model for multi-client editing.

## Concepts

### Session
- Named editing context (e.g., "default", "project-a")
- Contains shared state: buffers, extensions
- Multiple clients can attach to the same session
- Each client has independent editing state (mode, cursor, selection)

### Client
- Individual connection to the server
- Identified by `ClientId` (numeric, auto-generated)
- Can attach/detach from sessions
- Has own `EditingState` (mode, cursor, viewport, selection)

## Session/Client Diagram

```
┌─────────────────────────────────────────────────┐
│  reovim server                                  │
│                                                 │
│  Session: "default" (SessionId)                 │
│  ├── Shared State:                              │
│  │   ├── buffers (content shared by all)        │
│  │   ├── extensions (module state)              │
│  │   └── active_buffer                          │
│  │                                              │
│  └── Attached Clients:                          │
│      ├── ClientId(1) - EditingState             │
│      │   ├── mode_stack (NORMAL)                │
│      │   ├── cursor (10, 5)                     │
│      │   ├── viewport (120x40)                  │
│      │   └── selection (none)                   │
│      └── ClientId(2) - EditingState             │
│          ├── mode_stack (INSERT)                │
│          ├── cursor (20, 3)                     │
│          ├── viewport (80x24)                   │
│          └── selection (none)                   │
└─────────────────────────────────────────────────┘
```

## Type Mapping

| Concept | Type | Layer | Description |
|---------|------|-------|-------------|
| Session Name | `SessionId(Arc<str>)` | server | Named editing context (e.g., "default") |
| Session State | `SessionState` | server | Contains driver::Session + registries |
| Client ID | `ClientId(usize)` | server | Connection identifier (auto-generated) |
| Client State | `EditingState` | server | Per-client mode, cursor, viewport, selection |
| Window | `WindowId(usize)` | kernel | Window identifier |
| Buffer | `BufferId(usize)` | kernel | Buffer identifier |

**Note:** The kernel also has a `SessionId(usize)` type for internal session tracking. This is distinct from the server-layer `SessionId(Arc<str>)` which uses named string identifiers.

**Important distinction:**
- `SessionId` is a **name** (string) - identifies which session
- `driver::Session` holds **shared state** - buffers, extensions (bootstrap-only for mode/windows)
- `ClientId` is a **numeric ID** - identifies which connection
- `EditingState` holds **per-client state** - mode, cursor, selection, viewport

## Multi-Client Example

Two terminals attach to the same session:

1. Client A connects → `ClientId(1)`, attaches to `SessionId("default")`
2. Client B connects → `ClientId(2)`, attaches to `SessionId("default")`
3. Both see the same buffer content (shared)
4. Each has independent mode, cursor, and selection (per-client)
5. When A types `ihello<Esc>`:
   - A enters INSERT, types "hello", returns to NORMAL
   - B stays in NORMAL mode (independent)
   - B sees the text "hello" appear (shared buffer)

## State Ownership (#471)

### Shared (driver::Session + kernel)
- **Buffer content** - All clients see same text
- **Extensions** - Module state (e.g., LSP connections)
- **Active buffer** - Which buffer is being edited

### Per-Client (EditingState)
- **Mode stack** - Each client has independent mode (NORMAL/INSERT/VISUAL)
- **Cursor position** - Independent cursor per client
- **Selection** - Visual mode selection is per-client
- **Viewport** - Terminal dimensions, scroll offset
- **Pending keys** - Key sequence accumulator

### Deprecated for Runtime (driver::Session)
The following fields in `driver::Session` are **bootstrap-only** (#471):
- `mode_stack` - Use `EditingState.mode_stack` at runtime
- `windows` - Use `EditingState.windows` at runtime

Commands should use `SessionRuntime::new_for_client()` to operate on per-client state.

## Related Documents

- [Sessions and Viewports](./runner/server/sessions.md) - Implementation details
- [Driver Overview](./drivers/overview.md) - Driver layer architecture
- [Session Driver](./drivers/session/overview.md) - Session driver details
- [Type Layers](./type-layers.md) - ID type conventions
