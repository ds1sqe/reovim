# Server Architecture

The gRPC server manages sessions, clients, and request dispatching.

## Source Location

`server/lib/server/src/`

## Components

| Component | File | Purpose |
|-----------|------|---------|
| Server | `mod.rs` | Main server struct, accept loop |
| Session | `session/` | Session state management |
| Client | `session/client.rs` | Per-client state, viewport |
| gRPC Handlers | `grpc/` | Request routing, 12 service handlers |
| Transport | `shared/net/` | Network transport layer (separate crate) |
| Notification | `grpc/notification.rs` | Broadcast to clients |

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
│  │ ClientDirectory                                      │   │
│  │ ├── Client 1 (viewport, writer)                     │   │
│  │ ├── Client 2 (viewport, writer)                     │   │
│  │ └── Client N (viewport, writer)                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ gRPC Handlers (grpc/)                               │   │
│  │ └── 12 service handlers (input, state, buffer, ...)  │   │
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

### gRPC v2 Protocol

gRPC v2 protocol defined in `shared/protocol/` (.proto files).

See [RPC Protocol Documentation](./rpc-protocol.md) for message formats.

## Related Documents

- [Sessions](./sessions.md) - Session and viewport management
- [Transport](./transport.md) - Transport layer (shared/net/)
- [RPC Protocol](./rpc-protocol.md) - gRPC v2 protocol spec
- [Notifications](./notifications.md) - Client notifications
