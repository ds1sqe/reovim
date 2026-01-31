# Server Architecture

The RPC server manages sessions, clients, and request dispatching.

## Source Location

`server/lib/server/src/` (moved from `runner/src/server/` in Phase 8)

## Components

| Component | File | Purpose |
|-----------|------|---------|
| Server | `mod.rs` | Main server struct, accept loop |
| Session | `session/` | Session state management |
| Client | `client/` | Per-client state, viewport |
| RPC Dispatcher | `rpc/` | Request routing, handlers |
| Transport | `transport/` | TCP, Unix socket, stdio |
| Notification | `notification.rs` | Broadcast to clients |

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                       Server                                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ SessionRegistry                                      │   │
│  │ └── Session "default"                               │   │
│  │     └── SessionState (KernelContext + Registries)   │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ClientRegistry                                       │   │
│  │ ├── Client 1 (viewport, writer)                     │   │
│  │ ├── Client 2 (viewport, writer)                     │   │
│  │ └── Client N (viewport, writer)                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ RpcDispatcher                                        │   │
│  │ └── dispatch(request) → handler → response          │   │
│  └─────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────┘
```

## Key Concepts

### Sessions

Sessions hold shared editor state (buffers, modes, registries). Multiple clients can connect to the same session.

See [Sessions Documentation](./sessions.md) for details.

### Viewports

Each client has independent viewport state (terminal size, active buffer, cursor). This enables multi-client editing with different views.

See [Sessions Documentation](./sessions.md#per-client-viewport) for the viewport architecture.

### RPC Protocol

JSON-RPC 2.0 over TCP, Unix socket, or stdio.

See [RPC Protocol Documentation](./rpc-protocol.md) for message formats.

## Related Documents

- [Sessions](./sessions.md) - Session and viewport management
- [Transport](./transport.md) - Transport layer options
- [RPC Protocol](./rpc-protocol.md) - JSON-RPC 2.0 spec
- [Notifications](./notifications.md) - Client notifications
