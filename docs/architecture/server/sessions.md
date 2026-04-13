# Sessions and Per-Client State

Sessions manage shared editor state while `EditingState` provides per-client isolation.

## Source Location

- Sessions: `server/lib/server/src/session/`
- Per-client state: `server/lib/server/src/session/client.rs` (`EditingState`)
- Driver Session: `server/lib/drivers/session/` (shared state + bootstrap)

## Session Architecture (#471, #491)

```
┌─────────────────────────────────────────────────────────────┐
│  Session "default"                                           │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ SessionState (shared)                                   │ │
│  │ ├── driver_session: DriverSession                      │ │
│  │ │   └── shared: SessionShared                          │ │
│  │ │       ├── compositor                                 │ │
│  │ │       ├── home_mode (for new client init)            │ │
│  │ │       └── global_marks (A-Z marks)                   │ │
│  │ ├── app: AppState                                      │ │
│  │ │   ├── kernel (buffers, event_bus)                    │ │
│  │ │   ├── undo_registry                                  │ │
│  │ │   ├── cmdline                                        │ │
│  │ │   └── extensions (session-wide module state)         │ │
│  │ └── CommandRegistry, KeymapRegistry, ModeRegistry      │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ clients: HashMap<ClientId, Client> (per-client state)   │ │
│  │ ├── Client 1 → EditingState                            │ │
│  │ │   ├── mode_stack (NORMAL)                            │ │
│  │ │   ├── windows (cursor@10:5)                          │ │
│  │ │   ├── viewport (80x24)                               │ │
│  │ │   ├── extensions (per-client module state)           │ │
│  │ │   ├── selection (none)                               │ │
│  │ │   ├── compositor (window layout)                     │ │
│  │ │   ├── registers (a-z, unnamed)                       │ │
│  │ │   ├── clipboard_history (ring 0-9)                   │ │
│  │ │   └── local_marks (a-z)                              │ │
│  │ ├── Client 2 → EditingState                            │ │
│  │ │   ├── mode_stack (INSERT)                            │ │
│  │ │   ├── windows (cursor@20:3)                          │ │
│  │ │   ├── viewport (200x50)                              │ │
│  │ │   ├── extensions (per-client module state)           │ │
│  │ │   ├── selection (char: 0,0-0,5)                      │ │
│  │ │   ├── compositor (window layout)                     │ │
│  │ │   ├── registers (a-z, unnamed)                       │ │
│  │ │   ├── clipboard_history (ring 0-9)                   │ │
│  │ │   └── local_marks (a-z)                              │ │
│  │ └── Client 3 → EditingState                            │ │
│  │     ├── mode_stack (VISUAL)                            │ │
│  │     ├── windows (cursor@1:0)                           │ │
│  │     ├── viewport (120x40)                              │ │
│  │     ├── extensions (per-client module state)           │ │
│  │     ├── selection (line: 1-5)                          │ │
│  │     ├── compositor (window layout)                     │ │
│  │     ├── registers (a-z, unnamed)                       │ │
│  │     ├── clipboard_history (ring 0-9)                   │ │
│  │     └── local_marks (a-z)                              │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## State Ownership (#471, #491)

### Shared State (driver_session.shared + app)

| Field | Location | Description |
|-------|----------|-------------|
| `home_mode` | `driver_session.shared` | Mode for initializing new clients |
| `compositor` | `driver_session.shared` | Window layout template |
| `global_marks` | `driver_session.shared` | Global marks (A-Z) shared across clients |
| `kernel` | `app` | Core kernel services (buffers, events) |
| `undo_registry` | `app` | Per-buffer undo trees |
| `cmdline` | `app` | Command-line mode state |
| `extensions` | `app` | Session-wide module state |

### Per-Client State (EditingState)

| Field | Location | Description |
|-------|----------|-------------|
| `mode_stack` | `EditingState` | Per-client editing mode (NORMAL/INSERT/VISUAL) |
| `pending_keys` | `EditingState` | Per-client key sequence accumulator |
| `active_buffer` | `EditingState` | Per-client active buffer (migrated from shared in #471) |
| `terminal_size` | `EditingState` | Per-client terminal dimensions (migrated from shared in #471) |
| `windows` | `EditingState` | Per-client window layout and cursor positions |
| `viewport` | `EditingState` | Per-client viewport scroll offset |
| `selection` | `EditingState` | Per-client visual selection |
| `extensions` | `EditingState` | Per-client module state (#477) |
| `compositor` | `EditingState` | Per-client compositor cloned from shared template (#474) |
| `registers` | `EditingState` | Per-client register storage (a-z, A-Z, unnamed) (#515) |
| `clipboard_history` | `EditingState` | Per-client yank/delete history ring for registers 0-9 (#515) |
| `local_marks` | `EditingState` | Per-client local marks (a-z); global marks A-Z shared (#515) |

Commands use `SessionRuntime` which borrows both shared and per-client state.

### Accessing Per-Client State

After #741 the direct `session.client_state()` and `session.update_client_state()` methods
were removed. The correct pattern is to go through `session.clients()`:

```rust
// Get per-client state (read-only) — acquire clients lock, read state
let state = {
    let clients = session.clients().read();
    clients.get(&client_id)
        .and_then(|c| c.editing_state())
        .cloned()
};

// Update per-client state — acquire clients write lock
{
    let mut clients = session.clients().write();
    if let Some(client) = clients.get_mut(&client_id) {
        if let Some(editing_state) = client.editing_state_mut() {
            editing_state.mode_stack.push(insert_mode);
            editing_state.windows.active_mut().map(|w| {
                w.cursor = CursorPosition { line: 10, column: 5 };
            });
        }
    }
}

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

    /// Per-client module extensions (#477) - type-erased storage
    /// for VimSessionState, SearchState, CmdlineState, etc.
    pub extensions: ExtensionMap,

    /// Per-client compositor for window layout (#474) - cloned
    /// from the shared template at join time
    pub compositor: Option<Box<dyn RootCompositor>>,

    /// Per-client register storage (#515) - named registers (a-z/A-Z),
    /// unnamed register (""). System clipboard (+, *) remains shared.
    pub registers: RegisterBank,

    /// Per-client clipboard history ring (#515) - tracks yank/delete
    /// history for numbered registers 0-9
    pub clipboard_history: HistoryRing,

    /// Per-client local marks (a-z) (#515) - global marks (A-Z)
    /// remain shared in KernelContext.global_marks
    pub local_marks: MarkBank,
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
Level 1 (Acquired First):  ClientDirectory (RwLock<HashMap<ClientId, Client>>)
       ↓
Level 2 (Acquired Second): RwLock<SessionState>
```

**Rule**: Always acquire clients (L1) before session state (L2). Never hold a session
state lock while attempting to acquire the clients lock.

This order matches how gRPC handlers actually execute:
- `resolve_key_for_client`: acquires clients first, then session state
- `execute_command_for_client`: acquires clients first, then session state

```rust
// SAFE: clients (L1) acquired first, state (L2) acquired after clients lock is dropped
let mode = {
    let clients = session.clients().read();
    clients.get(&client_id)
        .and_then(|c| c.editing_state())
        .map(|s| s.mode_stack.current().clone())
}; // Clients lock dropped here

session.with_state(|state| { ... });  // Now safe to acquire session state
```

## Token-Based Authentication (#483)

All gRPC RPCs (except `Join()`) require token authentication. The server issues
a `SessionToken` on `Join()` and the client includes it in every subsequent request
via the `x-reovim-token` metadata header.

### TokenRegistry

`TokenRegistry` (`server/lib/server/src/session/token_registry.rs`) maintains a
bidirectional mapping between tokens and client IDs:

```rust
pub struct TokenRegistry {
    maps: RwLock<TokenMaps>,  // forward: token→ClientId, reverse: ClientId→token
}
```

- `register(client_id)` -- generates a 128-bit random token (32 hex chars), returns `SessionToken`
- `resolve(token)` -- O(1) lookup via read lock (hot path, called on every request)
- `revoke(token)` / `revoke_by_client(client_id)` -- removes mapping on Leave/disconnect

### AuthInterceptor

The `AuthInterceptor` (`server/lib/server/src/grpc/auth.rs`) is a tonic interceptor
inserted into every gRPC service:

1. Reads `x-reovim-token` from request metadata
2. Resolves token to `ClientId` via `TokenRegistry` (read lock, O(1))
3. Inserts `ClientId` into request extensions
4. Handlers extract `ClientId` from extensions (not from the request body)

### Client ID Resolution

Two helper functions in `auth.rs` extract the authenticated client:

- `require_client_id(token_client_id)` -- for caller-identity RPCs (`send_keys`, `leave`, etc.).
  Returns `Unauthenticated` if no token was provided.
- `resolve_target_client_id(token_client_id, target_client_id)` -- for state queries
  where the body field selects whose state to return. `target=0` means "self" (uses
  token identity), `target>0` returns that specific client's state.

## ClientRelation Model (#480)

Each `Client` has a `relation` field (`Option<ClientRelation>`) that controls input
routing and state visibility:

```rust
pub enum ClientRelation {
    /// Read-only spectator. Input is ignored, sees target's state.
    Following { target: ClientId },
    /// Bidirectional co-editing. Input goes to target's state.
    Sharing { with: ClientId },
}
```

When `relation` is `None`, the client is independent (input goes to its own state).

### Behavior Matrix

| Relation         | My Input       | I See          | Use Case            |
|------------------|----------------|----------------|---------------------|
| `None`           | goes to self   | my state       | Solo editing        |
| `Following(X)`   | ignored        | X's state      | Spectator/present   |
| `Sharing(X)`     | goes to X      | X's state      | Pair programming    |

### State Transitions

```
Independent <-> Following(B) <-> Sharing(B) <-> Independent
```

Transitions are validated by `Client::try_set_relation()` which returns
`TransitionResult`:

- `Ok` -- transition succeeded
- `RequiresCursorSync { current, target }` -- caller must sync cursor first (Following to Sharing upgrade)
- `TargetNotFound(id)` -- target client does not exist
- `WouldCreateCycle` -- would create A follows B follows A
- `CannotTargetSelf` -- cannot follow/share with self

## Presence Model (Phase 14, #465)

The presence system tracks connected clients for multi-client awareness.

### ClientPresence

```rust
pub struct ClientPresence {
    pub client_id: ClientId,
    pub client_type: String,       // "tui", "android", "web", "cli"
    pub display_name: String,      // "laptop", "phone", "tablet"
    pub buffer_id: Option<usize>,
    pub cursor: (usize, usize),    // (line, column)
    pub visible_lines: (usize, usize), // (start, end exclusive)
    pub mode: String,              // "NORMAL", "INSERT", etc.
    pub sync_mode: SyncMode,
    pub joined_at: SystemTime,
}
```

### SyncMode

```rust
pub enum SyncMode {
    Independent,              // Own cursor, own scroll
    Follow { target: ClientId }, // Follow another client's cursor/scroll
    Present,                  // Normal editing, tagged so others can follow
}
```

### PresenceMap

`PresenceMap` (`server/lib/server/src/session/presence.rs`) is a per-session,
thread-safe (`RwLock<HashMap<ClientId, ClientPresence>>`) map:

| Operation       | Complexity | Notes                    |
|-----------------|------------|--------------------------|
| `join()`        | O(n)       | Collects existing peers  |
| `leave()`       | O(1)       | HashMap remove           |
| `update()`      | O(1)       | Closure-based mutation   |
| `get()`         | O(1)       | HashMap get              |
| `list()`        | O(n)       | Collects all             |
| `followers_of()`| O(n)       | Scans for Follow targets |

## gRPC Handler Pattern (#471, #483)

gRPC handlers authenticate via `x-reovim-token` metadata. The interceptor injects
`ClientId` into request extensions. Handlers use `resolve_target_client_id()` to
determine whose state to query:

```rust
async fn get_mode(&self, request: Request<GetModeRequest>) -> Result<Response<GetModeResponse>, Status> {
    let req = request.into_inner();
    let session = self.get_session()?;

    // Token auth (#483): resolve caller identity from x-reovim-token header.
    // target_client_id=0 means "self" (uses token), >0 targets another client.
    let token_client_id = request.extensions().get::<ClientId>().copied();
    let client_id = resolve_target_client_id(token_client_id, req.target_client_id)?;

    if let Some(mode) = session.client_current_mode(client_id) {
        return Ok(Response::new(GetModeResponse {
            name: mode.name().to_string(),
            display: mode.name().to_uppercase(),
            is_insert: mode.name().contains("insert"),
        }));
    }

    // Fallback to shared mode (backward compatibility)
    // ...
}
```

## Session Authority Decomposition (#741)

The `Session` surface was refactored to extract explicit authority objects, reducing the
footprint of the monolithic session struct and making lock boundaries explicit at the
type level.

### Extracted Authorities

**`ClientDirectory`** (`session/client_directory.rs`) — Owns client membership, the
editing-relation graph, and input-target resolution. Accessed via `session.clients()`.

Key methods:

| Method | Description |
|--------|-------------|
| `add_client_with_state` | Register a new client with its initial editing state |
| `remove_client_with` | Remove a client and run a cleanup closure |
| `get_client` | Read-only access to a client entry |
| `set_client_relation` | Update the `ClientRelation` for a client |
| `find_input_target` | Resolve which client receives input (follows Sharing graph) |
| `connected_client_ids` | Return all currently connected client IDs |
| `client_count` | Number of connected clients |

**`PresenceService`** (`session/presence_service.rs`) — Owns the presence and sync graph.
Wraps `PresenceMap`. Compiled unconditionally. Accessed via `session.presence()`.

**`Session`** remains the composition root (~27 public methods) responsible for
multi-lock coordination between `ClientDirectory`, `PresenceService`, and
`SessionState`. Callers should not bypass the `Session` API to reach inner authorities
directly.

### Updated Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Session "default"                                           │
│  ├── ClientDirectory (RwLock)   ← client membership, input  │
│  │   └── HashMap<ClientId, Client>                          │
│  │       └── EditingState (per-client)                      │
│  ├── PresenceService            ← presence / sync graph     │
│  │   └── PresenceMap (RwLock)                               │
│  └── SessionState (RwLock)      ← shared editor state       │
│      ├── driver_session: DriverSession                      │
│      ├── app: AppState                                      │
│      └── CommandRegistry, KeymapRegistry, ModeRegistry      │
└─────────────────────────────────────────────────────────────┘
```

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Notifications](./notifications.md) - Buffer-scoped notifications
- [Concurrency Reference](../../contributing/internals/concurrency.md) - Lock patterns
- [Session Model](../session-model.md) - High-level session architecture
