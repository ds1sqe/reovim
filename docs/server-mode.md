# Server Mode

Reovim can run as a JSON-RPC 2.0 server for programmatic control, enabling integration with external tools, IDEs, and automation scripts.

## Quick Start

```bash
# Start server on default port (12521)
reovim --server

# Start server with a file
reovim --server myfile.txt

# Connect with reo-cli
reo-cli keys 'iHello<Esc>'
```

## Transport Options

### TCP (Default)

```bash
# Default: 127.0.0.1:12521
reovim --server

# Custom port
reovim --listen-tcp 9000

# Custom host and port
reovim --listen-host 0.0.0.0 --listen-tcp 9000
```

### Unix Socket

```bash
reovim --listen-socket /tmp/reovim.sock
```

### Stdio

For process piping and subprocess communication:

```bash
reovim --stdio
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
$ reo-cli list
PID      PORT   ADDRESS
123456   12521  127.0.0.1:12521
123789   12522  127.0.0.1:12522
```

### Auto-Discovery

When using `reo-cli` without specifying a server:
- **Single server**: Connects automatically
- **Multiple servers**: Prompts for selection
- **No servers**: Returns an error

```bash
# Auto-connect to single server
reo-cli keys 'j'

# Specify server explicitly
reo-cli --tcp 127.0.0.1:12522 keys 'j'
```

## Server Modes

### Persistent Mode (Default)

Server runs indefinitely, accepting multiple sequential connections:

```bash
reovim --server
```

Clients can connect, disconnect, and reconnect without restarting the server.

### Test Mode

Server exits when all clients disconnect. Useful for CI/testing:

```bash
reovim --server --test
```

### Dual Output Mode

Render to terminal while serving RPC:

```bash
reovim --server --terminal
```

This allows visual monitoring while the server handles RPC requests.

## Server Options Reference

| Flag | Description |
|------|-------------|
| `--server` | Start in server mode (TCP on 127.0.0.1:12521) |
| `--test` | Exit when all clients disconnect |
| `--stdio` | Use stdio transport (always one-shot) |
| `--listen-socket <PATH>` | Listen on Unix socket |
| `--listen-tcp <PORT>` | Listen on custom TCP port |
| `--listen-host <HOST>` | Bind to specific host (default: 127.0.0.1) |
| `--terminal` | Also render to terminal in server mode |

## reo-cli Client

The `reo-cli` tool provides command-line access to reovim servers.

### Commands

```bash
# List running servers
reo-cli list

# Inject keys and show status
reo-cli keys 'iHello<Esc>'

# Get screen content
reo-cli screen

# Get screen dimensions
reo-cli screen-size

# Kill server
reo-cli kill

# Interactive REPL mode
reo-cli -i
```

### Connection Options

```bash
# TCP connection (explicit)
reo-cli --tcp localhost:12521 keys 'j'

# Unix socket connection
reo-cli --socket /tmp/reovim.sock keys 'j'
```

### Interactive REPL

```bash
$ reo-cli -i
reovim> keys iHello<Esc>
Mode: Normal
Cursor: (0, 5)
reovim> screen
Hello
~
~
reovim> quit
```

## JSON-RPC Protocol

### Request Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "keys",
  "params": { "keys": "iHello<Esc>" }
}
```

### Response Format

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

### Available Methods

| Method | Params | Description |
|--------|--------|-------------|
| `keys` | `{ keys: string }` | Inject key sequence |
| `screen` | `{ format?: string }` | Get screen content |
| `screen_size` | - | Get terminal dimensions |
| `cursor` | - | Get cursor position |
| `mode` | - | Get current mode |
| `kill` | - | Terminate server |

### Screen Formats

```json
// Plain text (default)
{ "method": "screen", "params": { "format": "plain" } }

// Raw ANSI escape sequences
{ "method": "screen", "params": { "format": "ansi" } }

// Cell grid (structured)
{ "method": "screen", "params": { "format": "cells" } }
```

## Use Cases

### Automated Testing

```bash
#!/bin/bash
# Start server in test mode
reovim --server --test testfile.txt &
sleep 1

# Run test sequence
reo-cli keys 'iTest content<Esc>'
reo-cli keys ':w<CR>'

# Verify content
if reo-cli screen | grep -q "Test content"; then
  echo "PASS"
fi

# Server exits automatically after disconnect
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
# Open file and navigate to line 100
reo-cli keys ':e myfile.rs<CR>'
reo-cli keys '100gg'

# Search and replace
reo-cli keys ':%s/foo/bar/g<CR>'

# Save and quit
reo-cli keys ':wq<CR>'
```

## Troubleshooting

### Connection Refused

1. Check if server is running: `reo-cli list`
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

- `RpcServer` coordinates JSON-RPC 2.0 request handling
- `TransportConfig` selects transport: Stdio, UnixSocket, Tcp
- `TransportReader`/`TransportWriter` provide async I/O abstraction
- `TransportListener` accepts connections for socket/TCP
- `ChannelKeySource` injects keys from RPC into the runtime
- `FrameBufferHandle` provides unified capture for all RPC formats

### Source Files

- `lib/core/src/rpc/` - RPC server implementation
- `runner/src/server.rs` - Server startup and port management
- `runner/src/dirs.rs` - Port file management
- `tools/reo-cli/` - CLI client implementation
