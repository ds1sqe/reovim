# tmux-like Session Architecture

reovim follows a tmux-inspired session model for multi-client editing.

## Concepts

### Session
- Named editing context (e.g., "default", "project-a")
- Contains shared state: buffers, mode_stack, windows, extensions
- Multiple clients can attach to the same session
- Terminal size follows most recent resize

### Client
- Individual connection to the server
- Identified by `ClientId` (numeric, auto-generated)
- Can attach/detach from sessions
- Has own viewport (cursor position, scroll offset)

## Session/Client Diagram

```
┌─────────────────────────────────────────────────┐
│  reovim server                                  │
│                                                 │
│  Session: "default" (SessionId)                 │
│  ├── Shared State (driver::Session):            │
│  │   ├── mode_stack                             │
│  │   ├── pending_keys                           │
│  │   ├── active_buffer                          │
│  │   └── extensions                             │
│  │                                              │
│  └── Attached Clients:                          │
│      ├── ClientId(1) - /dev/pts/0, 120x40       │
│      └── ClientId(2) - /dev/pts/1, 80x24        │
└─────────────────────────────────────────────────┘
```

## Type Mapping (Clarified)

| Concept | Type | Layer | Description |
|---------|------|-------|-------------|
| Session Name | `SessionId(Arc<str>)` | runner | Named editing context (e.g., "default") |
| Session State | `SessionState` | runner | Contains `driver::Session` (SSOT) |
| Client ID | `ClientId(usize)` | driver | Connection identifier (auto-generated) |
| Client State | `ClientViewport` | runner | Per-client viewport, cursor, scroll |
| Window | `WindowId(usize)` | kernel | Window identifier |
| Buffer | `BufferId(usize)` | kernel | Buffer identifier |

**Important distinction:**
- `SessionId` is a **name** (string) - identifies which session
- `driver::Session` holds the **state** - mode, pending keys, extensions
- `ClientId` is a **numeric ID** - identifies which connection

## Multi-Client Example

Two terminals attach to the same session:

1. Client A connects → `ClientId(1)`, attaches to `SessionId("default")`
2. Client B connects → `ClientId(2)`, attaches to `SessionId("default")`
3. Both see the same buffers and mode
4. Each has independent cursor position and viewport
5. When A types `ihello<Esc>`, B sees the text appear

## State Ownership

### Shared (driver::Session)
- Mode stack (all clients in same mode)
- Pending keys (keymap accumulator)
- Extensions (module state)

### Per-Client (ClientViewport)
- Terminal dimensions
- Active buffer (can differ)
- Cursor position per buffer
- Scroll offset

## Related Documents

- [Sessions and Viewports](./runner/server/sessions.md) - Implementation details
- [Driver Overview](./drivers/overview.md) - Driver layer architecture
- [Session Driver](./drivers/session/overview.md) - Session driver details
- [Type Layers](./type-layers.md) - ID type conventions
