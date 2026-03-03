# CLI Reference

Complete command reference for reovim server, CLI client, and TUI client.

## Server

### Starting the Server

```bash
# Start server + TUI in one command (integrated mode)
reovim
# Server spawns, TUI attaches automatically
# TUI exit doesn't kill server (graceful detach)

# Explicit server mode (gRPC on 127.0.0.1:12540, or next available port)
reovim server
# Server prints "Listening on 127.0.0.1:<PORT>" to stderr

# Custom gRPC port
reovim server --grpc 9000

# Custom TCP port (legacy transport)
reovim server --tcp 9000

# Unix socket
reovim server --socket /tmp/reovim.sock

# Named session
reovim server --session myproject

# Named instance
reovim server --instance work
```

### Server Options

| Flag | Description |
|------|-------------|
| `--grpc <PORT>` | Listen on gRPC port (default: 12540) |
| `--tcp <PORT>` | Listen on legacy TCP port |
| `--socket <PATH>` | Listen on Unix socket |
| `--session <NAME>` | Named session identifier |
| `--instance <NAME>` | Named instance identifier |

## CLI Client

The CLI client provides command-line access to running reovim servers.

### Connection

```bash
# Connect to specific server via gRPC (required)
reovim cli --grpc 127.0.0.1:12540 <command>

# JSON output format
reovim cli --grpc 127.0.0.1:12540 --format json <command>
```

### All Subcommands

| Subcommand | Required Args | Description |
|------------|--------------|-------------|
| `keys` | `KEY --client ID` | Inject key sequence (vim notation) |
| `mode` | `--client ID` | Get current editor mode |
| `cursor` | `--client ID` | Get cursor position |
| `buffers` | | List all open buffers |
| `buffer` | | Get buffer content (`--id ID` for specific buffer) |
| `registers` | | Query register contents (optional `NAME` for specific register) |
| `capture` | | Capture TUI frame (`--client ID`, `--capture-format FMT`) |
| `ping` | | Health check |
| `version` | | Get server version |
| `log-tail` | | Get recent log entries |
| `clients` | | List connected clients |
| `extension-state` | `KIND --client ID` | Query extension state |
| `extensions` | | List registered extensions |

### Key Input

```bash
# Inject key sequence (vim notation)
reovim cli --grpc 127.0.0.1:12540 keys 'iHello<Esc>' --client 1

# Examples
reovim cli --grpc 127.0.0.1:12540 keys '100gg' --client 1
reovim cli --grpc 127.0.0.1:12540 keys ':%s/foo/bar/g<CR>' --client 1
reovim cli --grpc 127.0.0.1:12540 keys ':wq<CR>' --client 1
```

### State Queries

```bash
# Get current mode (requires --client)
reovim cli --grpc 127.0.0.1:12540 mode --client 1

# Get cursor position (requires --client)
reovim cli --grpc 127.0.0.1:12540 cursor --client 1

# List open buffers
reovim cli --grpc 127.0.0.1:12540 buffers

# Get buffer content
reovim cli --grpc 127.0.0.1:12540 buffer
reovim cli --grpc 127.0.0.1:12540 buffer --id 1

# Query registers
reovim cli --grpc 127.0.0.1:12540 registers
reovim cli --grpc 127.0.0.1:12540 registers a
```

### Server Queries

```bash
# List connected clients
reovim cli --grpc 127.0.0.1:12540 clients

# Health check
reovim cli --grpc 127.0.0.1:12540 ping

# Server version
reovim cli --grpc 127.0.0.1:12540 version
```

### Debug Commands

```bash
# View recent log entries (default: 50)
reovim cli --grpc 127.0.0.1:12540 log-tail

# Custom count
reovim cli --grpc 127.0.0.1:12540 log-tail --count 100

# Filter by level (shows entries >= specified level)
reovim cli --grpc 127.0.0.1:12540 log-tail --level warn

# Filter by target module
reovim cli --grpc 127.0.0.1:12540 log-tail --target runner::server

# Search messages (case-insensitive)
reovim cli --grpc 127.0.0.1:12540 log-tail --grep "error"

# Combine filters
reovim cli --grpc 127.0.0.1:12540 log-tail --level info --target runner --grep error
```

### Extension Queries

Query extension state for debugging (e.g., which-key popup, cmdline input).

```bash
# List all registered extensions
reovim cli --grpc 127.0.0.1:12540 extensions

# Query extension state for a specific client
reovim cli --grpc 127.0.0.1:12540 extension-state whichkey --client 1
reovim cli --grpc 127.0.0.1:12540 extension-state cmdline --client 1

# JSON format for programmatic use
reovim cli --grpc 127.0.0.1:12540 --format json extension-state whichkey --client 1
```

### Frame Capture

```bash
# Capture TUI frame (requires connected TUI)
# --client specifies which TUI client to capture from
reovim cli --grpc 127.0.0.1:12540 capture --client 1
reovim cli --grpc 127.0.0.1:12540 capture --client 1 --capture-format plain_text
reovim cli --grpc 127.0.0.1:12540 capture --client 1 --capture-format cell_grid
```

Supported formats: `raw_ansi` (default), `plain_text`, `cell_grid`, `png`, `html`

## TUI Client

The TUI client provides a full terminal user interface.

### Starting the TUI

```bash
# Connect to a running server
reovim tui --grpc 127.0.0.1:12540

# Headless mode (no TTY, for CI/scripting)
reovim tui --grpc 127.0.0.1:12540 --headless

# Custom viewport size (headless)
reovim tui --grpc 127.0.0.1:12540 --headless --width 120 --height 40
```

### TUI Options

| Flag | Description |
|------|-------------|
| `--grpc <ADDR>` | Connect to server via gRPC (required) |
| `--headless` | Headless mode for CI/scripting (no terminal required) |
| `--width <N>` | Viewport width (headless mode) |
| `--height <N>` | Viewport height (headless mode) |

## Integrated Mode

Running `reovim` without subcommands starts the integrated mode:

```bash
# Start server + TUI together
reovim

# Opens a file
reovim myfile.txt
```

The server runs in the background and the TUI attaches automatically. Exiting the TUI does not kill the server.

## Global Options

| Flag | Description |
|------|-------------|
| `--format <FMT>` | Output format for CLI: `plain` (default) or `json` |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `REOVIM_LOG` | Set log level (e.g., `info`, `debug`, `trace`) |
| `REOVIM_EXTRA_MODULES` | Additional module paths to load |
| `REOVIM_TEST_BINARY` | Path to reovim binary (testing) |

## Key Notation

| Notation | Key Event |
|----------|-----------|
| `a`-`z`, `0`-`9` | Character keys |
| `<Esc>` | Escape key |
| `<CR>` or `<Enter>` | Enter key |
| `<BS>` | Backspace |
| `<Tab>` | Tab |
| `<Space>` | Space |
| `<C-x>` | Ctrl+X |
| `<S-x>` | Shift+X |
| `<A-x>` or `<M-x>` | Alt/Meta+X |
| `<Up>`, `<Down>`, `<Left>`, `<Right>` | Arrow keys |
| `<Home>`, `<End>` | Navigation keys |
| `<PageUp>`, `<PageDown>` | Page keys |
| `<F1>` - `<F12>` | Function keys |

## Related Documents

- [Server Mode](./server-mode.md) - Server architecture and gRPC protocol
- [Frame Capture](./frame-capture.md) - Frame capture details
- [Commands](./commands.md) - Ex-commands reference
- [Keymap Configuration](./keymap-config.md) - Custom keybindings
