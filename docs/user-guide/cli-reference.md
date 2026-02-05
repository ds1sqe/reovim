# CLI Reference

Complete command reference for reovim server, CLI client, and TUI client.

## Server Commands

### Starting the Server

```bash
# Default: Start server + TUI in one command (tmux-like)
cargo run
# Server spawns in background, TUI attaches automatically
# TUI exit doesn't kill server (graceful detach)

# Start server in detached/daemon mode (no TUI)
cargo run -- -d
cargo run -- --detach

# Explicit server mode (TCP on 127.0.0.1:12521, or next available port)
cargo run -- server
# Server prints "Listening on 127.0.0.1:<PORT>" to stderr

# Run server with stdio transport
cargo run -- server --stdio

# Run server on Unix socket
cargo run -- server --socket /tmp/reovim.sock

# Run server on custom TCP port
cargo run -- server --tcp 9000
```

### Server Options

| Flag | Description |
|------|-------------|
| `-d`, `--detach` | Start server in daemon mode (no TUI) |
| `--tcp <PORT>` | Listen on custom TCP port (default: 12521) |
| `--socket <PATH>` | Listen on Unix socket |
| `--stdio` | Use stdio transport (always one-shot) |

## CLI Client Commands

The CLI client provides command-line access to running reovim servers.

### Connection Options

```bash
# Auto-discover and connect to available server
reovim cli <command>

# Connect to specific server via TCP
reovim cli --tcp 127.0.0.1:12521 <command>

# Connect to specific server via Unix socket
reovim cli --socket /tmp/reovim.sock <command>

# JSON output format
reovim cli --format json <command>
```

### Server Management

```bash
# List running servers
reovim cli list

# Kill server
reovim cli kill
```

### Key Input

```bash
# Inject key sequence (vim notation)
reovim cli keys 'iHello<Esc>'

# Examples
reovim cli keys '100gg'              # Go to line 100
reovim cli keys ':%s/foo/bar/g<CR>'  # Search and replace
reovim cli keys ':wq<CR>'            # Save and quit
```

### State Queries

```bash
# Get current mode
reovim cli mode

# Get cursor position
reovim cli cursor

# Get layout information
reovim cli layout
```

### Debug Commands

```bash
# Get/set log level dynamically
reovim cli log-level                 # Get current level
reovim cli log-level debug           # Set to debug

# View recent log entries
reovim cli log-tail                  # Last 50 entries
reovim cli log-tail --count 100      # Last 100 entries

# Filter logs
reovim cli log-tail --level warn     # WARN and above
reovim cli log-tail --target runner  # Filter by module
reovim cli log-tail --grep "error"   # Search messages

# Stream logs in real-time (like tail -f)
reovim cli log-tail --follow
reovim cli log-tail --follow --level warn
```

### Frame Capture

```bash
# Capture TUI frame (requires connected TUI)
reovim cli capture                        # With ANSI colors
reovim cli capture --format plain_text    # Plain text without colors
```

### Interactive REPL

```bash
reovim cli -i                        # Enter interactive mode

# REPL commands
reovim> keys iHello<Esc>
ok: true
reovim> mode
Mode: NORMAL
reovim> cursor
Cursor: line 0, column 5
reovim> quit
```

## TUI Client Commands

The TUI client provides a full terminal user interface.

### Starting the TUI

```bash
# Auto-discover and connect to server
reovim tui

# Connect to specific server
reovim tui --tcp 127.0.0.1:12521
reovim tui --socket /tmp/reovim.sock

# Attach to existing server
cargo run -- attach
cargo run -- attach --tcp 127.0.0.1:12521
```

### TUI Debug Mode

```bash
# Enable debug statusline and frame capture
reovim tui --debug

# Custom log directory
reovim tui --debug --debug-dir /tmp/reovim-debug

# Custom session name
reovim tui --debug --debug-name mysession

# Headless mode (no TTY, for CI/scripting)
reovim tui --headless
```

### TUI Options

| Flag | Description |
|------|-------------|
| `--tcp <ADDR>` | Connect to server via TCP |
| `--socket <PATH>` | Connect to server via Unix socket |
| `--debug` | Enable debug mode (statusline + frame capture) |
| `--debug-dir <DIR>` | Custom log directory |
| `--debug-name <NAME>` | Session name for filenames |
| `--headless` | Headless mode for CI/scripting |

### Debug Output Files

When debug mode is enabled:

- **Session log**: `~/.local/share/reovim/logs/tui/{name}_{start_time}.log`
- **Frame captures**: `~/.local/share/reovim/logs/tui/frame-buffer/{name}-{timestamp}.frame`

**Statusline format:**
```
[YY-MM-DD HH:MM:SS TZ] [server: ADDR] [mode: MODE] [modules: N]
```

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
| `<Up>`, `<Down>`, `<Left>`, `<Right>` | Arrow keys |
| `<Home>`, `<End>` | Navigation keys |
| `<PageUp>`, `<PageDown>` | Page keys |

## Related Documents

- [Server Mode](./server-mode.md) - Server architecture and RPC protocol
- [Commands](./commands.md) - Ex-commands reference
