# Client Architecture

CLI and TUI clients for connecting to reovim servers.

## Source Location

`runner/src/client/`

## Components

| Component | Path | Purpose |
|-----------|------|---------|
| Common | `runner/src/client/common/` | Shared connection, RPC, discovery |
| CLI | `runner/src/client/cli/` | Command-line interface |
| TUI | `runner/src/client/tui/` | Terminal user interface |

## Client Types

### CLI Client

Command-line interface for scripting and automation:

```bash
# Inject keys
reovim cli keys 'iHello<Esc>'

# Query state
reovim cli mode
reovim cli cursor

# Interactive REPL
reovim cli -i
```

See [CLI Documentation](./cli.md) for details.

### TUI Client

Full terminal interface with rendering:

```bash
# Auto-discover server
reovim tui

# Connect to specific server
reovim tui --tcp 127.0.0.1:12521
```

See [TUI Documentation](./tui.md) for details.

## Server Discovery

Clients auto-discover servers via port files:

```
~/.local/share/reovim/servers/<pid>.port
```

```bash
# List running servers
reovim cli list
127.0.0.1:12521 (pid: 123456)
127.0.0.1:12522 (pid: 123789)
```

## Connection Options

```bash
# TCP connection
reovim cli --tcp localhost:12521 keys 'j'

# Unix socket
reovim cli --socket /tmp/reovim.sock keys 'j'
```

## Related Documents

- [CLI Documentation](./cli.md) - CLI commands
- [TUI Documentation](./tui.md) - TUI features
- [Server Mode Reference](../../user-guide/server-mode.md) - Server usage
