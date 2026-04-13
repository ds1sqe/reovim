# Runner Architecture (Superseded)

> **Superseded by**: [Server Architecture](../architecture/server/overview.md) and
> [Client Architecture](../architecture/client/overview.md).
> The runner was dissolved in Phase 8. Server and client docs now live separately.

> **Note:** This documentation describes the pre-Phase 8 architecture. The runner has been
> restructured into separate components:
> - Server runtime: `server/lib/server/`
> - TUI client: `clients/tui/`
> - CLI client: `clients/cli/`
> - Main binary: `apps/bin/`
>
> See [Phase 8 Migration Guide](./phase-8-migration.md) for details on the restructure.

The runner was the application layer that orchestrated kernel, drivers, and modules.

## Source Location

The runner has been split into:
- **Server runtime**: `server/lib/server/src/`
- **TUI client**: `clients/tui/src/`
- **CLI client**: `clients/cli/src/`
- **Binary entry**: `apps/bin/src/`

## Components (Phase 8 locations)

| Component | Old Path | New Path |
|-----------|----------|----------|
| Server | `runner/src/server/` | `server/lib/server/src/` |
| CLI Client | `runner/src/client/cli/` | `clients/cli/src/` |
| TUI Client | `runner/src/client/tui/` | `clients/tui/src/` |
| Module Loader | `runner/src/module/` | `server/lib/server/src/registry/` |

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

The runner can operate as a gRPC v2 server:

```bash
# Start server on default port (12540)
reovim server

# Connect with CLI client
reovim cli keys 'iHello<Esc>'

# Connect with TUI client
reovim tui
```

See [Server Mode Reference](../user-guide/server-mode.md) for complete usage.

## Related Documents

- [Server Architecture](../architecture/server/overview.md) - RPC server details
- [Client Architecture](../architecture/client/overview.md) - CLI/TUI clients
- [Server Mode Reference](../user-guide/server-mode.md) - Usage guide
