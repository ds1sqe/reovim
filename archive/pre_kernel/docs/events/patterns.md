# Event Design Patterns

When designing events for your plugin, choose the appropriate pattern based on your needs.

## Unified Command-Event Types (Recommended)

**Modern approach:** Use `declare_event_command!` macro to create a single type that serves as both command and event.

```rust
use reovim_core::declare_event_command;

// Single unified type - both command AND event
declare_event_command! {
    ExplorerRefresh,
    id: "explorer_refresh",
    description: "Refresh explorer view",
}

// Register as command
ctx.register_command(ExplorerRefresh);

// Subscribe as event (same type!)
bus.subscribe::<ExplorerRefresh, _>(100, |_event, ctx| {
    ctx.state.with_mut::<ExplorerState, _, _>(|state| {
        state.refresh();
        EventResult::Handled
    }).unwrap_or(EventResult::NotHandled)
});
```

**When to use:**
- Handler has access to all needed state via `PluginStateRegistry`
- No parameters required from command
- Simple trigger/notification
- **Preferred for most plugin commands**

**Benefits:**
- 50% fewer types (one instead of two)
- ~20 lines of boilerplate eliminated per command
- Minimal memory overhead (zero-sized type)
- Implements `Copy` - cheap to clone
- Simple to construct with `Default`

## Legacy: Separate Event Types (Deprecated)

**Old approach:** Define standalone event types (no longer recommended).

```rust
// DON'T DO THIS - use declare_event_command! instead
#[derive(Debug, Clone, Copy, Default)]
pub struct ExplorerRefreshEvent;

impl Event for ExplorerRefreshEvent {}

// Requires separate command type too (20+ lines of boilerplate)
```

## Unified Types with Data (Counted Commands)

Use `declare_counted_event_command!` for commands that use repeat counts:

```rust
use reovim_core::declare_counted_event_command;

// Single type with count - both command AND event
declare_counted_event_command! {
    ExplorerCursorDown,
    id: "explorer_cursor_down",
    description: "Move cursor down",
}

// Register (Default provides count=1)
ctx.register_command(ExplorerCursorDown::new(1));

// Subscribe - access event.count directly
bus.subscribe::<ExplorerCursorDown, _>(100, |event, ctx| {
    for _ in 0..event.count {
        // Move down
    }
    EventResult::Handled
});
```

**When to use:**
- Command receives repeat count (e.g., `5j` for "down 5 times")
- Handler needs count parameter
- **Preferred over separate Command/Event types**

## Custom Data Fields

For events with custom data that don't fit the standard macros:

```rust
use reovim_core::event_bus::Event;

#[derive(Debug, Clone, Copy)]
pub struct ExplorerInputChar {
    pub c: char,
}

impl ExplorerInputChar {
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl Event for ExplorerInputChar {
    fn priority(&self) -> u32 { 100 }
}

// Still use same type for both command and event
bus.subscribe::<ExplorerInputChar, _>(100, |event, ctx| {
    // Access event.c
    EventResult::Handled
});
```

**When to use:**
- Command receives custom parameters (char, string, complex data)
- Handler needs specific context not available in state
- Information passed from external source

**Benefits:**
- Explicit context passing
- Type-safe parameters
- Self-documenting through fields
- Still uses single type for both command and event

## Decision Guidelines

| Pattern | When to Use |
|---------|-------------|
| `declare_event_command!` | State in registry, no params, simple trigger (**default choice**) |
| `declare_counted_event_command!` | Need count from command (e.g., `5j`) |
| Custom implementation | Passing custom data, external context required |

## Common Patterns

### Pattern 1: Simple Toggle

```rust
// Unified type - state knows whether it's on/off
declare_event_command! {
    Toggle,
    id: "toggle",
    description: "Toggle feature",
}
```

### Pattern 2: Counted Action

```rust
// Unified with count - count affects behavior
declare_counted_event_command! {
    Repeat,
    id: "repeat",
    description: "Repeat action",
}
```

### Pattern 3: Custom Data

```rust
// Custom implementation for specific data
#[derive(Debug, Clone)]
pub struct MoveInDirection {
    pub direction: Direction,
    pub count: usize,
}

impl Event for MoveInDirection {
    fn priority(&self) -> u32 { 100 }
}
```

### Pattern 4: State Change Notification

```rust
// Unified type - handler queries state
declare_event_command! {
    StateChanged,
    id: "state_changed",
    description: "State has changed",
}
```

## Example: Complete Plugin Command

```rust
use reovim_core::{
    declare_event_command,
    event_bus::{EventBus, EventResult, HandlerContext},
    plugin::{Plugin, PluginContext, PluginStateRegistry},
};
use std::sync::Arc;

declare_event_command! {
    MyAction,
    id: "my_action",
    description: "Perform my action",
}

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str { "my-plugin" }

    fn build(&self, ctx: &mut PluginContext) {
        ctx.commands.register(MyAction);
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        bus.subscribe::<MyAction, _>(100, |_event, ctx| {
            tracing::info!("MyAction triggered!");
            ctx.request_render();
            EventResult::Handled
        });
    }
}
```

## Related Documentation

- [Event Bus](./eventbus.md) - EventBus API
- [Plugin Tutorial](../plugins/tutorial.md) - Getting started
- [Plugin Advanced](../plugins/advanced.md) - Advanced patterns
