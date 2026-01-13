# Plugin Development Tutorial

This beginner-friendly guide walks you through creating your first reovim plugin.

## Prerequisites

- Rust 1.92+ (2024 edition)
- Basic understanding of Rust traits and async
- Familiarity with reovim's basic usage

## Overview

Reovim plugins are Rust crates that implement the `Plugin` trait. Plugins can:
- Register commands and keybindings
- Subscribe to events
- Provide UI overlays
- Extend the editor's functionality

## Step 1: Create Plugin Crate

Create a new plugin in the `plugins/features/` directory:

```bash
cd plugins/features
cargo new --lib my-plugin
cd my-plugin
```

Update `Cargo.toml`:

```toml
[package]
name = "reovim-plugin-my-plugin"
version = "0.1.0"
edition = "2024"

[dependencies]
reovim-core = { path = "../../../lib/core" }
tracing = "0.1"
```

## Step 2: Implement the Plugin Trait

Create `src/lib.rs`:

```rust
use reovim_core::plugin::{Plugin, PluginContext};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str {
        "my-plugin"
    }

    fn build(&self, ctx: &mut PluginContext) {
        tracing::info!("MyPlugin initialized!");
    }
}
```

## Step 3: Add a Simple Command

Commands are the primary way plugins add functionality. Use the `declare_event_command!` macro:

```rust
use reovim_core::{
    declare_event_command,
    command::{CommandResult, ExecutionContext},
    plugin::{Plugin, PluginContext},
};

// Declare a command that's also an event
declare_event_command! {
    MyGreeting,
    id: "my_greeting",
    description: "Display a greeting message",
}

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str {
        "my-plugin"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register the command
        ctx.commands.register(MyGreeting);
    }
}
```

## Step 4: Handle Events

Subscribe to your command's event to execute logic:

```rust
use reovim_core::{
    declare_event_command,
    command::{CommandResult, ExecutionContext},
    event_bus::{EventBus, EventResult, HandlerContext},
    plugin::{Plugin, PluginContext, PluginStateRegistry},
};
use std::sync::Arc;

declare_event_command! {
    MyGreeting,
    id: "my_greeting",
    description: "Display a greeting message",
}

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str {
        "my-plugin"
    }

    fn build(&self, ctx: &mut PluginContext) {
        ctx.commands.register(MyGreeting);
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Handle the MyGreeting event
        bus.subscribe::<MyGreeting, _>(100, |_event, ctx| {
            tracing::info!("Hello from MyPlugin!");
            ctx.request_render();
            EventResult::Handled
        });
    }
}
```

## Step 5: Add Keybindings

Bind your command to a key sequence:

```rust
use reovim_core::{
    bind::{KeyMap, KeymapScope},
    keystroke::KeySequence,
    modd::EditModeKind,
};

impl Plugin for MyPlugin {
    // ... name() and subscribe() ...

    fn build(&self, ctx: &mut PluginContext) {
        // Register command
        ctx.commands.register(MyGreeting);

        // Bind to "g h" in normal mode
        ctx.keymap.bind(
            KeymapScope::DefaultNormal,
            KeySequence::parse("g h").unwrap(),
            MyGreeting,
        );
    }
}
```

## Step 6: Register the Plugin

Add your plugin to `runner/src/plugins.rs`:

```rust
use reovim_plugin_my_plugin::MyPlugin;

pub fn load_all_plugins(loader: &mut PluginLoader) {
    // ... existing plugins ...
    loader.add(MyPlugin);
}
```

Also add the dependency to `runner/Cargo.toml`:

```toml
[dependencies]
reovim-plugin-my-plugin = { path = "../plugins/features/my-plugin" }
```

## Step 7: Build and Test

```bash
# Build the editor
cargo build

# Run and test your keybinding
cargo run -- test.txt
# Press "g h" in normal mode
```

## Complete Example

Here's the complete `src/lib.rs`:

```rust
use reovim_core::{
    bind::{KeyMap, KeymapScope},
    command::{CommandResult, ExecutionContext},
    declare_event_command,
    event_bus::{EventBus, EventResult, HandlerContext},
    keystroke::KeySequence,
    modd::EditModeKind,
    plugin::{Plugin, PluginContext, PluginStateRegistry},
};
use std::sync::Arc;

declare_event_command! {
    MyGreeting,
    id: "my_greeting",
    description: "Display a greeting message",
}

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str {
        "my-plugin"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register command
        ctx.commands.register(MyGreeting);

        // Bind to "g h" in normal mode
        ctx.keymap.bind(
            KeymapScope::DefaultNormal,
            KeySequence::parse("g h").unwrap(),
            MyGreeting,
        );
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        bus.subscribe::<MyGreeting, _>(100, |_event, ctx| {
            tracing::info!("Hello from MyPlugin!");
            ctx.request_render();
            EventResult::Handled
        });
    }
}
```

## Next Steps

- **Add state**: Use `PluginStateRegistry` to store plugin state
- **Create UI**: Implement `OverlayRenderer` or `PluginWindow` for visual elements
- **Handle input**: Subscribe to `PluginTextInput` for character input
- **Register options**: Use `RegisterOption` to add configurable settings

See [Plugin Advanced Guide](./advanced.md) for these topics.

## Common Patterns

### Commands with Count

For commands that accept a repeat count (like `3j`):

```rust
use reovim_core::declare_counted_event_command;

declare_counted_event_command! {
    MyMove,
    id: "my_move",
    description: "Move by count",
}

// In handler:
bus.subscribe::<MyMove, _>(100, |event, ctx| {
    let count = event.count.unwrap_or(1);
    // Use count...
    EventResult::Handled
});
```

### Emitting Events from Handlers

```rust
bus.subscribe::<SomeEvent, _>(100, |event, ctx| {
    // Emit another event
    ctx.emit(AnotherEvent { data: "hello" });
    EventResult::Handled
});
```

### Requesting Mode Change

```rust
use reovim_core::modd::ModeState;

bus.subscribe::<MyEvent, _>(100, |event, ctx| {
    ctx.request_mode(ModeState::insert());
    EventResult::Handled
});
```

## Debugging

Enable debug logging to see your plugin's output:

```bash
REOVIM_LOG=debug cargo run -- test.txt
```

Or write to a log file:

```bash
cargo run -- --log=/tmp/reovim.log test.txt
tail -f /tmp/reovim.log
```

## Related Documentation

- [Plugin System](./system.md) - Full plugin API reference
- [Plugin Advanced Guide](./advanced.md) - Advanced patterns
- [Event System](../events/overview.md) - Event flow details
- [Commands](../reference/commands.md) - Command system reference
