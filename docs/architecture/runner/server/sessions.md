# Sessions and Per-Client State

Sessions manage shared editor state while `EditingState` provides per-client isolation.

## Source Location

- Sessions: `server/lib/server/src/session/` (moved from `runner/` in Phase 8)
- Per-client state: `server/lib/server/src/session/client.rs` (`EditingState`)
- Driver Session: `server/lib/drivers/session/` (shared state + bootstrap)

## Session Architecture (#471)

```
┌─────────────────────────────────────────────────────────────┐
│  Session "default"                                           │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ SessionState (shared)                                   │ │
│  │ ├── driver_session: DriverSession                      │ │
│  │ │   ├── extensions (module state)                      │ │
│  │ │   ├── active_buffer                                  │ │
│  │ │   ├── terminal_size                                  │ │
│  │ │   ├── mode_stack (BOOTSTRAP ONLY - deprecated)       │ │
│  │ │   └── windows (BOOTSTRAP ONLY - deprecated)          │ │
│  │ ├── app: AppState                                      │ │
│  │ │   ├── kernel (buffers, event_bus)                    │ │
│  │ │   ├── undo_registry                                  │ │
│  │ │   └── cmdline                                        │ │
│  │ └── CommandRegistry, KeymapRegistry, ModeRegistry      │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ ClientRegistry (per-client isolation)                   │ │
│  │ ├── Client 1 → EditingState                            │ │
│  │ │   ├── mode_stack (NORMAL)                            │ │
│  │ │   ├── windows (cursor@10:5)                          │ │
│  │ │   ├── viewport (80x24)                               │ │
│  │ │   └── selection (none)                               │ │
│  │ ├── Client 2 → EditingState                            │ │
│  │ │   ├── mode_stack (INSERT)                            │ │
│  │ │   ├── windows (cursor@20:3)                          │ │
│  │ │   ├── viewport (200x50)                              │ │
│  │ │   └── selection (char: 0,0-0,5)                      │ │
│  │ └── Client 3 → EditingState                            │ │
│  │     ├── mode_stack (VISUAL)                            │ │
│  │     ├── windows (cursor@1:0)                           │ │
│  │     ├── viewport (120x40)                              │ │
│  │     └── selection (line: 1-5)                          │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## State Ownership (#471)

### Shared State (driver_session + kernel)

| Field | Location | Description |
|-------|----------|-------------|
| `extensions` | `driver_session` | Module-provided policy state |
| `active_buffer` | `driver_session` | Currently active buffer ID |
| `terminal_size` | `driver_session` | Session-level terminal dimensions |
| `kernel` | `app` | Core kernel services (buffers, events) |
| `undo_registry` | `app` | Per-buffer undo trees |
| `cmdline` | `app` | Command-line mode state |

### Per-Client State (EditingState)

| Field | Location | Description |
|-------|----------|-------------|
| `mode_stack` | `EditingState` | Per-client editing mode (NORMAL/INSERT/VISUAL) |
| `windows` | `EditingState` | Per-client window layout and cursor positions |
| `viewport` | `EditingState` | Per-client terminal dimensions, scroll offset |
| `selection` | `EditingState` | Per-client visual selection |
| `pending_keys` | `EditingState` | Per-client key sequence accumulator |

### Deprecated (Bootstrap Only)

| Field | Location | Note |
|-------|----------|------|
| `mode_stack` | `driver_session` | Use `EditingState.mode_stack` at runtime |
| `windows` | `driver_session` | Use `EditingState.windows` at runtime |

Commands should use `SessionRuntime::new_for_client()` to operate on per-client state.

### Accessing Per-Client State

```rust
// Get per-client state (read-only)
let state = session.client_state(client_id);

// Update per-client state
session.update_client_state(client_id, |editing_state| {
    editing_state.mode_stack.push(insert_mode);
    editing_state.windows.active_mut().map(|w| {
        w.cursor = CursorPosition { line: 10, column: 5 };
    });
});

// Get per-client mode
let mode = session.client_current_mode(client_id);
```

## Per-Client Editing State

Each client maintains independent editing state:

```rust
pub struct EditingState {
    /// Per-client mode stack (NORMAL, INSERT, VISUAL, etc.)
    pub mode_stack: ModeStack,

    /// Per-client pending key sequence
    pub pending_keys: KeySequence,

    /// Per-client window layout and cursors
    pub windows: WindowLayout,

    /// Per-client viewport (terminal size, scroll)
    pub viewport: Viewport,

    /// Per-client visual selection
    pub selection: Option<ClientSelection>,
}
```

### Independent Modes and Cursors

Multiple clients can edit the same buffer with independent modes and cursors:

```
┌─────────────────────────────────────────────────────────────┐
│                    Buffer 1 (file.rs)                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Line 20: fn main() {                                │ ← Client 2 cursor (INSERT mode)
│  │ ...                                                 │
│  │ Line 30:     println!("hello");                     │ ← Client 1 cursor (NORMAL mode)
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

- Client 1 is in NORMAL mode, cursor at line 30
- Client 2 is in INSERT mode, cursor at line 20
- Both see the same buffer content (shared)
- When Client 2 types, Client 1 sees the text appear

### Command Execution with Per-Client State

Commands execute on per-client state using `execute_for_client()`:

```rust
// Execute command with per-client mode and cursor
let (result, changes) = registry.execute_for_client(
    &command_id,
    driver_session,           // Shared state (buffers, extensions)
    &mut editing_state.mode_stack,  // Per-client mode
    &mut editing_state.windows,     // Per-client cursor
    app,
    vfs,
    args,
)?;
```

## Lock Hierarchy

To prevent deadlocks, locks are acquired in strict order:

```
Level 0 (Lock-Free):  ArcSwap<SessionRegistry>
       ↓
Level 1 (Per-Session): RwLock<SessionState>
       ↓
Level 2 (Per-Client):  RwLock<HashMap<ClientId, Client>>
```

**Rule**: Always drop higher-level locks before acquiring lower-level locks.

```rust
// SAFE: client state (L2) → session (L1)
let mode = {
    let clients = session.clients().read();
    clients.get(&client_id)
        .and_then(|c| c.editing_state())
        .map(|s| s.mode_stack.current().clone())
}; // Lock dropped

session.with_state(|state| { ... });  // Now safe to acquire
```

## gRPC Handler Pattern (#471)

gRPC handlers query per-client state when `client_id > 0`:

```rust
async fn get_mode(&self, request: Request<GetModeRequest>) -> Result<Response<GetModeResponse>, Status> {
    let req = request.into_inner();
    let session = self.get_session()?;

    // Per-client state (#471): Query per-client mode when client_id provided
    if req.client_id > 0 {
        let client_id = ClientId::new(req.client_id as usize);
        if let Some(mode) = session.client_current_mode(client_id) {
            return Ok(Response::new(GetModeResponse {
                name: mode.name().to_string(),
                display: mode.name().to_uppercase(),
                is_insert: mode.name().contains("insert"),
            }));
        }
    }

    // Fallback to shared mode (backward compatibility)
    // ...
}
```

## Buffer Close Cleanup

When a buffer is closed, clients viewing it have their state updated:

```rust
clear_client_state_for_closed_buffer(&session, closed_buffer_id);
```

Per-client cursor positions in windows are preserved for potential undo/reload.

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Notifications](./notifications.md) - Buffer-scoped notifications
- [Concurrency Reference](../../contributing/internals/concurrency.md) - Lock patterns
- [Session Model](../../session-model.md) - High-level session architecture
