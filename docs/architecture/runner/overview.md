# Runner Architecture

The runner (`runner/`) is the application layer that orchestrates kernel, drivers, and modules.

## Source Location

`runner/src/`

## Components

| Component | Path | Purpose | Documentation |
|-----------|------|---------|---------------|
| Server | `runner/src/server/` | RPC server, sessions, event loop | [server/overview.md](./server/overview.md) |
| Client | `runner/src/client/` | CLI, TUI clients | [client/overview.md](./client/overview.md) |
| Buffer Manager | `runner/src/buffer_manager.rs` | Buffer lifecycle management | - |
| Module Loader | `runner/src/module/` | Dynamic module loading | - |

## Layer Position

```
┌─────────────────────────────────────────────────────────────┐
│  Runner (application orchestration)  ← YOU ARE HERE         │
│  ├── Server (RPC, sessions, viewports)                      │
│  ├── Client (CLI, TUI)                                      │
│  └── Module Loader (hot-reload)                             │
├─────────────────────────────────────────────────────────────┤
│  Modules (policy)                                           │
├─────────────────────────────────────────────────────────────┤
│  Drivers (mechanism services)                               │
├─────────────────────────────────────────────────────────────┤
│  Kernel (core primitives)                                   │
└─────────────────────────────────────────────────────────────┘
```

## Event-Driven Architecture

The runner follows an **emit + process** pattern:

```rust
loop {
    // 1. Read (blocking)
    let key = read_key();

    // 2. Emit (non-blocking)
    bus.emit(KeyPressEvent { key, session_id, client_id });

    // 3. Process (handlers execute)
    bus.process_queue();

    // 4. Render (if requested)
    if render_requested.take() {
        render();
    }
}
```

### Thin Runner Philosophy

The runner is a **thin dispatcher**, not a fat coordinator:

| Before (Fat) | After (Thin) |
|--------------|--------------|
| ~500 lines `AppStateRuntime` | Deleted |
| Calculation in event loop | Handlers do calculation |
| Tight coupling to resolvers | Loose coupling via EventBus |
| Hard to test | Mock EventBus for testing |

The runner's only job: **dispatch events and render results**.

## Server Mode

The runner can operate as a JSON-RPC 2.0 server:

```bash
# Start server on default port (12521)
reovim server

# Connect with CLI client
reovim cli keys 'iHello<Esc>'

# Connect with TUI client
reovim tui
```

See [Server Mode Reference](../user-guide/server-mode.md) for complete usage.

## Related Documents

- [Server Architecture](./server/overview.md) - RPC server details
- [Client Architecture](./client/overview.md) - CLI/TUI clients
- [Server Mode Reference](../user-guide/server-mode.md) - Usage guide
