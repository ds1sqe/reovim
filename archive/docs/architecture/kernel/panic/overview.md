# panic/ - Panic Handling Subsystem

Custom panic handlers, crash recovery, and state dumping.

Linux equivalent: `kernel/panic.c`

## Source Location

`server/lib/kernel/src/panic/`

## Architecture

```
+---------------------------------------------------------+
|                     Panic Subsystem                      |
|                                                          |
|  +------------------+  +------------------+               |
|  |    Handler       |  |    Recovery      |               |
|  | (panic hook,     |  | (buffer save,    |               |
|  |  recovery cb)    |  |  file mgmt)      |               |
|  +------------------+  +------------------+               |
|                                                          |
|  +------------------+                                     |
|  |    Report        |                                     |
|  | (crash report    |                                     |
|  |  generation)     |                                     |
|  +------------------+                                     |
|                                                          |
+---------------------------------------------------------+
```

## Components

### handler.rs - Panic Handler Installation

Installs a custom panic handler that:
1. Calls recovery callback to save unsaved buffers
2. Generates a crash report with debug context
3. Writes crash report to file

**Key exports:**
- `install_panic_handler()` - Install the custom panic handler
- `is_handler_installed()` - Check if handler is installed
- `set_recovery_callback()` - Set callback for buffer recovery
- `set_debug_context_callback()` - Set callback for debug context

### recovery.rs - Buffer Recovery

Provides utilities for saving buffer state during panic:

**Key exports:**
- `RecoverySnapshot` - State snapshot for crash recovery
- `UnsavedBuffer` - Buffer data for recovery
- `save_buffer_for_recovery()` - Save buffer to recovery file
- `list_recovery_files()` - List available recovery files
- `cleanup_old_recovery_files()` - Remove old recovery data
- `recovery_dir()` - Get recovery directory path

Recovery files are stored at:
```
~/.local/share/reovim/recovery/
```

### report.rs - Crash Report Generation

Generates human-readable crash reports:

**Key exports:**
- `CrashReport` - Crash report structure
- `generate_crash_report()` - Create report from panic info

**Report format:**
```
========================================
REOVIM CRASH REPORT
========================================
Timestamp: 2026-02-04 12:34:56 UTC
Version: 0.9.3-dev
...

PANIC INFO
----------
Message: ...
Location: file.rs:123

DEBUG CONTEXT
-------------
Server logs: ...
Client dump paths: ...

RECOVERY INFO
-------------
Unsaved buffers: ...
Recovery files: ...
```

Crash reports are written to:
```
~/.local/share/reovim/crash/crash-{date}_{time}-{nanos}.txt
```

## Usage Example

```rust
use reovim_kernel::panic::{
    install_panic_handler,
    set_recovery_callback,
    save_buffer_for_recovery,
};

// Set up recovery callback
set_recovery_callback(Box::new(|_info| {
    // Save unsaved buffers
    for buffer in get_unsaved_buffers() {
        save_buffer_for_recovery(
            buffer.id,
            buffer.path.as_deref(),
            &buffer.content,
        ).ok();
    }
}));

// Install the panic handler
install_panic_handler();
```

## Debug Context Integration

The panic handler integrates with the server's debug ring buffer (#478, #481):

```rust
use reovim_kernel::panic::{set_debug_context_callback, DebugContext};

// Set debug context callback for crash reports
set_debug_context_callback(Box::new(|| {
    DebugContext {
        server_logs: dump_server_ring_buffer(),
        client_dump_paths: list_client_dumps(),
    }
}));
```

## Related Documents

- [Server Debug Infrastructure](../../server/debug/overview.md) - Ring buffer and logging
- [Kernel Overview](../overview.md) - Kernel architecture
