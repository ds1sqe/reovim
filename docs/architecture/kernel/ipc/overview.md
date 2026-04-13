# ipc/ - Inter-Process Communication

Event-based communication between components.

## Source Location

`lib/kernel/src/ipc/`

## Key Types

```rust
pub struct EventBus {
    handlers: ArcSwap<HandlerMap>,  // Lock-free dispatch via ArcSwap
}

pub struct EventScope {
    counter: AtomicUsize,
}
```

`EventBus` uses `ArcSwap` for its handler map, enabling lock-free reads during dispatch
while allowing atomic handler-list updates on subscribe/unsubscribe.

## EventBus

Publish-subscribe system for decoupled communication:

```rust
// Subscribe to events — priority determines dispatch order (lower = earlier)
let _handle = event_bus.subscribe::<ModeChanged, _>(priority::NORMAL, |event| {
    // Handle mode change; return EventResult to continue or stop propagation
    EventResult::Handled
});
// _handle must be kept alive; dropping it unsubscribes automatically.

// Emit events (synchronous, in-process)
event_bus.emit(ModeChanged { from: "normal".into(), to: "insert".into() });

// Emit with scope tracking
let scope = EventScope::new();
event_bus.emit_scoped(ModeChanged { from: "normal".into(), to: "insert".into() }, &scope);

// Async emit (fire-and-forget, crosses thread boundary)
event_bus.emit_async(ModeChanged { from: "normal".into(), to: "insert".into() });
```

## Priority Constants

Lower values run first:

| Constant           | Value | Intended use                    |
|--------------------|-------|---------------------------------|
| `priority::CRITICAL` | 0   | Kernel-internal invariants      |
| `priority::CORE`     | 10  | Core drivers (session, buffer)  |
| `priority::NORMAL`   | 50  | Standard modules                |
| `priority::PLUGIN`   | 100 | Third-party plugins             |
| `priority::LOW`      | 200 | Logging, telemetry, UI overlays |

## EventScope

Tracks in-flight event counts for synchronization:

```rust
let scope = EventScope::new();
scope.increment();  // Event started
// ... process event ...
scope.decrement();  // Event completed
scope.wait();       // Block until counter reaches zero (synchronous, not async)
```

`wait()` is a synchronous spin/park operation, not an `async fn`. Do not `.await` it.

## Kernel Events

All kernel events are accessible via `reovim_kernel::api::v1::events`:

| Event               | Trigger                                      |
|---------------------|----------------------------------------------|
| `BufferCreated`     | A new buffer is allocated in the kernel      |
| `BufferClosed`      | A buffer is deallocated                      |
| `BufferBytesEdited` | Raw byte content of a buffer changed         |
| `BufferSwitched`    | Active buffer changed for a client/window    |
| `BufferWillSave`    | Buffer is about to be written to disk        |
| `BufferSaved`       | Buffer successfully written to disk          |
| `FileOpened`        | A file path was opened into a buffer         |
| `WindowCreated`     | A new window is created                      |
| `WindowClosed`      | A window is removed                          |
| `WindowFocused`     | Focus moved to a window                      |
| `LayoutChanged`     | Window layout recalculated                   |
| `ModeChanged`       | Editor mode transitioned (Normal/Insert/...) |
| `OptionChanged`     | An option value was set                      |
| `OptionReset`       | An option was reset to its default           |
| `Shutdown`          | Server shutdown sequence started             |

There is no `BufferChanged` event in the kernel. Content-change signals are
`BufferBytesEdited`; higher-level change semantics live in `reovim-domain-text`.

## Event Layer Model

The kernel IPC layer is the lowest of three event layers. See
[Event Layers](../../event-layers.md) for the full three-layer model (kernel substrate,
per-domain, streaming).

## API Exports

```rust
use reovim_kernel::api::v1::{
    EventBus, EventScope, Event, DynEvent, EventResult,
    events,  // Access kernel events: events::ModeChanged, etc.
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Core Primitives](../core/overview.md) - Mode, Motion types
- [Event Layers](../../event-layers.md) - Three-layer event model
