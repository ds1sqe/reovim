# Server Mode

Reovim can run as a gRPC server for programmatic control, enabling integration with external tools, IDEs, and automation scripts.

## Quick Start

```bash
# Terminal 1: Start server on default port (12540)
reovim server

# Terminal 2: Connect with CLI client
reovim cli --grpc 127.0.0.1:12540 keys 'iHello<Esc>' --client 1

# Terminal 3: Connect with TUI client
reovim tui --grpc 127.0.0.1:12540
```

## Transport Options

### gRPC (Default, Recommended)

```bash
# Default: 127.0.0.1:12540
reovim server

# Custom gRPC port
reovim server --grpc 9000
```

### TCP (Legacy)

```bash
# Custom TCP port
reovim server --tcp 9000
```

### Unix Socket

```bash
reovim server --socket /tmp/reovim.sock
```

## Multi-Instance Support

Multiple reovim servers can run concurrently on the same machine.

### Port Fallback

When the default port (12540) is in use, the server automatically tries:
- 12541, 12542, ... up to 12549

The server prints the bound port to stderr on startup:
```
Listening on 127.0.0.1:12541
```

### Named Sessions

Use `--session` to create named editing contexts:

```bash
reovim server --session project-a
reovim server --session project-b --grpc 12541
```

## Multi-Client Architecture

Reovim supports multiple clients connected to the same server session (tmux-like model).

### Joining a Session

Clients join via `PresenceService.Join` and receive:
- A unique `client_id` for identification
- A `session_token` for authentication

All subsequent RPC calls include the token via `x-reovim-token` metadata header.

### Per-Client State

Each client has isolated:
- Cursor position
- Mode state
- Visual selection
- Viewport
- Register bank

Shared across clients:
- Buffer content
- File system state
- Module configuration

### Client Management

```bash
# List connected clients
reovim cli --grpc 127.0.0.1:12540 clients

# Target a specific client
reovim cli --grpc 127.0.0.1:12540 mode --client 1
reovim cli --grpc 127.0.0.1:12540 cursor --client 2
```

## Server Options Reference

| Flag | Description |
|------|-------------|
| `--grpc <PORT>` | Listen on gRPC port (default: 12540) |
| `--tcp <PORT>` | Listen on legacy TCP port |
| `--socket <PATH>` | Listen on Unix socket |
| `--session <NAME>` | Named session identifier |
| `--instance <NAME>` | Named instance identifier |

## CLI Client

The built-in CLI client provides command-line access to reovim servers.

### Commands

```bash
# Inject keys (requires --client)
reovim cli --grpc 127.0.0.1:12540 keys 'iHello<Esc>' --client 1

# Query state (requires --client)
reovim cli --grpc 127.0.0.1:12540 mode --client 1
reovim cli --grpc 127.0.0.1:12540 cursor --client 1

# List clients
reovim cli --grpc 127.0.0.1:12540 clients

# JSON output format
reovim cli --grpc 127.0.0.1:12540 --format json mode --client 1

# Health check
reovim cli --grpc 127.0.0.1:12540 ping
```

### Debug Commands

```bash
# View recent log entries
reovim cli --grpc 127.0.0.1:12540 log-tail
reovim cli --grpc 127.0.0.1:12540 log-tail --count 100

# Filter logs
reovim cli --grpc 127.0.0.1:12540 log-tail --level warn
reovim cli --grpc 127.0.0.1:12540 log-tail --target runner
reovim cli --grpc 127.0.0.1:12540 log-tail --grep "error"
```

See [CLI Reference](./cli-reference.md) for the complete command list.

## gRPC Protocol

Reovim uses gRPC v2 protocol (Protocol Buffers over HTTP/2) for client-server communication. The protocol definitions are in `shared/protocol/proto/reovim/v2/` (13 proto files).

### Available Services

| Service | Proto File | Purpose |
|---------|-----------|---------|
| `InputService` | `input.proto` | Key injection |
| `StateService` | `state.proto` | Mode, cursor, layout, selection, screen capture, registers |
| `BufferService` | `buffer.proto` | Buffer content, annotations, file I/O |
| `EditorService` | `editor.proto` | Resize, quit, active buffer |
| `NotificationService` | `notification.proto` | Server streaming (17 event types) |
| `ServerService` | `server.proto` | Ping, info, kill |
| `DebugService` | `debug.proto` | Log tail, debug queries (no auth required) |
| `ModuleService` | `module.proto` | Module lifecycle (list, load, unload, reload) |
| `CommandService` | `command.proto` | Command search and argument completion |
| `SyntaxService` | `syntax.proto` | Syntax tokens and language info |
| `PresenceService` | `presence.proto` | Multi-client presence and collaboration |
| `ExtensionService` | `extension.proto` | Extension state queries |

### Streaming RPCs

Three services use server streaming for real-time updates:

| Service | RPC | Purpose |
|---------|-----|---------|
| `NotificationService` | `Subscribe` | Editor state changes (17 event types) |
| `SyntaxService` | `StreamTokens` | Syntax token updates on buffer edits |
| `PresenceService` | `StreamPresence` | Peer presence changes (join/leave/update) |

### Authentication

After `PresenceService.Join`, clients receive a `session_token`. All authenticated requests include:

```
x-reovim-token: <session_token>
```

The `DebugService` bypasses authentication, allowing the stateless CLI to target specific clients without joining.

### Protocol Files

```
shared/protocol/proto/reovim/v2/
├── common.proto         # Shared types (Position, Selection, WindowRect)
├── input.proto          # InputService
├── state.proto          # StateService
├── buffer.proto         # BufferService
├── editor.proto         # EditorService
├── notification.proto   # NotificationService + 17 payload types
├── server.proto         # ServerService
├── debug.proto          # DebugService
├── module.proto         # ModuleService
├── command.proto        # CommandService
├── syntax.proto         # SyntaxService
├── presence.proto       # PresenceService
└── extension.proto      # ExtensionService
```

## Use Cases

### Automated Testing

```bash
#!/bin/bash
# Start server
reovim server --grpc 12530 &
SERVER_PID=$!
sleep 1

# Start headless TUI
reovim tui --grpc 127.0.0.1:12530 --headless &
TUI_PID=$!
sleep 1

# Run test sequence
reovim cli --grpc 127.0.0.1:12530 keys 'iTest content<Esc>' --client 1

# Verify mode
if reovim cli --grpc 127.0.0.1:12530 mode --client 1 | grep -q "NORMAL"; then
  echo "PASS"
fi

# Clean up
kill $TUI_PID $SERVER_PID
```

### IDE Integration (gRPC)

```python
import grpc
from reovim.v2 import input_pb2, input_pb2_grpc

channel = grpc.insecure_channel('127.0.0.1:12540')
stub = input_pb2_grpc.InputServiceStub(channel)

# Inject keys (token set via metadata)
metadata = [('x-reovim-token', session_token)]
request = input_pb2.SendKeysRequest(keys='iHello<Esc>')
response = stub.SendKeys(request, metadata=metadata)
```

### Scripting

```bash
# Navigate and edit
reovim cli --grpc 127.0.0.1:12540 keys '100gg' --client 1

# Search and replace
reovim cli --grpc 127.0.0.1:12540 keys ':%s/foo/bar/g<CR>' --client 1

# Save and quit
reovim cli --grpc 127.0.0.1:12540 keys ':wq<CR>' --client 1
```

## Troubleshooting

### Connection Refused

1. Check if server is running: look for the `Listening on` message in server stderr
2. Verify port: `reovim cli --grpc 127.0.0.1:12540 ping`
3. Check firewall/network settings if connecting remotely

### Server Not Responding

Check server logs via the debug service:
```bash
reovim cli --grpc 127.0.0.1:12540 log-tail --level warn
```

## Implementation Details

### Default Port

The default gRPC port is `12540`, with fallback range `12541-12549`.

### Architecture

- gRPC services handle client requests via `tonic`
- `TransportConfig` selects transport: UnixSocket, Tcp, gRPC
- Token-based authentication via `x-reovim-token` header
- Per-client state isolation with shared buffer content
- Notification streaming via `tokio::sync::broadcast` channel

### Source Files

- `server/lib/server/` - Server implementation (gRPC handlers, sessions)
- `clients/tui/` - TUI client implementation
- `clients/cli/` - CLI client implementation
- `shared/protocol/` - gRPC v2 protocol definitions (13 proto files)
- `shared/net/` - Network transport layer

## Related Documents

- [CLI Reference](./cli-reference.md) - Complete CLI command reference
- [Frame Capture](./frame-capture.md) - Frame capture for testing/debugging
- [gRPC Protocol](../architecture/runner/server/rpc-protocol.md) - Full protocol specification
- [Notifications](../architecture/runner/server/notifications.md) - Notification payload types
- [Sessions](../architecture/runner/server/sessions.md) - Per-client state architecture
