# Plugin Development Overview

Reovim's plugin system enables modular feature development with full lifecycle management.

## Quick Start

1. Create a new plugin crate in `plugins/features/`
2. Implement the `Plugin` trait
3. Register commands, keybindings, and state
4. Subscribe to events via EventBus
5. Add the plugin to `runner/src/plugins.rs`

See [Tutorial](./tutorial.md) for a step-by-step guide.

## Plugin Lifecycle

```
1. build()      → Register commands, keybindings
2. init_state() → Initialize plugin state in registry
3. finish()     → Post-registration setup
4. subscribe()  → Subscribe to events via event bus
```

## Key Concepts

| Concept | Description |
|---------|-------------|
| `Plugin` trait | Interface all plugins implement |
| `PluginContext` | Access to keymaps, commands, registries during build |
| `PluginStateRegistry` | Type-erased state storage for plugins |
| `EventBus` | Type-erased event system for plugin communication |
| `CommandRef` | Reference to a registered command |

## Documentation

| Doc | Description |
|-----|-------------|
| [Tutorial](./tutorial.md) | Beginner guide - create your first plugin |
| [Advanced](./advanced.md) | Advanced patterns and techniques |
| [System](./system.md) | Full plugin API reference |

## Example Plugin

```rust
use reovim_core::{
    declare_event_command,
    event_bus::{EventBus, EventResult},
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

- [Architecture](../architecture/plugins.md) - Plugin system architecture
- [Event System](../events/overview.md) - Event bus and patterns
- [Rendering](../rendering/ui-systems.md) - Plugin UI systems
