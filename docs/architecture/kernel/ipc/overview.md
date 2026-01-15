# ipc/ - Inter-Process Communication

Event-based communication between components.

## Source Location

`lib/kernel/src/ipc/`

## Key Types

```rust
pub struct EventBus {
    subscribers: HashMap<TypeId, Vec<Handler>>,
}

pub struct EventScope {
    counter: AtomicUsize,
    // For synchronization
}
```

## EventBus

Publish-subscribe system for decoupled communication:

```rust
// Subscribe to events
event_bus.subscribe::<BufferChanged>(|event| {
    // Handle buffer change
});

// Emit events (not "publish")
event_bus.emit(BufferChanged { buffer_id });

// Emit with scope tracking
let scope = EventScope::new();
event_bus.emit_scoped(BufferChanged { buffer_id }, &scope);

// Async emit (fire-and-forget)
event_bus.emit_async(BufferChanged { buffer_id });
```

## EventScope

Tracks event lifecycles for synchronization:

```rust
let scope = EventScope::new();
scope.increment();  // Event started
// ... process event ...
scope.decrement();  // Event completed
scope.wait().await; // Wait for all events
```

## Built-in Events

Access events via `reovim_kernel::api::v1::events`:

```rust
use reovim_kernel::api::v1::events::ModeChanged;

// Emit a mode change event
ctx.event_bus.emit(ModeChanged {
    from: "normal".to_string(),
    to: "insert".to_string(),
});
```

## API Exports

```rust
use reovim_kernel::api::v1::{
    EventBus, EventScope, Event, DynEvent,
    events,  // Access kernel events: events::ModeChanged, etc.
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Core Primitives](../core/overview.md) - Mode, Motion types
