# mm/ - Memory Management

Kernel-owned identifiers and background saturation support.

## Source Location

`server/lib/kernel/src/mm/`

## Overview

Following the domain decoupling in #740, the `mm/` subsystem no longer holds
text content types. Buffer, Position, Edit, Line, Selection, Range, Rope, and
related structures now live in `ext/server/domain/text/` (crate `reovim-domain-text`)
and `ext/server/providers/text/` (crate `reovim-provider-text`).

The kernel `mm/` subsystem retains only what is strictly kernel-owned: opaque
identifiers for kernel resources and the `SaturatorHandle` background-work
primitive.

## Key Types

```rust
/// Unique identifier for a buffer (monotonically increasing, never reused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

/// Unique identifier for a tab page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(usize);

/// Handle to send work to a background saturator.
pub struct SaturatorHandle<T> { /* ... */ }

/// Request priority for saturation work.
pub enum RequestPriority { High, Low }
```

## Type Reference

| Type | Purpose |
|------|---------|
| `BufferId` | Opaque buffer handle; monotonically increasing, never reused |
| `WindowId` | Opaque window handle |
| `TabId` | Opaque tab page handle |
| `SaturatorHandle<T>` | Send work to a prioritized background thread |
| `SaturationRequest<T>` | Work item with explicit priority and optional `EventScope` |
| `RequestPriority` | `High` (viewport) or `Low` (off-screen) work priority |
| `SaturatorConfig` | Configuration for saturator behaviour on shutdown |

## ID Types

All three ID types follow the same pattern:

- Backed by a `usize` stored in a newtype wrapper.
- Monotonically increasing via a process-global `AtomicUsize`.
- `new()` allocates the next ID; `from_raw(v)` is available for deserialization and tests.
- `as_usize()` returns the raw value.
- `Display` formats as `Buffer(n)`, `window:n`, or `tab:n`.

```rust
use reovim_kernel::api::v1::*;

let id1 = BufferId::new();
let id2 = BufferId::new();
assert_ne!(id1, id2);
assert!(id1 < id2);
```

## Saturator

`SaturatorHandle<T>` is a generic background-work primitive for tasks such as
syntax highlighting, where viewport lines need high-priority processing and
off-screen lines can be deferred.

```rust
let handle = spawn_saturator(
    |line_idx: usize| format!("processed line {line_idx}"),
    |result| println!("{result}"),
);

// High priority (viewport)
handle.submit(0, None);

// Low priority (off-screen)
handle.submit_background(100, None);
```

The saturator spawns a dedicated background thread that drains the high-priority
queue before servicing the low-priority queue. When `SaturatorHandle` is dropped,
the worker thread finishes remaining high-priority work and then shuts down.

## API Exports

```rust
use reovim_kernel::api::v1::{
    BufferId, WindowId, TabId,
    SaturatorHandle, SaturationRequest, SaturatorConfig,
    RequestPriority, spawn_saturator,
};
```

## Moved Types

The following types were removed from `mm/` as part of #740 and now live in
`reovim-domain-text` or `reovim-provider-text`:

| Type | New location |
|------|-------------|
| `Buffer`, `Rope` | `reovim-provider-text` |
| `BufferSnapshot` | `reovim-provider-text` |
| `Position`, `Edit`, `Selection` | `reovim-domain-text` |
| `LineIndex`, delimiter matching | `reovim-domain-text` |
| `FileMapping`, `PieceTree` | `reovim-subsys-vfs` |

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Block Subsystem](../block/overview.md) - Byte-level storage and undo log
- [IPC Subsystem](../ipc/overview.md) - EventBus and EventScope
