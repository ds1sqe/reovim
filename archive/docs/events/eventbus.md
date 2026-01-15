# Event Bus

Type-erased event system for plugin communication.

## Location

`lib/core/src/event_bus/`

## Event Trait

```rust
// Event trait - implemented by all plugin events
pub trait Event: Send + Sync + 'static {
    fn priority(&self) -> u32 { 100 }  // Lower = higher priority
}
```

## EventBus API

```rust
// EventBus - type-erased event dispatch
pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<EventHandler>>>,
}

impl EventBus {
    // Subscribe to events of type E
    pub fn subscribe<E: Event, F>(&self, priority: u32, handler: F)
    where
        F: Fn(&E, &mut HandlerContext) -> EventResult + Send + Sync + 'static;

    // Emit an event to all subscribers
    pub fn emit<E: Event>(&self, event: E);
}
```

## EventResult

```rust
pub enum EventResult {
    Handled,      // Event was handled, continue processing other handlers
    Consumed,     // Event was consumed, stop propagation to other handlers
    NotHandled,   // Event was not handled by this handler
    NeedsRender,  // Request a render after all handlers complete
    Quit,         // Editor should quit
}
```

## Basic Usage

```rust
// Define event
#[derive(Debug, Clone)]
pub struct MyActionEvent {
    pub data: String,
}

impl Event for MyActionEvent {
    fn priority(&self) -> u32 { 100 }
}

// Subscribe in plugin
bus.subscribe::<MyActionEvent, _>(100, |event, _ctx| {
    tracing::trace!(data = ?event.data, "Action triggered");
    EventResult::Handled
});

// Emit from plugin or runtime
event_bus.emit(MyActionEvent {
    data: "example".to_string(),
});
```

## HandlerContext Methods

The `HandlerContext` passed to event handlers provides several utility methods:

### Basic Methods

```rust
// Emit a new event
ctx.emit(SomeEvent { ... });

// Request a render after event processing
ctx.request_render();

// Request the editor to quit
ctx.request_quit();
```

### Mode Change Helpers

```rust
// Enter interactor mode for a plugin component
// Sets both interactor_id and SubMode::Interactor to the component
ctx.enter_interactor_mode(COMPONENT_ID);

// Return to normal editor mode
ctx.exit_to_normal();

// Set arbitrary mode state (for edge cases)
ctx.set_mode(ModeState::with_interactor_id_and_mode(
    COMPONENT_ID,
    EditMode::Normal,
));
```

**When to use:**
- `enter_interactor_mode(id)` - Plugin needs to receive text input (like search, rename dialogs)
- `exit_to_normal()` - Return to normal editing after plugin operation completes
- `set_mode(mode)` - Custom mode transitions that don't fit the standard patterns

## Event Scope Tracking

**Location:** `lib/core/src/event_bus/scope.rs`

EventScope provides GC-like tracking of event lifecycles for deterministic synchronization:

```rust
use reovim_core::event_bus::{EventScope, ScopeId};

// Create a scope to track events
let scope = EventScope::new();

// Increment when emitting events
scope.increment();  // Event 1 emitted
scope.increment();  // Event 2 emitted (child)

// Decrement when events complete
scope.decrement();  // Event 1 done
scope.decrement();  // Event 2 done, counter = 0

// Wait for all events in scope to complete
scope.wait().await;  // Returns when counter = 0
```

### Key Methods

| Method | Description |
|--------|-------------|
| `EventScope::new()` | Create new scope with counter = 0 |
| `scope.increment()` | Track new event (counter++) |
| `scope.decrement()` | Mark event complete (counter--), notify if zero |
| `scope.wait()` | Async wait for counter to reach 0 |
| `scope.wait_timeout(duration)` | Wait with timeout, returns `false` if timed out |
| `scope.in_flight()` | Get current counter value |
| `scope.is_complete()` | Check if counter is 0 |

### Debugging Stuck Scopes

Enable trace logging to see scope lifecycle:

```bash
REOVIM_LOG=trace reovim myfile.txt
```

Output:
```
DEBUG EventScope created                    scope_id=1
TRACE EventScope increment                  scope_id=1 in_flight=1
TRACE EventScope increment                  scope_id=1 in_flight=2
TRACE EventScope decrement                  scope_id=1 in_flight=1
TRACE EventScope decrement                  scope_id=1 in_flight=0
DEBUG EventScope completed                  scope_id=1 elapsed_ms=5
```

If a scope hangs, look for increments without matching decrements to find the leak.

## Core Events

Core events are defined in `lib/core/src/event_bus/core_events.rs`:

| Event | Description |
|-------|-------------|
| `RequestSetRegister` | Set register content (unnamed or named) |
| `RegisterExCommand` | Plugin registers an ex-command handler |
| `RegisterConfigurable` | Component registers for profile system |
| `ProfileListEvent` | Profile list requested |
| `ProfileLoadEvent` | Profile load requested |
| `ProfileSaveEvent` | Profile save requested |

## Option Events

Option events are defined in `lib/core/src/option/events.rs`:

| Event | Priority | Description |
|-------|----------|-------------|
| `RegisterSettingSection` | 40 | Plugin registers a settings section with UI metadata |
| `RegisterOption` | 50 | Plugin registers an option specification |
| `OptionChanged` | 100 | Option value was modified |
| `QueryOption` | 100 | Request to query an option value |
| `ResetOption` | 100 | Request to reset option to default |

### RegisterSettingSection

```rust
use reovim_core::option::RegisterSettingSection;

bus.emit(RegisterSettingSection::new("my_plugin", "My Plugin")
    .with_description("Plugin-specific settings")
    .with_order(100));  // Core sections use 0-50
```

### OptionChanged

```rust
use reovim_core::option::{OptionChanged, ChangeSource};

bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
    if event.name == "plugin.treesitter.highlight_timeout_ms" {
        if let Some(timeout) = event.as_int() {
            // Update internal timeout setting
        }
        ctx.request_render();
    }
    EventResult::Continue
});
```

`ChangeSource` variants:
- `UserCommand` - User typed a `:set` command
- `ProfileLoad` - Loaded from a profile file
- `Plugin` - Changed programmatically by a plugin
- `SettingsMenu` - Changed via the settings menu UI
- `Rpc` - Changed via RPC (server mode)
- `Default` - Default value applied during initialization

## Plugin Events

Example plugin events:

| Event | Description |
|-------|-------------|
| `ExplorerRefresh` | Refresh explorer view |
| `MicroscopeOpen` | Open microscope picker |
| `CompletionTrigger` | Trigger completion |
| `SettingsMenuOpen` | Open settings menu |

## Related Documentation

- [Patterns](./patterns.md) - Event design patterns
- [Runtime Events](./runtime-events.md) - RuntimeEventPayload
- [Plugin System](../architecture/plugins.md) - Plugin lifecycle
