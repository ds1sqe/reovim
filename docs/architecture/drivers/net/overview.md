# net/ - Network Driver

RPC server and transport abstractions.

## Source Location

`lib/drivers/net/src/`

## Key Traits

```rust
pub trait RpcServer: Send + Sync {
    fn start(&mut self, config: TransportConfig) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn handle_request(&mut self, request: RpcRequest) -> RpcResponse;
}
```

## Transport Configuration

```rust
pub enum TransportConfig {
    Stdio,
    Tcp { host: String, port: u16 },
    UnixSocket { path: PathBuf },
}
```

## Request/Response

```rust
pub struct RpcRequest {
    pub id: RequestId,
    pub method: String,
    pub params: Value,
}

pub struct RpcResponse {
    pub id: RequestId,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}
```

## Transport Layer

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

- [Driver Overview](../overview.md) - Driver layer architecture
- [Server Mode Reference](../../../user-guide/server-mode.md) - RPC server usage
- [Server Architecture](../../server/overview.md) - Server implementation
