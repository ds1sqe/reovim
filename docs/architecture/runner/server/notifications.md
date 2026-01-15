# Notifications

Server notifications inform clients of state changes.

## Source Location

`runner/src/server/notification.rs`

## Notification Scoping

Notifications are scoped based on relevance:

| Scope | Description | Example |
|-------|-------------|---------|
| Session-wide | All clients in session | Mode change |
| Buffer-scoped | Clients viewing specific buffer | Cursor moved |
| Except-sender | All clients except triggering one | Buffer modified |

## NotificationBroadcaster

```rust
pub struct NotificationBroadcaster;

impl NotificationBroadcaster {
    /// Broadcast to ALL clients in session
    pub async fn broadcast_to_session(session: &Session, notification: &str);

    /// Broadcast to all clients EXCEPT one
    pub async fn broadcast_except(session: &Session, notification: &str, except: ClientId);

    /// Broadcast to clients viewing a specific buffer
    pub async fn broadcast_to_buffer(session: &Session, buffer_id: BufferId, notification: &str);
}
```

## Buffer-Scoped Notifications

Only clients viewing a buffer receive buffer-specific notifications:

```
┌──────────────────────────────────────────────────────────────┐
│  Session                                                      │
│  ├── Client 1 (viewing Buffer A) ← Receives notification     │
│  ├── Client 2 (viewing Buffer B) ← Does NOT receive          │
│  └── Client 3 (viewing Buffer A) ← Receives notification     │
└──────────────────────────────────────────────────────────────┘

NotificationBroadcaster::broadcast_to_buffer(&session, buffer_a, "cursor/moved");
// Only Client 1 and Client 3 receive this notification
```

## Lock Safety

The broadcaster briefly acquires viewport locks to check active buffers, but drops them before I/O:

```rust
for client in session.clients().iter() {
    let is_viewing = {
        let viewport = client.viewport().read().await;
        viewport.active_buffer == Some(buffer_id)
    }; // Lock dropped before I/O

    if is_viewing {
        client.send_line(notification).await;
    }
}
```

## Notification Types

### Session-Wide

```json
{"jsonrpc":"2.0","method":"mode/changed","params":{"mode":"Insert"}}
```

Sent to all clients when the editor mode changes.

### Buffer-Scoped

```json
{"jsonrpc":"2.0","method":"cursor/moved","params":{"line":10,"column":5,"buffer_id":1}}
```

Sent only to clients viewing the affected buffer.

### Except-Sender

```json
{"jsonrpc":"2.0","method":"buffer/modified","params":{"buffer_id":1}}
```

Sent to all clients except the one that made the modification (they already know).

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Sessions](./sessions.md) - Per-client viewport architecture
