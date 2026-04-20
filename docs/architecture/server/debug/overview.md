# debug/ - Server Debug Infrastructure

Post-mortem analysis and debugging infrastructure for server-side crash reporting.

## Source Location

`server/lib/server/src/debug/`

## Architecture

```
+------------------------------------------------------------------+
|                      Debug Infrastructure                          |
|                                                                    |
|  Server Ring Buffer (64 KB)                                        |
|  +-----------------------------------------------------------+    |
|  | Captures: pr_err!, pr_info!, pr_warn!, pr_debug!          |    |
|  | Thread-safe: parking_lot::RwLock                          |    |
|  | Non-blocking: try_dump() for panic safety                 |    |
|  +-----------------------------------------------------------+    |
|                           |                                        |
|         +-----------------+------------------+                     |
|         |                                    |                     |
|  Per-Client Buffers (8 KB each)     CompositeLogger               |
|  +------------------------+         +------------------------+     |
|  | Keys, commands, mode   |         | Forwards to:           |     |
|  | Dump on disconnect to  |         | - Ring buffer          |     |
|  | crash/ directory       |         | - tracing subscriber   |     |
|  +------------------------+         +------------------------+     |
|                                                                    |
+------------------------------------------------------------------+
```

## Components

### ring_buffer.rs - Debug Ring Buffer

A lock-free ring buffer for capturing events for crash analysis:

```rust
pub struct DebugRingBuffer {
    // 64 KB server-wide buffer
    // 8 KB per-client buffers
}

impl DebugRingBuffer {
    pub fn push(&self, entry: LogEntry);
    pub fn dump(&self) -> Vec<LogEntry>;
    pub fn try_dump(&self) -> Option<Vec<LogEntry>>; // Non-blocking
    pub fn stats(&self) -> BufferStats;
}
```

**Key exports:**
- `DebugRingBuffer` - The ring buffer implementation
- `LogEntry` - Log entry with timestamp, level, target, message
- `LogEntryView` - View into entry without allocation
- `BufferStats` - Buffer statistics
- `init_debug_ring()` - Initialize global ring buffer
- `debug_ring()` - Get reference to global buffer
- `try_debug_ring()` - Non-blocking access for panic handler

**Constants:**
- `DEFAULT_CAPACITY` - 64 KB (server), 8 KB (per-client)
- `MAX_MESSAGE_LEN` - Maximum message length before truncation

### composite_logger.rs - Dual Output Logger

Forwards log messages to both ring buffer and tracing:

```rust
pub struct CompositeLogger {
    ring_buffer: Arc<DebugRingBuffer>,
    tracing_subscriber: TracingSubscriber,
}

impl Logger for CompositeLogger {
    fn log(&self, record: &Record) {
        self.ring_buffer.push(LogEntry::from(record));
        self.tracing_subscriber.emit(record);
    }
}
```

**Key exports:**
- `CompositeLogger` - The dual-output logger
- `COMPOSITE_LOGGER` - Global singleton

## Integration Points

### Kernel Logger Trait

The ring buffer implements the kernel's `Logger` trait:

```rust
// server/lib/kernel/src/printk/mod.rs
pub trait Logger: Send + Sync {
    fn log(&self, record: &Record);
}
```

`Record` carries level, target, message, and optional source location info.

This enables kernel code to log via `pr_err!`, `pr_info!`, etc.

### Panic Handler Integration

The debug ring buffer integrates with the panic subsystem:

```rust
use reovim_kernel::panic::set_debug_context_callback;

set_debug_context_callback(Box::new(|| {
    DebugContext {
        server_logs: try_debug_ring()
            .map(|rb| rb.try_dump())
            .flatten()
            .unwrap_or_default(),
        client_dump_paths: /* ... */,
    }
}));
```

### Client Disconnect Dumps

When a client disconnects, their ring buffer is dumped:

```rust
// Session::remove_client()
fn remove_client(&self, client_id: ClientId) {
    // Dump client ring buffer to file
    let path = format!(
        "~/.local/share/reovim/crash/client-{}-{}.log",
        client_id, timestamp
    );
    client.ring_buffer.dump_to_file(&path);

    // Log disconnect to server ring buffer
    self.ring_buffer.push(LogEntry::new(
        Level::Info,
        "session",
        format!("CLIENT_DISCONNECT: {}", client_id),
    ));
}
```

## CLI Access

Query the ring buffer via the CLI:

```bash
# Get recent log entries
reovim cli log-tail

# Filter by count
reovim cli log-tail --count 100

# Filter by level (trace, debug, info, warn, error)
reovim cli log-tail --level warn

# Filter by target module
reovim cli log-tail --target runner::server

# Search messages
reovim cli log-tail --grep "connection"

# Stream logs (like tail -f)
reovim cli log-tail --follow

# JSON output
reovim cli --format json log-tail
```

**Color coding (TTY output):**
- ERROR: Red
- WARN: Yellow
- INFO: Green
- DEBUG: Cyan
- TRACE: Gray

## gRPC Protocol

The `DebugService` provides access via gRPC:

```protobuf
// uapi/protocol/proto/reovim/v2/debug.proto
service DebugService {
    rpc LogTail(LogTailRequest) returns (LogTailResponse);
    rpc LogLevel(LogLevelRequest) returns (LogLevelResponse);
    rpc DebugSendKeys(DebugSendKeysRequest) returns (DebugSendKeysResponse);
    rpc DebugCapture(DebugCaptureRequest) returns (DebugCaptureResponse);
    rpc DebugGetMode(DebugGetModeRequest) returns (DebugGetModeResponse);
    rpc DebugGetCursor(DebugGetCursorRequest) returns (DebugGetCursorResponse);
    rpc DebugListClients(DebugListClientsRequest) returns (DebugListClientsResponse);
    rpc DebugGetExtensionState(DebugGetExtensionStateRequest) returns (DebugGetExtensionStateResponse);
    rpc DebugListExtensions(DebugListExtensionsRequest) returns (DebugListExtensionsResponse);
}

message LogTailRequest {
    optional uint32 count = 1;
    optional string level = 2;
    optional string target = 3;
    optional string grep = 4;
}

message LogTailResponse {
    repeated LogEntry entries = 1;
}
```

## File Locations

| Type | Path |
|------|------|
| Crash reports | `~/.local/share/reovim/crash/crash-{date}_{time}-{nanos}.txt` |
| Client dumps | `~/.local/share/reovim/crash/client-{id}-{timestamp}.log` |
| Recovery files | `~/.local/share/reovim/recovery/` |

## Related Documents

- [Kernel Panic Handling](../../kernel/panic/overview.md) - Panic subsystem
- [Server Mode](../../../user-guide/server-mode.md) - CLI usage
