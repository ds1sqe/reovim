# Sessions and Viewports

Sessions manage shared editor state while viewports provide per-client independence.

## Source Location

- Sessions: `runner/src/server/session/`
- Viewports: `runner/src/server/client/viewport.rs`

## Session Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Session "default"                                           │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ SessionState (shared)                                   │ │
│  │ ├── KernelContext (buffers, event_bus, registries)     │ │
│  │ ├── ModeId (current mode)                              │ │
│  │ └── CommandRegistry, KeymapRegistry, ...               │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ ClientRegistry                                          │ │
│  │ ├── Client 1 → ClientViewport (80x24, buf1, cursor@10:5)│ │
│  │ ├── Client 2 → ClientViewport (200x50, buf1, cursor@20:3)│
│  │ └── Client 3 → ClientViewport (120x40, buf2, cursor@1:0)│ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Per-Client Viewport

Each client maintains independent viewport state:

```rust
pub struct ClientViewport {
    /// Terminal width in columns
    pub terminal_width: u16,

    /// Terminal height in rows
    pub terminal_height: u16,

    /// Active buffer for this client
    pub active_buffer: Option<BufferId>,

    /// Cursor positions per buffer (restored on buffer switch)
    pub buffer_cursors: HashMap<BufferId, Position>,

    /// Scroll top line
    pub scroll_top: usize,
}
```

### Independent Cursors

Multiple clients can edit the same buffer with independent cursor positions:

```
┌─────────────────────────────────────────────────────────────┐
│                    Buffer 1 (file.rs)                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Line 20: fn main() {                                │ ← Client 2 cursor (20:32)
│  │ ...                                                 │
│  │ Line 30:     println!("hello");                     │ ← Client 1 cursor (30:23)
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

This enables multi-terminal editing like running vim in separate tmux panes.

### Cursor Restoration

When switching buffers, cursor positions are preserved:

```rust
// Client switches from buffer A to B
viewport.set_cursor_for_buffer(buf_a, current_cursor);
viewport.active_buffer = Some(buf_b);
let restored = viewport.cursor_for_buffer(buf_b);  // Restored!
```

## Lock Hierarchy

To prevent deadlocks, locks are acquired in strict order:

```
Level 0 (Lock-Free):  ArcSwap<SessionRegistry>
       ↓
Level 1 (Per-Session): RwLock<SessionState>
       ↓
Level 2 (Per-Client):  RwLock<ClientViewport>
```

**Rule**: Always drop higher-level locks before acquiring lower-level locks.

```rust
// SAFE: viewport (L2) → session (L1)
let buffer_id = {
    let vp = client.viewport().read().await;
    vp.active_buffer
}; // Lock dropped

session.with_state(|state| { ... }).await;  // Now safe to acquire
```

## RPC Context

Handlers receive `RpcContext` with session, client, and viewport access:

```rust
pub struct RpcContext {
    pub session: Arc<Session>,
    pub client_id: ClientId,
    pub client: Arc<Client>,  // Direct viewport access
}

// In a handler:
async fn handle(ctx: &RpcContext) {
    let viewport = ctx.client.viewport().read().await;
    let size = (viewport.terminal_width, viewport.terminal_height);
}
```

## Buffer Close Cleanup

When a buffer is closed, clients viewing it have their `active_buffer` cleared:

```rust
clear_viewports_for_closed_buffer(&session, closed_buffer_id).await;
```

Cursor positions are preserved for potential undo/reload.

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Notifications](./notifications.md) - Buffer-scoped notifications
- [Concurrency Reference](../../contributing/internals/concurrency.md) - Lock patterns
