# Server Mode

Reovim can run as a gRPC server for programmatic control, enabling integration with external tools, IDEs, and automation scripts.

## Quick Start

```bash
# Start server on default port (12521)
reovim server

# Connect with CLI client
reovim cli keys 'iHello<Esc>'

# Connect with TUI client
reovim tui
```

## Transport Options

### TCP (Default)

```bash
# Default: 127.0.0.1:12521
reovim server

# Custom port
reovim server --tcp 9000
```

### Unix Socket

```bash
reovim server --socket /tmp/reovim.sock
```

### Stdio

For process piping and subprocess communication:

```bash
reovim server --stdio
```

Note: `--stdio` mode always exits when the connection closes (one-shot mode).

## Multi-Instance Support

Multiple reovim servers can run concurrently on the same machine.

### Port Fallback

When the default port (12521) is in use, the server automatically tries:
- 12522, 12523, ... up to 12530

The server prints the bound port to stderr on startup:
```
Listening on 127.0.0.1:12522
```

### Port File Discovery

Each server writes a port file for discovery:
```
~/.local/share/reovim/servers/<pid>.port
```

Port files are automatically removed when the server exits cleanly.

### Listing Running Servers

```bash
$ reovim cli list
127.0.0.1:12521 (pid: 123456)
127.0.0.1:12522 (pid: 123789)
```

### Auto-Discovery

When using CLI/TUI without specifying a server:
- **Single server**: Connects automatically
- **Multiple servers**: Connects to first found
- **No servers**: Returns an error

```bash
# Auto-connect to available server
reovim cli keys 'j'

# Specify server explicitly
reovim cli --tcp 127.0.0.1:12522 keys 'j'
```

## Server Modes

### Persistent Mode (Default)

Server runs indefinitely, accepting multiple sequential connections:

```bash
reovim server
```

Clients can connect, disconnect, and reconnect without restarting the server.

## Server Options Reference

| Flag | Description |
|------|-------------|
| `--tcp <PORT>` | Listen on custom TCP port (default: 12521) |
| `--socket <PATH>` | Listen on Unix socket |
| `--stdio` | Use stdio transport (always one-shot) |

## CLI Client

The built-in CLI client provides command-line access to reovim servers.

### Commands

```bash
# List running servers
reovim cli list

# Inject keys
reovim cli keys 'iHello<Esc>'

# Query state
reovim cli mode                           # Get current mode
reovim cli cursor                         # Get cursor position

# JSON output format
reovim cli --format json mode

# Kill server
reovim cli kill

# Interactive REPL mode
reovim cli -i
```

### Debug Commands

```bash
# Get/set log level dynamically
reovim cli log-level                      # Get current level
reovim cli log-level debug                # Set to debug

# View recent log entries
reovim cli log-tail                       # Last 50 entries
reovim cli log-tail --count 100           # Last 100 entries

# Filter logs
reovim cli log-tail --level warn          # WARN and above
reovim cli log-tail --target runner       # Filter by module
reovim cli log-tail --grep "error"        # Search messages

# Stream logs in real-time (like tail -f)
reovim cli log-tail --follow
reovim cli log-tail --follow --level warn # Stream with filter
```

### Connection Options

```bash
# TCP connection (explicit)
reovim cli --tcp localhost:12521 keys 'j'

# Unix socket connection
reovim cli --socket /tmp/reovim.sock keys 'j'
```

### Interactive REPL

```bash
$ reovim cli -i
reovim> keys iHello<Esc>
ok: true
reovim> mode
Mode: NORMAL
reovim> cursor
Cursor: line 0, column 5
reovim> quit
```

## TUI Client

Connect to a running server with a full terminal UI:

```bash
# Auto-discover and connect
reovim tui

# Connect to specific server
reovim tui --tcp 127.0.0.1:12521
reovim tui --socket /tmp/reovim.sock
```

### TUI Debug Mode

Enable debug mode for TUI diagnostics:

```bash
# Enable debug statusline and frame capture
reovim tui --debug

# Custom log directory
reovim tui --debug --debug-dir /tmp/reovim-debug

# Custom session name
reovim tui --debug --debug-name mysession
```

**Debug Features:**

1. **Statusline**: Shows current time, server address, mode, and module count at the bottom of the screen
   ```
   [26-01-18 12:34:56 KST] [server: 127.0.0.1:12521] [mode: NORMAL] [modules: 5]
   ```

2. **Frame Buffer Capture**: Captures rendered frames every 5 seconds
   - Path: `~/.local/share/reovim/logs/tui/frame-buffer/{name}-{timestamp}.frame`

3. **Session Log**: Records key events (mode changes, resize, etc.)
   - Path: `~/.local/share/reovim/logs/tui/{name}_{start_time}.log`

**TUI Debug Options:**

| Flag | Description |
|------|-------------|
| `--debug` | Enable debug mode (statusline + frame capture) |
| `--debug-dir <DIR>` | Custom log directory (default: `~/.local/share/reovim/logs/tui/`) |
| `--debug-name <NAME>` | Session name for filenames (default: `default`) |

## gRPC Protocol

Reovim uses gRPC v2 protocol for client-server communication. The protocol definitions are in `shared/protocol/proto/reovim/v2/`.

### Available Services

| Service | Purpose |
|---------|---------|
| `InputService` | Key injection (SendKeys) |
| `StateService` | Query mode, cursor, layout, selection |
| `BufferService` | Buffer content access |
| `NotificationService` | Server-to-client streaming |
| `PresenceService` | Multi-client collaboration |
| `SyntaxService` | Syntax token queries |
| `DebugService` | Log level and log tail access |
| `ServerService` | Ping, info, kill |

### Key Methods

| Method | Service | Description |
|--------|---------|-------------|
| `SendKeys` | InputService | Inject key sequence |
| `GetMode` | StateService | Get current mode |
| `GetCursor` | StateService | Get cursor position |
| `GetLayout` | StateService | Get window layout |
| `GetSelection` | StateService | Get visual selection |
| `GetRawContent` | BufferService | Get buffer content |
| `SubscribeNotifications` | NotificationService | Stream notifications |
| `LogTail` | DebugService | Get log entries |
| `Kill` | ServerService | Terminate server |

### Protocol Files

```
shared/protocol/proto/reovim/v2/
├── input.proto          # InputService
├── state.proto          # StateService
├── buffer.proto         # BufferService
├── notification.proto   # NotificationService, payloads
├── presence.proto       # PresenceService
├── syntax.proto         # SyntaxService
├── debug.proto          # DebugService
└── server.proto         # ServerService
```

## Use Cases

### Automated Testing

```bash
#!/bin/bash
# Start server
reovim server &
SERVER_PID=$!
sleep 1

# Run test sequence
reovim cli keys 'iTest content<Esc>'

# Verify mode
if reovim cli mode | grep -q "NORMAL"; then
  echo "PASS"
fi

# Clean up
reovim cli kill
```

### IDE Integration

```python
import socket
import json

def send_keys(keys):
    sock = socket.create_connection(('127.0.0.1', 12521))
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "keys",
        "params": {"keys": keys}
    }
    sock.send(json.dumps(request).encode() + b'\n')
    response = sock.recv(4096)
    sock.close()
    return json.loads(response)
```

### Scripting

```bash
# Navigate and edit
reovim cli keys '100gg'

# Search and replace
reovim cli keys ':%s/foo/bar/g<CR>'

# Save and quit
reovim cli keys ':wq<CR>'
```

## Troubleshooting

### Connection Refused

1. Check if server is running: `reovim cli list`
2. Verify port: Server prints `Listening on <host>:<port>` on startup
3. Check firewall/network settings if connecting remotely

### Multiple Server Conflicts

If you have stale port files:
```bash
# Clean up orphaned port files
rm ~/.local/share/reovim/servers/*.port
```

### Server Not Responding

Check server logs:
```bash
tail -f $(ls -t ~/.local/share/reovim/reovim-*.log | head -1)
```

## Implementation Details

### Default Port

The default port `12521` is derived from ASCII: `'r'×100 + 'e'×10 + 'o' = 114×100 + 101×10 + 111 = 12521`

### Architecture

- gRPC services handle client requests
- `TransportConfig` selects transport: Stdio, UnixSocket, Tcp
- `TransportReader`/`TransportWriter` provide async I/O abstraction
- `TransportListener` accepts connections for socket/TCP
- `ChannelKeySource` injects keys from RPC into the runtime
- `FrameBufferHandle` provides unified capture for all RPC formats

### Source Files

- `server/lib/server/` - Server implementation (gRPC handlers, sessions)
- `clients/tui/` - TUI client implementation
- `clients/cli/` - CLI client implementation
- `shared/protocol/` - gRPC v2 protocol definitions
- `shared/net/` - Network transport layer
