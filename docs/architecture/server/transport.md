# Transport Layer

The server supports multiple transport protocols for client connections.

## Source Location

`shared/net/src/`

## Transport Options

| Transport | Flag | Use Case |
|-----------|------|----------|
| gRPC | `--grpc <PORT>` | gRPC over HTTP/2 (recommended) |
| TCP | `--tcp <PORT>` | Network connections, multiple clients |
| Unix Socket | `--socket <PATH>` | Local IPC, faster than TCP |

## gRPC Transport (Recommended)

gRPC over HTTP/2 is the primary transport for client-server communication.

```bash
# Default port 12540
reovim server

# Custom port
reovim server --grpc 9000
```

## TCP Transport (Legacy)

```bash
reovim server --tcp 9000
```

### Port Fallback

When the default port is in use:
- Server tries 12541, 12542, ... up to 12549
- Prints actual port to stderr: `Listening on 127.0.0.1:12541`

## Unix Socket Transport

```bash
reovim server --socket /tmp/reovim.sock
```

Lower latency than TCP for local connections.

## Transport Traits (Legacy TCP Path)

The current primary transport is gRPC via `tonic`. The trait definitions below
in `shared/net/src/traits.rs` are for the legacy TCP path and are not used by
the gRPC transport:

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
