# Client Architecture

CLI and TUI clients for connecting to reovim servers.

## Source Location

Clients moved to dedicated directories in Phase 8:

## Components

| Component | Path | Purpose |
|-----------|------|---------|
| CLI | `clients/cli/` | Command-line interface (gRPC v2) |
| TUI | `clients/tui/` | Terminal user interface (gRPC v2) |
| Web | `clients/web/` | Web client (TypeScript + WASM) |
| Common Model | `shared/clients/model/` | Platform-agnostic client abstractions |

## Client Types

### CLI Client

Command-line interface for scripting and automation:

```bash
# Inject keys
reovim cli keys 'iHello<Esc>'

# Query state
reovim cli mode
reovim cli cursor

# List connected clients
reovim cli clients
```

See [CLI Documentation](./cli.md) for details.

### TUI Client

Full terminal interface with rendering:

```bash
# Auto-discover server
reovim tui

# Connect to specific server
reovim tui --grpc 127.0.0.1:12540
```

See [TUI Documentation](./tui.md) for details.

## Connecting to Servers

Clients connect to servers via gRPC:

```bash
# Connect to default address
reovim cli --grpc 127.0.0.1:12540 clients
reovim tui --grpc 127.0.0.1:12540
```

The default server port is 12540 (fallback range: 12540-12549).

## Connection Options

```bash
# gRPC connection
reovim cli --grpc 127.0.0.1:12540 keys 'j' --client 1
```

## Related Documents

- [CLI Documentation](./cli.md) - CLI commands
- [TUI Documentation](./tui.md) - TUI features
- [Server Mode Reference](../../user-guide/server-mode.md) - Server usage
