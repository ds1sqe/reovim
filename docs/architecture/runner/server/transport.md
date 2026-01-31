# Transport Layer

The server supports multiple transport protocols for client connections.

## Source Location

`shared/net/src/` (moved from `runner/src/server/transport/` in Phase 8)

## Transport Options

| Transport | Flag | Use Case |
|-----------|------|----------|
| TCP | `--tcp <PORT>` | Network connections, multiple clients |
| Unix Socket | `--socket <PATH>` | Local IPC, faster than TCP |
| Stdio | `--stdio` | Subprocess communication, IDE integration |

## TCP Transport (Default)

```bash
# Default port 12521
reovim server

# Custom port
reovim server --tcp 9000
```

### Port Fallback

When the default port is in use:
- Server tries 12522, 12523, ... up to 12530
- Prints actual port to stderr: `Listening on 127.0.0.1:12522`

### Port File Discovery

Each server writes a port file:
```
~/.local/share/reovim/servers/<pid>.port
```

## Unix Socket Transport

```bash
reovim server --socket /tmp/reovim.sock
```

Lower latency than TCP for local connections.

## Stdio Transport

```bash
reovim server --stdio
```

- Single connection only (one-shot mode)
- Used for IDE integration and subprocess communication
- Exits when connection closes

## Transport Traits

```rust
pub trait TransportReader: Send {
    fn read(&mut self) -> Result<RpcMessage>;
}

pub trait TransportWriter: Send {
    fn write(&mut self, message: &RpcMessage) -> Result<()>;
}

pub trait TransportListener: Send {
    fn accept(&mut self) -> Result<(Box<dyn TransportReader>, Box<dyn TransportWriter>)>;
}
```

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Server Mode Reference](../../user-guide/server-mode.md) - Usage guide
