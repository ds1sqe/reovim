# Kernel Architecture

The kernel (`lib/kernel`) provides core mechanisms without policy. All subsystems are internal (`pub(crate)`) except the `api/` module.

## Subsystem Overview

| Subsystem | Linux Equivalent | Purpose | Documentation |
|-----------|------------------|---------|---------------|
| `mm/` | `mm/` | ID types (BufferId, WindowId, TabId) and Saturator | [mm/overview.md](./mm/overview.md) |
| `ipc/` | `ipc/` | Inter-process communication (EventBus) | [ipc/overview.md](./ipc/overview.md) |
| `core/` | `kernel/` | Config, Mode, OptionRegistry | [core/overview.md](./core/overview.md) |
| `block/` | `block/` | Byte-level storage traits, ByteEdit, ByteUndoLog | [block/overview.md](./block/overview.md) |
| `sched/` | `kernel/sched/` | Scheduler (Runtime, WorkQueue) | [sched/overview.md](./sched/overview.md) |
| `api/` | `include/linux/` | Public API surface | [api/overview.md](./api/overview.md) |
| `printk/` | `kernel/printk/` | Kernel logging (Logger trait, pr_* macros) | [printk/overview.md](./printk/overview.md) |
| `debug/` | `kernel/trace/` | Metrics, profiling, trace points | [debug/overview.md](./debug/overview.md) |
| `panic/` | `kernel/panic/` | Panic handling and recovery | [panic/overview.md](./panic/overview.md) |

## Layer Position

```
┌─────────────────────────────────────────────────────────────┐
│  Runner (application orchestration)                         │
├─────────────────────────────────────────────────────────────┤
│  Modules (policy)                                           │
├─────────────────────────────────────────────────────────────┤
│  Drivers (mechanism services)                               │
├─────────────────────────────────────────────────────────────┤
│  Kernel (core primitives)  ← YOU ARE HERE                   │
│  ├── api/    (PUBLIC)                                       │
│  ├── mm/     (internal)                                     │
│  ├── ipc/    (internal)                                     │
│  ├── core/   (internal)                                     │
│  ├── block/  (internal)                                     │
│  ├── sched/  (internal)                                     │
│  ├── printk/ (internal)                                     │
│  ├── debug/  (internal)                                     │
│  └── panic/  (internal)                                     │
└─────────────────────────────────────────────────────────────┘
```

## Visibility Rules

```rust
// server/lib/kernel/src/lib.rs

pub mod api;           // PUBLIC - the interface

pub(crate) mod mm;     // PRIVATE - internal only
pub(crate) mod ipc;    // PRIVATE
pub(crate) mod core;   // PRIVATE
pub(crate) mod block;  // PRIVATE
pub(crate) mod sched;  // PRIVATE
pub(crate) mod printk; // PRIVATE
pub(crate) mod debug;  // PRIVATE
pub(crate) mod panic;  // PRIVATE
```

Modules can only import from `api::*`:

```rust
// In a module
use reovim_kernel::api::v1::*;  // OK
use reovim_kernel::mm::*;       // ERROR: private
```

## Related Documents

- [Architecture Overview](../overview.md) - System-wide architecture
- [Mechanism vs Policy](../../contributing/philosophy/mechanism-vs-policy.md) - Design principle
- [Driver Layer](../drivers/overview.md) - Driver implementations
- [Module System](../modules/overview.md) - Dynamic modules
