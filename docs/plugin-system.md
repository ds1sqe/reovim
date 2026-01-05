# Plugin System

The plugin system enables modular, extensible features with full lifecycle management and type-erased event communication.

## Overview

```
reovim/
├── lib/core/src/
│   ├── plugin/                 # Plugin infrastructure
│   │   ├── mod.rs              # Public exports
│   │   ├── traits.rs           # Plugin trait definition
│   │   ├── context.rs          # PluginContext for registration
│   │   ├── loader.rs           # PluginLoader with dependency resolution
│   │   ├── state.rs            # PluginStateRegistry
│   │   ├── statusline.rs       # StatuslineSectionProvider trait
│   │   ├── runtime_context.rs  # RuntimeContext for plugins
│   │   └── builtin/            # Built-in plugins (shipped with core)
│   │       ├── core.rs         # CorePlugin - essential commands
│   │       ├── leap.rs         # LeapPlugin - two-char motion
│   │       └── window.rs       # WindowPlugin - window management
│   │
│   └── event_bus/              # Type-erased event system
│       ├── mod.rs              # Event trait, DynEvent
│       └── bus.rs              # EventBus implementation
│
├── plugins/features/           # External feature plugins
│   ├── completion/             # Text completion
│   ├── explorer/               # File browser
│   ├── fold/                   # Code folding
│   ├── leap/                   # Two-char motion
│   ├── lsp/                    # LSP integration
│   ├── microscope/             # Fuzzy finder
│   ├── notification/           # Toast notifications and progress bars
│   ├── pair/                   # Auto-pair brackets
│   ├── pickers/                # Picker UI components
│   ├── settings-menu/          # In-editor settings
│   ├── statusline/             # Statusline extension API
│   ├── treesitter/             # Syntax highlighting infrastructure
│   └── which-key/              # Keybinding hints
│
├── plugins/languages/          # Language plugins
│   ├── rust/                   # Rust support
│   ├── c/                      # C support
│   ├── javascript/             # JavaScript support
│   ├── python/                 # Python support
│   ├── json/                   # JSON support
│   ├── toml/                   # TOML support
│   └── markdown/               # Markdown support
│
└── runner/src/
    └── plugins.rs              # AllPlugins - combines all plugins
```

## Plugin Trait

All plugins implement the `Plugin` trait:

```rust
pub trait Plugin: Send + Sync + 'static {
    /// Unique identifier for this plugin
    fn id(&self) -> PluginId;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Short description
    fn description(&self) -> &'static str;

    /// Dependencies (TypeIds of other plugins)
    fn dependencies(&self) -> Vec<TypeId> { vec![] }

    /// Register commands and keybindings
    fn build(&self, ctx: &mut PluginContext);

    /// Initialize plugin state in registry
    fn init_state(&self, registry: &PluginStateRegistry) {}

    /// Post-registration setup
    fn finish(&self, ctx: &mut PluginContext) {}

    /// Subscribe to events via event bus
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {}
}
```

## Plugin Lifecycle

Plugins go through four phases during initialization:

```
┌─────────────────────────────────────────────────────────────┐
│  1. build()       │ Register commands, keybindings          │
├───────────────────┼─────────────────────────────────────────┤
│  2. init_state()  │ Initialize state in PluginStateRegistry │
├───────────────────┼─────────────────────────────────────────┤
│  3. finish()      │ Post-registration setup                 │
├───────────────────┼─────────────────────────────────────────┤
│  4. subscribe()   │ Subscribe to events via EventBus        │
└───────────────────┴─────────────────────────────────────────┘
```

**Important:** Plugins are loaded in dependency order. If plugin B depends on plugin A, A's lifecycle methods run before B's.

## PluginContext

Used during `build()` and `finish()` to register commands, components, and keybindings:

```rust
pub struct PluginContext {
    pub command_registry: Arc<CommandRegistry>,
    pub keymap: Arc<KeyMap>,
    plugin_state: Arc<PluginStateRegistry>,
}

impl PluginContext {
    /// Register a command
    pub fn register_command<C: CommandTrait + 'static>(&mut self, cmd: C) -> Result<(), PluginError>;

    /// Register a keybinding
    pub fn register_keybinding(&mut self, mode: &str, keys: &str, command_id: CommandId);

    /// Create an option builder for registering plugin options
    pub fn option(&mut self, name: &'static str) -> PluginOptionBuilder<'_>;
}
```

## Plugin Options

Plugins can register their own configurable options using the fluent builder API:

```rust
fn build(&self, ctx: &mut PluginContext) {
    // Register a boolean option
    ctx.option("enabled")
        .description("Enable this feature")
        .default_bool(true)
        .register()?;

    // Register an integer option with constraints
    ctx.option("timeout_ms")
        .short("to")  // Short alias for :set to=200
        .description("Timeout in milliseconds")
        .default_int(100)
        .min(10)
        .max(5000)
        .register()?;

    // Register a choice option
    ctx.option("mode")
        .description("Operating mode")
        .default_choice("auto", &["auto", "manual", "disabled"])
        .register()?;
}
```

### Option Types

| Type | Builder Method | Example |
|------|----------------|---------|
| Boolean | `.default_bool(true)` | `:set plugin.myplugin.enabled` |
| Integer | `.default_int(100)` | `:set plugin.myplugin.timeout=200` |
| String | `.default_string("value")` | `:set plugin.myplugin.path=/tmp` |
| Choice | `.default_choice("a", &["a", "b"])` | `:set plugin.myplugin.mode=manual` |

### Constraints

```rust
ctx.option("count")
    .default_int(5)
    .min(1)          // Minimum value
    .max(100)        // Maximum value
    .register()?;

ctx.option("count")
    .default_int(5)
    .range(1, 100)   // Shorthand for min + max
    .register()?;
```

### User Commands

Users can interact with plugin options via `:set`:

| Command | Action |
|---------|--------|
| `:set plugin.myplugin.enabled` | Enable boolean option |
| `:set noplugin.myplugin.enabled` | Disable boolean option |
| `:set plugin.myplugin.timeout=200` | Set value |
| `:set plugin.myplugin.timeout?` | Query current value |
| `:set plugin.myplugin.timeout&` | Reset to default |

### Reacting to Option Changes

Subscribe to `OptionChanged` events to react when options are modified:

```rust
use reovim_core::option::{OptionChanged, ChangeSource};

fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
    let state_clone = Arc::clone(&state);
    bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
        if event.name.starts_with("plugin.myplugin.") {
            // React to setting change
            if let Some(timeout) = event.as_int() {
                state_clone.with_mut::<MyState, _, _>(|s| {
                    s.set_timeout(timeout as u64);
                });
            }
            ctx.request_render();
        }
        EventResult::Continue
    });
}
```

### Profile Persistence

Plugin options are automatically saved to profiles under nested TOML sections:

```toml
# ~/.config/reovim/profiles/default.toml
[plugin.treesitter]
highlight_timeout_ms = 100
incremental_parse = true

[plugin.completion]
auto_trigger = true
min_prefix = 2
```

## Extensible Settings Menu

Plugins can register their settings to appear in the in-editor settings menu. This is done via the `RegisterSettingSection` and `RegisterOption` events during the `subscribe()` phase.

### Registering Settings Sections

Define sections with custom display name, description, and ordering:

```rust
use reovim_core::option::RegisterSettingSection;

fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
    // Register a settings section
    bus.emit(RegisterSettingSection::new("treesitter", "Treesitter")
        .with_description("Syntax highlighting settings")
        .with_order(100));  // Core sections use 0-50, plugins use 100+
}
```

### Registering Settings Options

Register individual options with full metadata for the settings menu:

```rust
use reovim_core::option::{
    RegisterOption, OptionSpec, OptionValue, OptionCategory, OptionConstraint,
};

fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
    // Boolean option
    bus.emit(RegisterOption::new(
        OptionSpec::new("enabled", "Enable feature", OptionValue::Bool(true))
            .with_category(OptionCategory::Plugin("myplugin".into()))
            .with_section("My Plugin")  // Section name (creates if doesn't exist)
            .with_display_order(10),    // Order within section
    ));

    // Integer option with constraints
    bus.emit(RegisterOption::new(
        OptionSpec::new("timeout_ms", "Timeout in ms", OptionValue::Integer(100))
            .with_category(OptionCategory::Plugin("myplugin".into()))
            .with_section("My Plugin")
            .with_constraint(OptionConstraint::range(10, 5000))
            .with_display_order(20),
    ));

    // Choice option
    bus.emit(RegisterOption::new(
        OptionSpec::new(
            "mode",
            "Operating mode",
            OptionValue::choice("auto", vec!["auto".into(), "manual".into()]),
        )
        .with_category(OptionCategory::Plugin("myplugin".into()))
        .with_section("My Plugin")
        .with_display_order(30),
    ));
}
```

### OptionSpec UI Metadata

| Field | Type | Description |
|-------|------|-------------|
| `section` | `Option<Cow<str>>` | Section name (falls back to category) |
| `display_order` | `u32` | Order within section (lower = earlier) |
| `show_in_menu` | `bool` | Whether to show in settings menu (default: true) |

### Built-in Sections

Core registers these sections with low order values:
- `Editor` (order: 0) - Line numbers, tabs, indent guides
- `Display` (order: 10) - Theme, color mode
- `Window` (order: 20) - Split behavior

Plugins should use order values >= 100 to appear after core sections.

### Listening to Option Changes

Subscribe to `OptionChanged` events to react when settings are modified:

```rust
use reovim_core::option::{OptionChanged, ChangeSource};

fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
    let state_clone = Arc::clone(&state);
    bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
        // React to settings menu changes
        if event.source == ChangeSource::SettingsMenu
            && event.name.starts_with("plugin.myplugin.")
        {
            // Update plugin state
            ctx.request_render();
        }
        EventResult::Handled
    });
}
```

## Registering Ex-Commands

Plugins can register custom ex-commands (`:command`) that users can invoke from the command line. The ex-command registry supports three patterns: zero-arg, single-arg, and subcommand.

### Ex-Command Handler Types

```rust
pub enum ExCommandHandler {
    /// Zero-arg command: `:name`
    ZeroArg {
        event_constructor: fn() -> DynEvent,
        description: &'static str,
    },

    /// Single string arg: `:name arg`
    SingleArg {
        event_constructor: fn(String) -> DynEvent,
        description: &'static str,
    },

    /// Subcommand: `:name subcommand [arg]`
    Subcommand {
        subcommands: HashMap<String, Box<Self>>,
        description: &'static str,
    },
}
```

### Registering Commands

Commands are registered by emitting a `RegisterExCommand` event during the plugin's `subscribe()` phase:

```rust
use reovim_core::{
    command_line::ExCommandHandler,
    event_bus::{core_events::RegisterExCommand, DynEvent},
};

impl Plugin for MyPlugin {
    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register a zero-arg command: :myplugin
        bus.emit(RegisterExCommand::new(
            "myplugin",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(MyPluginOpen),
                description: "Open my plugin",
            },
        ));
    }
}
```

### Zero-Arg Commands

Simple commands with no arguments:

```rust
// Register :settings command
bus.emit(RegisterExCommand::new(
    "settings",
    ExCommandHandler::ZeroArg {
        event_constructor: || DynEvent::new(SettingsMenuOpen),
        description: "Open the settings menu",
    },
));
```

When the user types `:settings`, the runtime:
1. Looks up the command in the registry
2. Calls the `event_constructor()` to create a `SettingsMenuOpen` event
3. Dispatches the event through the event bus
4. The plugin's event handler receives it and opens the settings menu

### Single-Arg Commands

Commands that take one string argument:

```rust
// Register :loadconfig <name> command
bus.emit(RegisterExCommand::new(
    "loadconfig",
    ExCommandHandler::SingleArg {
        event_constructor: |name| DynEvent::new(LoadConfigEvent { name }),
        description: "Load a configuration by name",
    },
));
```

When the user types `:loadconfig myconfig`, the runtime:
1. Parses the command name (`loadconfig`) and argument (`myconfig`)
2. Calls `event_constructor("myconfig")` to create the event
3. Dispatches the event with the provided name

### Subcommand Pattern

Commands with multiple subcommands (like `:profile list`, `:profile load`, `:profile save`):

```rust
use std::collections::HashMap;

// Build subcommand map
let mut subcommands = HashMap::new();

// :profile list - zero-arg subcommand
subcommands.insert(
    "list".to_string(),
    Box::new(ExCommandHandler::ZeroArg {
        event_constructor: || DynEvent::new(ProfileListEvent),
        description: "List all available profiles",
    })
);

// :profile load <name> - single-arg subcommand
subcommands.insert(
    "load".to_string(),
    Box::new(ExCommandHandler::SingleArg {
        event_constructor: |name| DynEvent::new(ProfileLoadEvent { name }),
        description: "Load a profile by name",
    })
);

// :profile save <name> - single-arg subcommand
subcommands.insert(
    "save".to_string(),
    Box::new(ExCommandHandler::SingleArg {
        event_constructor: |name| DynEvent::new(ProfileSaveEvent { name }),
        description: "Save current settings as a profile",
    })
);

// Register the main command with subcommands
bus.emit(RegisterExCommand::new(
    "profile",
    ExCommandHandler::Subcommand {
        subcommands,
        description: "Manage configuration profiles",
    },
));
```

### Complete Example

Here's a complete plugin that registers ex-commands:

```rust
use std::{collections::HashMap, sync::Arc};
use reovim_core::{
    command_line::ExCommandHandler,
    event_bus::{
        core_events::RegisterExCommand,
        DynEvent, Event, EventBus, EventResult,
    },
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

// Define events
#[derive(Debug, Clone, Copy)]
pub struct MyPluginOpen;

impl Event for MyPluginOpen {
    fn priority(&self) -> u32 { 100 }
}

#[derive(Debug, Clone)]
pub struct MyPluginLoad {
    pub name: String,
}

impl Event for MyPluginLoad {
    fn priority(&self) -> u32 { 100 }
}

// Plugin implementation
pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("my_plugin")
    }

    fn build(&self, _ctx: &mut PluginContext) {
        // Commands are registered in subscribe(), not build()
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register ex-commands
        bus.emit(RegisterExCommand::new(
            "myplugin",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(MyPluginOpen),
                description: "Open my plugin",
            },
        ));

        bus.emit(RegisterExCommand::new(
            "loadmyplugin",
            ExCommandHandler::SingleArg {
                event_constructor: |name| DynEvent::new(MyPluginLoad { name }),
                description: "Load my plugin configuration",
            },
        ));

        // Subscribe to events
        bus.subscribe::<MyPluginOpen, _>(100, |_event, ctx| {
            tracing::info!("My plugin opened!");
            ctx.request_render();
            EventResult::Handled
        });

        bus.subscribe::<MyPluginLoad, _>(100, |event, ctx| {
            tracing::info!("Loading configuration: {}", event.name);
            ctx.request_render();
            EventResult::Handled
        });
    }
}
```

### Best Practices

1. **Use descriptive command names** - Clear, concise names like `:settings`, `:profile`, `:reload`
2. **Provide descriptions** - Always include descriptions for help/completion systems
3. **Register in subscribe()** - Ex-commands are registered during the `subscribe()` phase, not `build()`
4. **Use subcommands for related operations** - Group related commands under a single prefix (`:profile load/save/list`)
5. **Emit events, don't execute directly** - The handler should create and emit an event that your plugin handles
6. **Handle errors gracefully** - If the command fails, log a warning but don't crash

### Event Flow

```
User types :mycommand arg
    ↓
Runtime parses ex-command
    ↓
ExCommand::Plugin { command: "mycommand arg" }
    ↓
Runtime handler calls ex_command_registry.dispatch("mycommand arg")
    ↓
Registry finds "mycommand" handler
    ↓
Calls event_constructor("arg") → creates DynEvent
    ↓
EventBus.dispatch(&event)
    ↓
Plugin's event handler receives it
    ↓
Plugin executes the action
```

### Migration from Hardcoded Commands

Before the ex-command registry, plugins had hardcoded `ExCommand` variants:

```rust
// OLD (deprecated)
ExCommand::Settings => {
    // Hardcoded in core - BAD!
}
```

Now, plugins register their own commands:

```rust
// NEW (correct)
bus.emit(RegisterExCommand::new(
    "settings",
    ExCommandHandler::ZeroArg {
        event_constructor: || DynEvent::new(SettingsMenuOpen),
        description: "Open the settings menu",
    },
));
```

This decouples core from plugin-specific commands and allows any plugin to register custom ex-commands.

## Profile System

Plugins can register configurable components that participate in profile save/load operations. This allows plugins to persist their settings and restore them when profiles are loaded.

### Configurable Trait

The `Configurable` trait defines how a component serializes/deserializes its configuration:

```rust
pub trait Configurable {
    /// Get the configuration section name (e.g., "core", "treesitter", "completion")
    fn config_section(&self) -> &'static str;

    /// Serialize current state to configuration data
    fn to_config(&self) -> HashMap<String, toml::Value>;

    /// Load state from configuration data
    fn from_config(&mut self, data: &HashMap<String, toml::Value>);

    /// Get default configuration values for this section
    fn default_config(&self) -> HashMap<String, toml::Value> {
        HashMap::new()
    }
}
```

### Registering Configurable Components

Components are registered by emitting a `RegisterConfigurable` event during the `subscribe()` phase:

```rust
use reovim_core::{
    config::Configurable,
    event_bus::core_events::RegisterConfigurable,
};
use std::{collections::HashMap, sync::{Arc, RwLock}};

// Define a configurable component
struct MyPluginConfig {
    enabled: bool,
    timeout_ms: u64,
    theme: String,
}

impl Configurable for MyPluginConfig {
    fn config_section(&self) -> &'static str {
        "my_plugin"
    }

    fn to_config(&self) -> HashMap<String, toml::Value> {
        let mut data = HashMap::new();
        data.insert("enabled".to_string(), toml::Value::Boolean(self.enabled));
        data.insert("timeout_ms".to_string(), toml::Value::Integer(self.timeout_ms as i64));
        data.insert("theme".to_string(), toml::Value::String(self.theme.clone()));
        data
    }

    fn from_config(&mut self, data: &HashMap<String, toml::Value>) {
        if let Some(enabled) = data.get("enabled").and_then(toml::Value::as_bool) {
            self.enabled = enabled;
        }
        if let Some(timeout) = data.get("timeout_ms").and_then(toml::Value::as_integer) {
            self.timeout_ms = timeout as u64;
        }
        if let Some(theme) = data.get("theme").and_then(toml::Value::as_str) {
            self.theme = theme.to_string();
        }
    }

    fn default_config(&self) -> HashMap<String, toml::Value> {
        let mut data = HashMap::new();
        data.insert("enabled".to_string(), toml::Value::Boolean(true));
        data.insert("timeout_ms".to_string(), toml::Value::Integer(1000));
        data.insert("theme".to_string(), toml::Value::String("default".to_string()));
        data
    }
}

// In plugin subscribe()
impl Plugin for MyPlugin {
    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Create configurable component wrapped in Arc<RwLock<>>
        let config: Arc<RwLock<dyn Configurable + Send + Sync>> =
            Arc::new(RwLock::new(MyPluginConfig {
                enabled: true,
                timeout_ms: 1000,
                theme: "default".to_string(),
            }));

        // Register with profile system
        bus.emit(RegisterConfigurable::new(config));
    }
}
```

### Profile Events

The profile system uses three events:

```rust
// List all available profiles (opens picker)
#[derive(Debug, Clone, Copy)]
pub struct ProfileListEvent;

// Load a specific profile
#[derive(Debug, Clone)]
pub struct ProfileLoadEvent {
    pub name: String,
}

// Save current settings to a profile
#[derive(Debug, Clone)]
pub struct ProfileSaveEvent {
    pub name: String,
}
```

Plugins can emit these events to trigger profile operations:

```rust
// User action triggers profile load
bus.emit(ProfileLoadEvent { name: "dark-theme".to_string() });

// User action triggers profile save
bus.emit(ProfileSaveEvent { name: "my-custom-setup".to_string() });
```

### How Profiles Work

When a profile is saved:
1. Runtime calls `profile_registry.save_all()`
2. Registry iterates all registered `Configurable` components
3. Each component's `to_config()` is called
4. Data is serialized to TOML under the component's section name
5. TOML file is written to `~/.config/reovim/profiles/<name>.toml`

When a profile is loaded:
1. TOML file is read from `~/.config/reovim/profiles/<name>.toml`
2. Runtime calls `profile_registry.load_all(config)`
3. Registry iterates the config data
4. For each section, finds the corresponding component
5. Calls component's `from_config(data)` to restore state

### Complete Example

```rust
use std::{collections::HashMap, sync::{Arc, RwLock}};
use reovim_core::{
    config::Configurable,
    event_bus::{
        core_events::{RegisterConfigurable, ProfileLoadEvent, ProfileSaveEvent},
        EventBus, EventResult,
    },
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

// Plugin configuration state
struct MyPluginState {
    enabled: bool,
    font_size: u32,
}

impl Configurable for MyPluginState {
    fn config_section(&self) -> &'static str {
        "my_plugin"
    }

    fn to_config(&self) -> HashMap<String, toml::Value> {
        let mut data = HashMap::new();
        data.insert("enabled".to_string(), toml::Value::Boolean(self.enabled));
        data.insert("font_size".to_string(), toml::Value::Integer(self.font_size as i64));
        data
    }

    fn from_config(&mut self, data: &HashMap<String, toml::Value>) {
        if let Some(enabled) = data.get("enabled").and_then(toml::Value::as_bool) {
            self.enabled = enabled;
            tracing::info!("My plugin enabled state loaded: {}", enabled);
        }
        if let Some(size) = data.get("font_size").and_then(toml::Value::as_integer) {
            self.font_size = size as u32;
            tracing::info!("My plugin font size loaded: {}", size);
        }
    }
}

pub struct MyPlugin {
    state: Arc<RwLock<MyPluginState>>,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MyPluginState {
                enabled: true,
                font_size: 14,
            })),
        }
    }
}

impl Plugin for MyPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("my_plugin")
    }

    fn build(&self, _ctx: &mut PluginContext) {}

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register configurable component
        let config: Arc<RwLock<dyn Configurable + Send + Sync>> = self.state.clone();
        bus.emit(RegisterConfigurable::new(config));

        // Optionally listen to profile events for custom behavior
        let state = Arc::clone(&self.state);
        bus.subscribe::<ProfileLoadEvent, _>(100, move |event, ctx| {
            tracing::info!("Profile '{}' loaded, my plugin state updated", event.name);
            // State is automatically updated by ProfileRegistry
            // You can perform additional actions here if needed
            ctx.request_render();
            EventResult::Handled
        });
    }
}
```

### Profile File Structure

Profile TOML files are organized by section:

```toml
# ~/.config/reovim/profiles/dark-theme.toml

[profile]
name = "dark-theme"
description = "Dark theme with custom settings"

[core]
theme = "tokyonight"
colormode = "truecolor"
indentguide = true

[my_plugin]
enabled = true
font_size = 14

[treesitter]
enabled = true
timeout_ms = 500
```

Each plugin's `config_section()` maps to a TOML section. The `ProfileRegistry` coordinates saving/loading across all sections.

### Best Practices

1. **Use descriptive section names** - Match your plugin ID for consistency
2. **Handle missing values gracefully** - Don't crash if a setting is missing from the profile
3. **Provide defaults** - Implement `default_config()` for new profile creation
4. **Use appropriate TOML types** - Boolean, Integer, String, Array, Table
5. **Keep state in Arc<RwLock<>>** - Required for thread-safe access from profile system
6. **Log configuration changes** - Help users debug profile loading issues
7. **Don't store sensitive data** - Profiles are plain text files

### Thread Safety

The `Configurable` component must be wrapped in `Arc<RwLock<>>` because:
- The profile registry stores `Arc<RwLock<dyn Configurable + Send + Sync>>`
- Multiple systems may need to access configuration simultaneously
- `from_config()` requires `&mut self` for updates
- `to_config()` requires `&self` for reads

```rust
// CORRECT
let config: Arc<RwLock<dyn Configurable + Send + Sync>> =
    Arc::new(RwLock::new(MyPluginState { ... }));
bus.emit(RegisterConfigurable::new(config));

// INCORRECT (won't compile)
let config = MyPluginState { ... };
bus.emit(RegisterConfigurable::new(config)); // Type mismatch
```

## PluginStateRegistry

Type-erased state storage accessible by any component:

```rust
pub struct PluginStateRegistry {
    states: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl PluginStateRegistry {
    /// Register state of type T
    pub fn register<T: Send + Sync + 'static>(&self, state: T);

    /// Get a clone of state (requires Clone)
    pub fn get<T: Clone + Send + Sync + 'static>(&self) -> Option<T>;

    /// Mutate state in place
    pub fn with_mut<T, R, F>(&self, f: F) -> Option<R>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&mut T) -> R;

    /// Check if state exists
    pub fn contains<T: 'static>(&self) -> bool;
}
```

**Example:**
```rust
// In init_state()
registry.register(LeapState::new());

// Later, access state
registry.with_mut::<LeapState, _, _>(|state| {
    state.start(direction, operator, count);
});
```

## Event Bus

Type-erased event system for plugin communication:

### Event Trait

```rust
pub trait Event: Send + Sync + 'static {
    /// Event priority (lower = higher priority)
    fn priority(&self) -> u32 { 100 }
}
```

### EventBus

```rust
pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<EventHandler>>>,
}

impl EventBus {
    /// Subscribe to events of type E
    pub fn subscribe<E: Event, F>(&self, priority: u32, handler: F)
    where
        F: Fn(&E, &EventContext) -> EventResult + Send + Sync + 'static;

    /// Emit an event to all subscribers
    pub fn emit<E: Event>(&self, event: E);
}
```

### EventResult

```rust
pub enum EventResult {
    Handled,     // Event handled, stop propagation
    Continue,    // Event handled, continue propagation
    NotHandled,  // Event not handled
}
```

## Built-in Plugins (in lib/core)

Built-in plugins ship with `reovim-core` and are included in `DefaultPlugins`.

### CorePlugin

Essential editor functionality (cursor, mode, text operations):

```rust
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn id(&self) -> PluginId { PluginId::new("reovim:core") }
    fn name(&self) -> &'static str { "Core" }

    fn build(&self, ctx: &mut PluginContext) {
        // Register cursor commands (unified command-event types)
        ctx.register_command(CursorLeft);
        ctx.register_command(CursorDown);
        // ... more commands

        // Register keybindings
        ctx.register_keybinding("normal", "h", CURSOR_LEFT);
        ctx.register_keybinding("normal", "j", CURSOR_DOWN);
        // ... more keybindings
    }
}
```

### LeapPlugin

Two-character jump navigation (s/S):

```rust
pub struct LeapPlugin;

impl Plugin for LeapPlugin {
    fn id(&self) -> PluginId { PluginId::new("reovim:leap") }
    fn name(&self) -> &'static str { "Leap" }
    fn dependencies(&self) -> Vec<TypeId> { vec![TypeId::of::<CorePlugin>()] }

    fn build(&self, ctx: &mut PluginContext) {
        // Register unified command-event types
        ctx.register_command(LeapForward);
        ctx.register_command(LeapBackward);
        ctx.register_command(LeapCancel);
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        registry.register(LeapState::new());
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let state_clone = Arc::clone(&state);
        // Subscribe using the same unified type (no "Event" suffix)
        bus.subscribe::<LeapStart, _>(100, move |event, _ctx| {
            state_clone.with_mut::<LeapState, _, _>(|leap_state| {
                leap_state.start(event.direction, event.operator, event.count);
            });
            EventResult::Handled
        });
    }
}
```

### WindowPlugin

Window management (splits, navigation):
- Split commands (horizontal/vertical)
- Window navigation (h/j/k/l between windows)
- Window resize

## External Feature Plugins (in plugins/features/)

External plugins are separate crates that depend on `reovim-core`. They register commands from core's command modules.

### FoldPlugin (`reovim-plugin-fold`)

Code folding with treesitter integration:
- Toggle fold (`za`)
- Open/close fold (`zo`/`zc`)
- Open/close all (`zR`/`zM`)

### SettingsMenuPlugin (`reovim-plugin-settings-menu`)

In-editor settings configuration with extensible settings registration:
- 19 commands for navigation and editing
- Live preview of setting changes
- Profile management
- **Dynamic section/option registration** via events (see below)

### CompletionPlugin (`reovim-plugin-completion`)

Text completion with popup menu and background processing (treesitter-like pattern):

**Features:**
- Popup menu with completion suggestions
- Ghost text preview (remaining text shown inline in dim grey)
- Background saturator for non-blocking completion
- Lock-free cache for responsive UI

**Keybindings:**
- `Alt-Space` (insert mode) - Trigger completion
- `Ctrl-n`/`Ctrl-p` - Navigate suggestions
- `Tab` - Confirm selection
- `Escape` - Dismiss popup

**Plugin Structure:**
```
plugins/features/completion/src/
├── lib.rs          # CompletionPlugin only (clean)
├── state.rs        # SharedCompletionManager (RwLock wrapper)
├── window.rs       # CompletionPluginWindow
├── cache.rs        # CompletionCache (ArcSwap, lock-free)
├── saturator.rs    # Background completion task
├── registry.rs     # SourceRegistry, SourceSupport trait
├── events.rs       # RegisterSource, CompletionReady events
├── commands.rs     # Unified command-event types
└── source/
    ├── mod.rs
    └── buffer.rs   # BufferWordsSource
```

**Architecture (follows treesitter pattern):**
- `CompletionPlugin` - Main plugin, registers commands and keybindings
- `SharedCompletionManager` - Thread-safe wrapper for cross-plugin access
- `CompletionPluginWindow` - PluginWindow for popup rendering
- `CompletionSaturator` - Background task for non-blocking completion
- `CompletionCache` - ArcSwap cache for lock-free render access
- `SourceRegistry` - Dynamic source registration
- `SourceSupport` trait - Interface for completion sources

**Source Registration:**
```rust
// External plugins register sources via events
bus.emit(RegisterSource {
    source: Arc::new(MyCompletionSource::new()),
});

// Completion plugin receives and registers
bus.subscribe::<RegisterSource, _>(100, move |event, ctx| {
    manager.register_source(Arc::clone(&event.source));
    EventResult::Handled
});
```

**SourceSupport Trait:**
```rust
pub trait SourceSupport: Send + Sync + 'static {
    fn source_id(&self) -> &'static str;
    fn priority(&self) -> u32 { 100 }
    fn complete<'a>(
        &'a self,
        ctx: &'a CompletionContext,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>>;
}
```

**Built-in Sources:**
- `BufferWordsSource` - Completes words from current buffer

### ExplorerPlugin (`reovim-plugin-explorer`)

File browser sidebar:
- Toggle (`Space e`)
- ~30 commands for navigation, tree ops, file ops
- Clipboard operations (copy/cut/paste)
- File details popup (`s`) - shows name, path, type, size, created/modified dates
- Copy path to clipboard (`t` in popup, or `y` in visual selection)

**File Details Popup:**

Press `s` on any file/directory to show a centered popup with details:
```
+-- File Details ---------------------------------+
|     Name: example.rs                           |
|     Path: /home/user/proj/src/example.rs       |
|     Type: file                                 |
|     Size: 12.26 KB                             |
|  Created: 2025-12-22 04:44 PM                  |
| Modified: 2025-12-22 04:44 PM                  |
+-- <T>: copy path / <Esc/Enter> Close ----------+
```

The popup syncs with cursor movement - navigate with `j`/`k` while popup is open to view details of different files. Press `t` to copy the path to both system clipboard and editor registers (pasteable with `p`).

### TelescopePlugin (`reovim-plugin-telescope`)

Fuzzy finder:
- Files picker (`Space ff`)
- Buffers picker (`Space fb`)
- Live grep (`Space fg`)
- 9 built-in pickers
- 19 navigation/action commands

### TreesitterPlugin (`reovim-plugin-treesitter`)

Syntax highlighting infrastructure using buffer-centric architecture (Helix-inspired):

**Plugin Components:**
- `TreesitterPlugin` - Main plugin, registers SyntaxFactory
- `TreesitterSyntaxFactory` - Creates TreeSitterSyntax for buffers
- `TreeSitterSyntax` - Per-buffer syntax provider (implements `SyntaxProvider`)
- `SharedTreesitterManager` - Shared state for language registry and queries
- `LanguageRegistry` - Dynamic language registration
- `BufferParser` - Per-buffer incremental parser
- `Highlighter` - Query execution and highlight generation
- `TextObjectResolver` - Semantic text object bounds

**Architecture:**
```
Buffer -> syntax: Option<Box<dyn SyntaxProvider>>
                              │
                              ▼
                    TreeSitterSyntax (plugin)
                    ├── parser: Parser
                    ├── tree: Option<Tree>
                    ├── query: Arc<Query>  (pre-compiled)
                    └── highlighter: Highlighter
```

**SyntaxFactory Registration:**
```rust
fn init_state(&self, registry: &PluginStateRegistry) {
    let manager = Arc::new(SharedTreesitterManager::new());
    registry.register(Arc::clone(&manager));

    // Register factory for runtime to use
    let factory = TreesitterSyntaxFactory::new(Arc::clone(&manager));
    registry.set_syntax_factory(Arc::new(factory));
}
```

**LanguageSupport Trait:**
```rust
pub trait LanguageSupport: Send + Sync + 'static {
    fn language_id(&self) -> &'static str;
    fn file_extensions(&self) -> &'static [&'static str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    fn highlights_query(&self) -> &'static str;
    fn folds_query(&self) -> Option<&'static str> { None }
    fn textobjects_query(&self) -> Option<&'static str> { None }
    fn decorations_query(&self) -> Option<&'static str> { None }
}
```

**Data Flow:**
1. **File Open**: Runtime calls `syntax_factory.create_syntax(path, content)`
2. **Attach**: Runtime calls `buffer.attach_syntax(syntax)`
3. **Parse**: `syntax.parse(content)` builds initial tree
4. **Render**: `RenderData::from_buffer()` calls `syntax.highlight_range()`
5. **Display**: Highlights applied to framebuffer during render

## Language Plugins (in plugins/languages/)

Language plugins provide language-specific support:

| Plugin | Crate | Extensions |
|--------|-------|------------|
| Rust | `reovim-lang-rust` | `.rs` |
| C | `reovim-lang-c` | `.c`, `.h` |
| JavaScript | `reovim-lang-javascript` | `.js`, `.jsx` |
| Python | `reovim-lang-python` | `.py` |
| JSON | `reovim-lang-json` | `.json` |
| TOML | `reovim-lang-toml` | `.toml` |
| Markdown | `reovim-lang-markdown` | `.md` |

### StatuslinePlugin (`reovim-plugin-statusline`)

Section-based API for statusline extensions. Other plugins can register dynamic sections.

**Plugin Components:**
- `StatuslinePlugin` - Main plugin, registers provider with core
- `StatuslineSection` - Section definition with id, priority, alignment, render callback
- `SharedStatuslineManager` - Thread-safe section registry implementing `StatuslineSectionProvider`
- Events: `StatuslineSectionRegister`, `StatuslineSectionUnregister`, `StatuslineRefresh`

**Core API (Generic Trait):**
```rust
// In lib/core/src/plugin/statusline.rs
pub trait StatuslineSectionProvider: Send + Sync + 'static {
    fn render_sections(&self, ctx: &StatuslineRenderContext) -> Vec<RenderedSection>;
}

pub struct RenderedSection {
    pub text: String,
    pub style: Option<Style>,
    pub alignment: SectionAlignment,  // Left, Center, Right
    pub priority: u32,
}
```

**Usage (Other Plugins Registering Sections):**
```rust
// In another plugin's subscribe()
bus.emit(StatuslineSectionRegister {
    section: StatuslineSection {
        id: "lsp_status",
        priority: 100,
        alignment: SectionAlignment::Right,
        render: Arc::new(|_ctx| SectionContent {
            text: " LSP ".into(),
            style: None,
            visible: true,
        }),
    },
});
```

### NotificationPlugin (`reovim-plugin-notification`)

Toast notifications and progress bars for non-blocking user feedback.

**Plugin Components:**
- `NotificationPlugin` - Main plugin, registers commands and window
- `Notification` - Toast notification with level (Info, Success, Warning, Error)
- `ProgressNotification` - Progress bar with percentage or indeterminate spinner
- `SharedNotificationManager` - Thread-safe notification state
- `NotificationPluginWindow` - PluginWindow implementation (z-order 500)
- `NotificationStyles` - Plugin-local styles (not in core Theme)

**Events:**
```rust
// Show a notification
bus.emit(NotificationShow {
    level: NotificationLevel::Success,
    message: "File saved".into(),
    duration_ms: Some(2000),
    source: None,
});

// Show progress
bus.emit(ProgressUpdate {
    id: "build".into(),
    title: "Building".into(),
    source: "cargo".into(),
    progress: Some(45),  // 0-100 or None for spinner
    detail: Some("4/250 (core)".into()),
});

// Complete progress
bus.emit(ProgressComplete {
    id: "build".into(),
    message: Some("Build complete".into()),
});
```

**Position Options:**
- `TopRight` (default), `TopLeft`, `BottomRight`, `BottomLeft`, `TopCenter`, `BottomCenter`

## Unified Command-Event Pattern (Recommended)

**Modern approach:** Use a single type that serves as both command and event.

### Using Declaration Macros

For most plugin commands, use the `declare_event_command!` macro:

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
bus.subscribe::<ExplorerRefresh, _>(100, |event, ctx| {
    // Handle refresh
    EventResult::Handled
});
```

**Benefits:**
- 50% fewer types (one instead of two)
- ~20 lines of boilerplate eliminated per command
- Clearer intent - action is both trigger and notification
- Zero-cost abstraction (zero-sized type)

### For Commands with Count

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

### For Commands with Custom Data

For commands that need custom data fields, implement manually:

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

// Still register and subscribe with same type
ctx.register_command(ExplorerInputChar::new('a'));
bus.subscribe::<ExplorerInputChar, _>(100, |event, ctx| {
    // Access event.c
    EventResult::Handled
});
```

## Legacy: Separate Command/Event Types (Deprecated)

**Old approach:** Create separate Command and Event types (no longer recommended).

```rust
// DON'T DO THIS - deprecated pattern

// Separate event type
#[derive(Debug, Clone)]
pub struct LeapStartEvent {
    pub direction: LeapDirection,
    pub operator: Option<OperatorType>,
    pub count: Option<usize>,
}

impl Event for LeapStartEvent {
    fn priority(&self) -> u32 { 50 }
}

// Separate command type
pub struct LeapStartCommand;

impl CommandTrait for LeapStartCommand {
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(LeapStartEvent { ... }))
    }
    // ... 15 more lines of boilerplate
}
```

**Migration:** Use unified types instead (see above).

## Plugin Loading

### PluginTuple Pattern

Plugins are loaded via `PluginLoader` using the `PluginTuple` trait:

```rust
// In lib/core - DefaultPlugins (built-in only)
pub struct DefaultPlugins;

impl PluginTuple for DefaultPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(CorePlugin);
        loader.add(WindowPlugin);
    }
}
```

```rust
// In runner/src/plugins.rs - AllPlugins (built-in + external)
pub struct AllPlugins;

impl PluginTuple for AllPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        // Built-in plugins
        loader.add_plugins(DefaultPlugins);

        // External feature plugins
        loader.add(FoldPlugin);
        loader.add(SettingsMenuPlugin);
        loader.add(CompletionPlugin);
        loader.add(ExplorerPlugin);
        loader.add(TelescopePlugin);
        loader.add(TreesitterPlugin);

        // Language plugins
        loader.add(RustPlugin);
        loader.add(CPlugin);
        loader.add(JavaScriptPlugin);
        loader.add(PythonPlugin);
        loader.add(JsonPlugin);
        loader.add(TomlPlugin);
        loader.add(MarkdownPlugin);
    }
}
```

### Runtime Initialization

```rust
// In runner/src/main.rs
use plugins::AllPlugins;

let runtime = Runtime::with_plugins(screen, AllPlugins)
    .with_file(cli.file)
    .with_profile(cli.profile);

runtime.init().await;
```

### Loader Phases

The loader processes plugins in dependency order:
1. Resolves dependencies (topological sort)
2. Calls `build()` on all plugins in order
3. Calls `init_state()` on all plugins
4. Calls `finish()` on all plugins
5. Calls `subscribe()` on all plugins

## Integration with Runtime

The runtime owns the event bus and plugin state registry:

```rust
pub struct Runtime {
    // ... other fields

    /// Type-erased event bus
    pub event_bus: Arc<EventBus>,

    /// Plugin state storage
    pub plugin_state: Arc<PluginStateRegistry>,
}
```

Plugins can emit events that the runtime handles:

```rust
// In event_loop.rs
fn handle_leap_event(&mut self, event: LeapEvent) {
    match event {
        LeapEvent::Start { direction, operator, count } => {
            // Update local state
            self.leap_state.start(direction, operator, count);

            // Emit to event bus for plugin consumption
            self.event_bus.emit(LeapStartEvent {
                direction,
                operator,
                count,
            });
        }
        // ...
    }
}
```

## Migration Guide

Features are being migrated from hardcoded `InnerEvent` variants to the event bus:

### Before (InnerEvent)
```rust
// In event/inner/mod.rs - requires modifying core enum
pub enum InnerEvent {
    LeapEvent(LeapEvent),
    // ... many variants
}
```

### After (Event Bus)
```rust
// In plugin - no core changes needed
#[derive(Debug, Clone)]
pub struct LeapStartEvent { ... }
impl Event for LeapStartEvent {}

// Subscribe in plugin
bus.subscribe::<LeapStartEvent, _>(100, |event, _ctx| {
    // Handle event
    EventResult::Handled
});
```

## Plugin Self-Registration

Plugins are fully self-contained - core has no knowledge of specific plugins. Plugins define and register their own:

### ComponentId

Each plugin defines its own `ComponentId` constant:

```rust
// In plugins/features/explorer/src/lib.rs
use reovim_core::modd::ComponentId;

pub const COMPONENT_ID: ComponentId = ComponentId("explorer");
```

Core only defines essential IDs: `EDITOR`, `COMMAND_LINE`, `STATUS_LINE`, `TAB_LINE`.

### Display Info

Plugins register their display strings and icons via `PluginContext`:

```rust
use reovim_core::display::{DisplayInfo, DisplayRegistry};

fn build(&self, ctx: &mut PluginContext) {
    // Register display info for this component
    ctx.register_display_info(COMPONENT_ID, DisplayInfo::new(" EXPLORER ", "󰙅 "));

    // Or for specific edit modes within this component
    ctx.register_mode_display(COMPONENT_ID, EditModeKey::Normal, DisplayInfo::new(" NORMAL ", "N"));
}
```

The `DisplayRegistry` provides fallback for unregistered components.

### Z-Order

Plugin windows control their z-order via the `PluginWindow::z_order()` method. Core only defines `z_order::BASE` (0) and `z_order::EDITOR` (2). Typical plugin z-orders:
- Leap: 100
- Completion: 200
- Telescope: 300
- Settings: 400

### Keybindings

Plugins register their own keybindings during `build()`:

```rust
fn build(&self, ctx: &mut PluginContext) {
    // Register unified command-event types (no "Command" suffix)
    ctx.register_command(ExplorerToggle);
    ctx.register_command(ExplorerCursorUp::new(1));

    // Register keybindings for specific scopes
    ctx.register_keybinding("normal", "Space e", EXPLORER_TOGGLE);
    ctx.register_keybinding("explorer", "k", EXPLORER_CURSOR_UP);
    ctx.register_keybinding("explorer", "j", EXPLORER_CURSOR_DOWN);
}
```

### Plugin Input Handling

Plugin input is handled via event subscriptions. When a plugin component has focus, the runtime emits `PluginTextInput` and `PluginBackspace` events that plugins can subscribe to:

```rust
use reovim_core::event_bus::core_events::{PluginTextInput, PluginBackspace};

fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
    let state_clone = Arc::clone(&state);
    bus.subscribe::<PluginTextInput, _>(100, move |event, ctx| {
        if event.target != COMPONENT_ID {
            return EventResult::NotHandled;
        }
        state_clone.with_mut::<ExplorerState, _, _>(|s| {
            s.input_char(event.c);
        });
        ctx.request_render();
        EventResult::Handled
    });

    let state_clone = Arc::clone(&state);
    bus.subscribe::<PluginBackspace, _>(100, move |event, ctx| {
        if event.target != COMPONENT_ID {
            return EventResult::NotHandled;
        }
        state_clone.with_mut::<ExplorerState, _, _>(|s| {
            s.input_backspace();
        });
        ctx.request_render();
        EventResult::Handled
    });
}
```

**Built-in vs Plugin Components:**

- **Built-in components** (Editor, CommandLine): Handled via fast path in Runtime with direct access to Runtime state (buffers, command_line)
- **Plugin components** (Explorer, Telescope): Receive input via `PluginTextInput` and `PluginBackspace` events

This separation ensures built-in components can execute synchronously with full Runtime access while plugins maintain proper encapsulation through the event bus and state registry.

## Rendering

Reovim uses the `PluginWindow` trait for plugin UI rendering:

- **PluginWindow**: Plugin panels and windows (explorer, telescope, settings)
  - Implement `PluginWindow` trait for visibility, bounds, and rendering
  - See [Plugin Rendering Guide](./plugin-rendering.md) for details

For built-in components (status line, tab line), rendering is handled directly by dedicated component modules.

## State Management Patterns

Reovim supports two state management patterns:

### Registry-Owned State (Simple)

For straightforward plugin state:

```rust
fn init_state(&self, registry: &PluginStateRegistry) {
    registry.register(MyState::new());
}
```

**Characteristics**:
- State owned by PluginStateRegistry
- Accessed via `state.with::<MyState, _, _>()`
- Simple, no sharing needed

### Plugin-Owned State (Shared)

For state that implements multiple traits or is shared:

```rust
struct MyPlugin {
    state: Arc<SharedState>,
}

fn init_state(&self, registry: &PluginStateRegistry) {
    registry.register(Arc::clone(&self.state));
    registry.set_visibility_source(
        Arc::clone(&self.state) as Arc<dyn BufferVisibilitySource>
    );
}
```

**Use when**:
- State implements BufferVisibilitySource, WindowProvider, etc.
- State is shared with other systems
- Need multiple trait registrations

## Creating External Plugins

### Crate Structure

External plugins follow this structure:

```
plugins/features/{name}/
├── Cargo.toml
└── src/
    └── lib.rs
```

### Cargo.toml Template

```toml
[package]
name = "reovim-plugin-{name}"
version.workspace = true
edition.workspace = true

[dependencies]
reovim-core = { path = "../../../lib/core" }
```

### Plugin Implementation

External plugins typically register commands from core:

```rust
use std::any::TypeId;
use reovim_core::plugin::{Plugin, PluginContext, PluginId};
use reovim_core::command::builtin::{SomeCommand, AnotherCommand};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:my-plugin")
    }

    fn name(&self) -> &'static str { "My Plugin" }
    fn description(&self) -> &'static str { "Description of my plugin" }
    fn dependencies(&self) -> Vec<TypeId> { vec![] }

    fn build(&self, ctx: &mut PluginContext) {
        let _ = ctx.register_command(SomeCommand);
        let _ = ctx.register_command(AnotherCommand);
    }
}

// Re-export types for external use
pub use reovim_core::some_module::{SomeState, SomeType};
```

### Registration

Add the plugin to `runner/src/plugins.rs`:

```rust
use reovim_plugin_my_plugin::MyPlugin;

impl PluginTuple for AllPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        // ...existing plugins...
        loader.add(MyPlugin);
    }
}
```

And add the dependency to `runner/Cargo.toml`:

```toml
[dependencies]
reovim-plugin-my-plugin.workspace = true
```

## Plugin Architecture Summary

```
┌──────────────────────────────────────────────────────────────┐
│                         Runner                               │
│  ┌──────────────────────────────────────────────────────────┐│
│  │                      AllPlugins                          ││
│  │  ┌─────────────────┐  ┌─────────────────────────────────┐││
│  │  │  DefaultPlugins │  │    External Plugins             │││
│  │  │  (from core)    │  │    (separate crates)            │││
│  │  │                 │  │                                 │││
│  │  │  • CorePlugin   │  │  • FoldPlugin                   │││
│  │  │  • WindowPlugin │  │  • SettingsMenuPlugin           │││
│  │  │                 │  │  • CompletionPlugin             │││
│  │  │                 │  │  • ExplorerPlugin               │││
│  │  │                 │  │  • TelescopePlugin              │││
│  │  │                 │  │  • LeapPlugin                   │││
│  │  │                 │  │  • TreesitterPlugin             │││
│  │  │                 │  │  • Language plugins...          │││
│  │  └─────────────────┘  └─────────────────────────────────┘││
│  └──────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       reovim-core                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Plugin    │  │   Event     │  │    Command          │  │
│  │   System    │  │   Bus       │  │    Registry         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Feature Modules                      ││
│  │  completion/ explorer/ telescope/ settings_menu/ fold/  ││
│  │         (types and commands for external plugins)       ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Plugin Decoupling Principles

Reovim follows strict plugin decoupling. Core must never contain plugin-specific code.

### Core Must Not Know About Plugins

Core provides general-purpose infrastructure:
- Event bus for type-erased events
- PluginStateRegistry for type-erased state
- PluginWindow trait for UI rendering
- Generic compositor IDs (`ComposableId::Custom("name")`)

Core must NOT contain:
- Plugin-specific event types
- Plugin-specific enum variants
- Plugin-specific feature flags
- Plugin-specific keybinding handlers

### Proposing API Extensions

When the current API is insufficient:

1. **Identify the gap** - What can't your plugin do?
2. **Design a general solution** - Would other plugins benefit?
3. **Create a proposal** - Document in `tmp/<name>-api-proposal.md`
4. **Discuss** - Get feedback before implementation
5. **Implement** - Add to core only if it's truly general-purpose

### Example: Which-Key Decoupling

**Before (wrong):**
- `lib/core/src/which_key.rs` - Event in core
- `ComposableId::WhichKey` - Plugin-specific enum variant
- Hard-coded `?` key dispatch in command handler

**After (correct):**
- Event defined in `plugins/features/which-key/src/commands.rs`
- Uses `ComposableId::Custom("which_key")`
- Plugin registers `?` keybinding via `build()`

## Related Documentation

- [Architecture](./architecture.md) - System design overview
- [Event System](./event-system.md) - Event flow details
- [Commands](./commands.md) - Command system
