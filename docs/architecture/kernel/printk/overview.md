# printk/ - Kernel Logging

Structured log message infrastructure following Linux's `printk` design.

## Source Location

`server/lib/kernel/src/printk/`

## Overview

The `printk/` subsystem provides the kernel's logging mechanism. The kernel
defines the interface (mechanism); drivers register an implementation (policy)
at startup.

Following Linux kernel convention:

- Kernel provides: `Logger` trait, `Level` enum, `Record` struct, `pr_*` macros.
- Drivers provide: where logs go, formatting, filtering, buffering.
- Zero external dependencies: pure Rust, no `tracing` crate in the kernel.
- Performance first: level check before formatting avoids allocation when logging
  is disabled.

```
User code:  pr_err!(...), pr_info!(...), pr_debug!(...)
                |
                v
Kernel:     Level check -> __log() -> logger().log(record)
                |
       OnceLock<&'static dyn Logger>
          /             \
NopLogger (default)    Driver Logger (e.g., TracingLogger)
```

## Key Types

### `Level`

```rust
#[repr(u8)]
pub enum Level {
    Error = 0,
    Warn  = 1,
    Info  = 2,  // default
    Debug = 3,
    Trace = 4,
}
```

Ordered by severity: `Error` (most severe) to `Trace` (most verbose). A logger
configured for `Info` will emit `Error`, `Warn`, and `Info` but not `Debug` or
`Trace`.

Parses from string (case-insensitive): `"error"`, `"warn"` / `"warning"`,
`"info"`, `"debug"`, `"trace"`.

### `Record`

Lifetime-bound log entry containing level, message, module path, file, and
line number. Constructed via `RecordBuilder`.

```rust
let record = Record::builder(Level::Info)
    .message("buffer opened")
    .module_path("reovim_kernel::mm")
    .file("buffer.rs")
    .line(42)
    .build();
```

### `Logger` trait

```rust
pub trait Logger: Send + Sync {
    fn log(&self, record: &Record);
    fn flush(&self);
    fn enabled(&self, level: Level) -> bool;
}
```

`NopLogger` is the default — `enabled()` returns `false` for all levels,
preventing any string formatting in the macros.

### Global Logger

```rust
// Set once at startup (typically in server/lib/server/)
set_logger(&MY_LOGGER).expect("logger already set");

// Access from anywhere
logger().enabled(Level::Info);
flush();
```

`set_logger` uses `OnceLock` — the first call wins; subsequent calls return
`Err(SetLoggerError)`.

## Macros

The `pr_*` macros check the level before formatting:

```rust
pr_err!("failed: {}", err);
pr_warn!("deprecated API used");
pr_info!("opened {} buffers", 3);
pr_debug!("cursor at {:?}", pos);
pr_trace!("entering function");
```

Expansion pattern (zero allocation when level is disabled):

```rust
if logger().enabled(Level::Info) {
    __log(Level::Info, module_path!(), file!(), line!(), format_args!(...));
}
```

## Type Reference

| Type | Purpose |
|------|---------|
| `Level` | Log severity (`Error` through `Trace`) |
| `ParseLevelError` | Error from `Level::from_str` |
| `Record<'a>` | Single log entry (message + metadata) |
| `RecordBuilder<'a>` | Builder for `Record` |
| `Logger` | Trait: driver-implemented log consumer |
| `NopLogger` | Zero-overhead no-op logger (default) |
| `SetLoggerError` | Error when `set_logger` is called more than once |

## API Exports

```rust
use reovim_kernel::api::v1::{
    Level, ParseLevelError,
    Record, RecordBuilder,
    Logger, NopLogger, SetLoggerError,
    set_logger, logger, flush,
    // macros (re-exported from crate root)
    pr_err, pr_warn, pr_info, pr_debug, pr_trace,
};
```

## Implementation Notes

The `shared/log/` crate (`reovim-log`) provides the driver-side logger
implementation used by the server runtime. It bridges the kernel `Logger`
trait to the `tracing` subscriber ecosystem.

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [debug Subsystem](../debug/overview.md) - Metrics and profiling
