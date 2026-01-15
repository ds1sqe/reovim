# Kernel Architecture

The kernel (`lib/kernel`) provides core mechanisms without policy. All subsystems are internal (`pub(crate)`) except the `api/` module.

## Subsystem Overview

| Subsystem | Linux Equivalent | Purpose | Documentation |
|-----------|------------------|---------|---------------|
| `mm/` | `mm/` | Memory management (Buffer, Position) | [mm/overview.md](./mm/overview.md) |
| `ipc/` | `ipc/` | Inter-process communication (EventBus) | [ipc/overview.md](./ipc/overview.md) |
| `core/` | `kernel/` | Core primitives (Motion, TextObject) | [core/overview.md](./core/overview.md) |
| `block/` | `block/` | Block operations (UndoTree, Transaction) | [block/overview.md](./block/overview.md) |
| `sched/` | `kernel/sched/` | Scheduler (Runtime, WorkQueue) | [sched/overview.md](./sched/overview.md) |
| `api/` | `include/linux/` | Public API surface | [api/overview.md](./api/overview.md) |
| `printk/` | `kernel/printk/` | Kernel logging | See [log driver](../drivers/log/overview.md) |

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
│  └── sched/  (internal)                                     │
└─────────────────────────────────────────────────────────────┘
```

## Visibility Rules

```rust
// lib/kernel/src/lib.rs

pub mod api;           // PUBLIC - the interface

pub(crate) mod mm;     // PRIVATE - internal only
pub(crate) mod ipc;    // PRIVATE
pub(crate) mod core;   // PRIVATE
pub(crate) mod block;  // PRIVATE
pub(crate) mod sched;  // PRIVATE
pub(crate) mod printk; // PRIVATE
```

Modules can only import from `api::*`:

```rust
// In a module
use reovim_kernel::api::v1::*;  // OK
use reovim_kernel::mm::*;       // ERROR: private
```

## Related Documents

- [Architecture Overview](../architecture/overview.md) - System-wide architecture
- [Mechanism vs Policy](../contributing/philosophy/mechanism-vs-policy.md) - Design principle
- [Driver Layer](../drivers/overview.md) - Driver implementations
- [Module System](../modules/overview.md) - Dynamic modules
