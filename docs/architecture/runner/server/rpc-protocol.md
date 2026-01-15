# RPC Protocol

JSON-RPC 2.0 protocol for client-server communication.

## Source Location

`lib/protocol/src/`

## Message Format

### Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "keys",
  "params": { "keys": "iHello<Esc>" }
}
```

### Response (Success)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "mode": "Normal",
    "cursor": { "line": 0, "column": 5 }
  }
}
```

### Response (Error)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid request"
  }
}
```

### Notification (No Response)

```json
{
  "jsonrpc": "2.0",
  "method": "mode/changed",
  "params": { "mode": "Insert" }
}
```

## Available Methods

### Input Methods

| Method | Params | Description |
|--------|--------|-------------|
| `keys` | `{ keys: string }` | Inject key sequence |

### State Query Methods

| Method | Params | Description |
|--------|--------|-------------|
| `state/screen` | `{}` | Get viewport state (dimensions, active buffer) |
| `state/cursor` | `{}` | Get cursor position |
| `state/mode` | `{}` | Get current mode |

### Editor Methods

| Method | Params | Description |
|--------|--------|-------------|
| `editor/resize` | `{ width, height }` | Set terminal dimensions |
| `editor/set_active_buffer` | `{ buffer_id }` | Switch active buffer |

### Buffer Methods

| Method | Params | Description |
|--------|--------|-------------|
| `buffer/open_file` | `{ path }` | Open file into buffer |
| `buffer/list` | `{}` | List all buffers |

### Server Methods

| Method | Params | Description |
|--------|--------|-------------|
| `server/kill` | `{}` | Terminate server |

## Server Notifications

Servers send notifications for state changes:

| Notification | Params | Scope |
|--------------|--------|-------|
| `mode/changed` | `{ mode }` | Session-wide |
| `cursor/moved` | `{ line, column }` | Buffer-scoped |
| `buffer/modified` | `{ buffer_id }` | Buffer-scoped |

See [Notifications](./notifications.md) for scoping details.

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Notifications](./notifications.md) - Notification routing
- [Server Mode Reference](../../user-guide/server-mode.md) - Usage guide
