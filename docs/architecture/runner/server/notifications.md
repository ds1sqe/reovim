# Notifications

Server-to-client streaming notifications for real-time state updates.

## Source Location

- Proto: `shared/protocol/proto/reovim/v2/notification.proto`
- Implementation: `server/lib/server/src/grpc/notification.rs`

## Transport

Clients subscribe via `NotificationService.Subscribe`, which returns a gRPC server stream of `Notification` messages.

```protobuf
service NotificationService {
  rpc Subscribe(SubscribeRequest) returns (stream Notification);
}
```

### Filtering

Clients can filter events by type in `SubscribeRequest.event_types`. An empty list subscribes to all events.

```protobuf
message SubscribeRequest {
  repeated string event_types = 1;  // empty = all events
}
```

## Notification Payloads

Each `Notification` contains a `oneof payload` with one of 17 typed payloads:

### Editor State

| Payload | Key Fields | Description |
|---------|-----------|-------------|
| `mode_changed` | `name`, `display`, `is_insert`, `client_id` | Editor mode changed |
| `cursor_moved` | `window_id`, `position`, `client_id` | Cursor position changed |
| `buffer_modified` | `buffer_id`, `change` (optional) | Buffer content changed |
| `layout_changed` | `focused_window_id`, `windows[]`, `client_id` | Window layout changed |
| `option_changed` | `name`, value (`bool`/`int`/`string`) | Editor option changed |
| `selection_changed` | `window_id`, `has_selection`, `selection`, `visual_mode`, `client_id` | Visual selection changed |
| `buffer_list_changed` | `action` ("added"/"removed"/"modified"), `buffer_id` | Buffer list changed |
| `render_complete` | `frame_id` | Frame ready signal for TUI |
| `detach` | `reason` | Client should disconnect |

### Viewport

| Payload | Key Fields | Description |
|---------|-----------|-------------|
| `viewport_updated` | `viewport_id`, `top_line`, `left_col`, `cursor_line`, `cursor_col` | Incremental scroll/cursor tracking (Phase 11.1) |

### Capture Relay

Screen capture flows: CLI -> Server -> TUI -> Server -> CLI.

| Payload | Key Fields | Description |
|---------|-----------|-------------|
| `capture_request` | `request_id`, `format`, `target_client_id` | Server requests TUI to capture frame |
| `capture_response` | `request_id`, `width`, `height`, `format`, `content` | TUI responds with captured frame |
| `resize_request` | `width`, `height`, `target_client_id` | CLI tells TUI to resize |

### Presence (Multi-Client)

| Payload | Key Fields | Description |
|---------|-----------|-------------|
| `presence_joined` | `ClientPresence` | New client joined session |
| `presence_left` | `client_id`, `display_name` | Client left session |
| `presence_updated` | `ClientPresence` | Client presence state changed |

### Extensions

| Payload | Key Fields | Description |
|---------|-----------|-------------|
| `extension_updated` | `kind`, `data` (JSON), `client_id` | Extension state changed (#514) |

## Per-Client Scoping

Notifications are broadcast to all subscribers via a `tokio::sync::broadcast` channel (capacity 256). Per-client scoping is handled via `client_id` fields in the payloads:

- `mode_changed.client_id` - which client's mode changed
- `cursor_moved.client_id` - which client's cursor moved
- `selection_changed.client_id` - which client's selection changed
- `layout_changed.client_id` - which client triggered the layout change

Clients filter for their own `client_id` to show their own state, or use other client IDs to render peer cursors/selections.

## Auto-Cleanup on Disconnect

The notification stream doubles as a heartbeat. When a client's stream drops (disconnect, crash), the server automatically:
1. Revokes the client's session token
2. Removes the client from the client map
3. Broadcasts `presence_left` to remaining clients

## Related Documents

- [gRPC Protocol](./rpc-protocol.md) - Full service reference
- [Sessions](./sessions.md) - Per-client state architecture
