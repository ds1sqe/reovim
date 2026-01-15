# log/ - Log Driver

Logger implementation using `tracing`.

## Source Location

`lib/drivers/log/src/`

## Implementation

```rust
pub struct TracingLogger {
    // Uses tracing subscriber
}

impl Logger for TracingLogger {
    fn log(&self, level: Level, message: &str) {
        match level {
            Level::Error => tracing::error!("{}", message),
            Level::Warn => tracing::warn!("{}", message),
            Level::Info => tracing::info!("{}", message),
            Level::Debug => tracing::debug!("{}", message),
            Level::Trace => tracing::trace!("{}", message),
        }
    }
}
```

## Design Note

This is the ONLY place where `tracing` dependency exists in the codebase. The kernel defines the `Logger` trait in `printk/`, but the actual implementation lives here in the driver layer.

This separation allows:
- Kernel to remain dependency-free
- Different logger implementations (file, syslog, etc.)
- Testing with mock loggers

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
- [Kernel Overview](../../kernel/overview.md) - printk/ module
