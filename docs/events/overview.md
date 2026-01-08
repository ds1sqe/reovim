# Event System Overview

The event system handles all input and internal communication in reovim.

## Module Structure

```
lib/core/src/
├── event_bus/          # Type-erased event system
│   ├── mod.rs          # Event trait, DynEvent, exports
│   ├── bus.rs          # EventBus implementation
│   ├── core_events.rs  # Core event definitions
│   └── scope.rs        # EventScope for lifecycle tracking
│
├── event/              # Legacy event system
│   ├── mod.rs          # Trait definitions, exports
│   ├── input.rs        # InputEventBroker
│   ├── key/
│   │   └── mod.rs      # KeyEventBroker
│   ├── handler/
│   │   ├── mod.rs      # TerminateHandler
│   │   ├── command/    # CommandHandler
│   │   │   ├── mod.rs
│   │   │   ├── dispatcher.rs
│   │   │   └── count_parser.rs
│   │   └── completion.rs # CompletionHandler
│   └── inner/
│       └── mod.rs      # RuntimeEventPayload enum
```

## Two Event Systems

### 1. Event Bus (Type-Erased)

The event bus provides a type-erased event system for plugin communication. Unlike `RuntimeEventPayload`, the event bus allows plugins to define their own event types without modifying core enums.

**Location:** `lib/core/src/event_bus/`

**Use for:**
- Plugin-to-plugin communication
- Custom plugin events
- Extensible event types

### 2. RuntimeEventPayload

Internal events passed to the runtime via mpsc channel. Used for core system events.

**Location:** `lib/core/src/event/inner/mod.rs`

**Use for:**
- Buffer operations
- Window management
- Mode changes
- Core command execution

## Event Flow Diagram

```
1. TERMINAL INPUT
   crossterm EventStream (async)
        │
        ▼
2. INPUT BROKER
   InputEventBroker::subscribe()
   - Filters Key Press events
   - Wraps in ScopedKeyEvent with optional scope
   - Calls KeyEventBroker::handle()
        │
        ▼
3. KEY BROADCAST
   KeyEventBroker (tokio broadcast, buffer: 255)
        │
        ├────────────────────┐
        ▼                    ▼
4. HANDLERS
   CommandHandler        TerminateHandler
   - Keys → Commands     - Ctrl+C → Kill
   - Track pending_keys (KeySequence)
   - Watch mode changes
   - Uses Dispatcher for events
        │                    │
        ▼                    ▼
   RuntimeEventPayload   RuntimeEvent::kill()
        │                    │
        └──────────┬─────────┘
                   ▼
5. RUNTIME EVENT LOOP
   Runtime::rx.recv().await (RuntimeEvent)

   match event.into_payload() {
       Buffer(_) => handle buffer operations
       Window(_) => handle window operations
       Command(_) => execute command via registry
       Editing(EditingEvent::TextInput(_)) => route to component
       Editing(EditingEvent::OperatorMotion(_)) => execute operator
       Mode(ModeEvent::Change(_)) => update mode, broadcast
       Mode(ModeEvent::PendingKeys(_)) => update status line
       Render(RenderEvent::Signal) => trigger render
       Render(RenderEvent::Syntax(_)) => update syntax provider
       Settings(_) => apply setting change
       File(_) => open/navigate to file
       Plugin(_) => emit via EventBus to plugin handlers
       Kill => exit
   }
        │
        ▼
6. EVENT BUS (Plugin Events)
   EventBus handles plugin-defined events:
   - Completion, Explorer, Microscope, etc.
   - Plugins subscribe with handlers
   - May request render or emit more events
        │
        ▼
7. RENDERING
   Screen::render()
```

## Related Documentation

- [Event Bus](./eventbus.md) - Type-erased event system
- [Runtime Events](./runtime-events.md) - RuntimeEventPayload
- [Patterns](./patterns.md) - Event design patterns
- [Handlers](./handlers.md) - CommandHandler, TerminateHandler
- [Keybindings](./keybindings.md) - Key translation
