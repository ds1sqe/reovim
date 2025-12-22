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
│   ├── fold/                   # Code folding
│   ├── settings-menu/          # In-editor settings
│   ├── completion/             # Text completion
│   ├── explorer/               # File browser
│   ├── telescope/              # Fuzzy finder
│   └── treesitter/             # Syntax highlighting infrastructure
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
    component_registry: ComponentRegistry,
    plugin_state: Arc<PluginStateRegistry>,
}

impl PluginContext {
    /// Register a command
    pub fn register_command<C: CommandTrait + 'static>(&mut self, cmd: C) -> Result<(), PluginError>;

    /// Register a keybinding
    pub fn register_keybinding(&mut self, mode: &str, keys: &str, command_id: CommandId);

    /// Register a UI component for input handling and rendering
    pub fn register_component(&mut self, component: Box<dyn UIComponent>);
}
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
        // Register cursor commands
        ctx.register_command(CursorLeftCommand);
        ctx.register_command(CursorDownCommand);
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
        ctx.register_command(LeapForwardCommand);
        ctx.register_command(LeapBackwardCommand);
        ctx.register_command(LeapCancelCommand);
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        registry.register(LeapState::new());
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let state_clone = Arc::clone(&state);
        bus.subscribe::<LeapStartEvent, _>(100, move |event, _ctx| {
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

In-editor settings configuration:
- 19 commands for navigation and editing
- Live preview of setting changes
- Profile management

### CompletionPlugin (`reovim-plugin-completion`)

Text completion with popup menu:
- Trigger (`Ctrl-Space`)
- Navigate (`Ctrl-n`/`Ctrl-p`)
- Confirm (`Tab`)
- Dismiss (`Esc`)

### ExplorerPlugin (`reovim-plugin-explorer`)

File browser sidebar:
- Toggle (`Space e`)
- ~30 commands for navigation, tree ops, file ops
- Clipboard operations (copy/cut/paste)

### TelescopePlugin (`reovim-plugin-telescope`)

Fuzzy finder:
- Files picker (`Space ff`)
- Buffers picker (`Space fb`)
- Live grep (`Space fg`)
- 9 built-in pickers
- 19 navigation/action commands

### TreesitterPlugin (`reovim-plugin-treesitter`)

Syntax highlighting infrastructure:
- `TreesitterPlugin` - Main plugin, handles events and lifecycle
- `SharedTreesitterManager` - Shared state for all buffers
- `LanguageRegistry` - Dynamic language registration
- `BufferParser` - Per-buffer incremental parser
- `Highlighter` - Query execution and highlight generation
- `TextObjectResolver` - Semantic text object bounds

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

## Defining Events

Create events by implementing the `Event` trait:

```rust
use crate::event_bus::Event;

#[derive(Debug, Clone)]
pub struct LeapStartEvent {
    pub direction: LeapDirection,
    pub operator: Option<OperatorType>,
    pub count: Option<usize>,
}

impl Event for LeapStartEvent {
    fn priority(&self) -> u32 { 50 }  // High priority for mode changes
}
```

## Plugin Loading

### PluginTuple Pattern

Plugins are loaded via `PluginLoader` using the `PluginTuple` trait:

```rust
// In lib/core - DefaultPlugins (built-in only)
pub struct DefaultPlugins;

impl PluginTuple for DefaultPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(CorePlugin);
        loader.add(UIComponentsPlugin);
        loader.add(LeapPlugin);
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
use reovim_core::ui_component::ComponentId;

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

### Z-Order (Overlay Rendering)

Plugins define their z-order via `OverlayRenderer::z_order()`:

```rust
use reovim_core::overlay::OverlayRenderer;

impl OverlayRenderer for TelescopeOverlay {
    fn z_order(&self) -> u16 { 300 }  // Plugin controls its own z-order
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme) { ... }
}
```

Core only defines `z_order::BASE` (0) and `z_order::EDITOR` (2). Typical plugin z-orders:
- Leap: 100
- Completion: 200
- Telescope: 300
- Settings: 400

### Keybindings

Plugins register their own keybindings during `build()`:

```rust
fn build(&self, ctx: &mut PluginContext) {
    // Register commands
    ctx.register_command(ExplorerToggleCommand);
    ctx.register_command(ExplorerUpCommand);

    // Register keybindings for specific scopes
    ctx.register_keybinding("normal", "Space e", EXPLORER_TOGGLE);
    ctx.register_keybinding("explorer", "k", EXPLORER_UP);
    ctx.register_keybinding("explorer", "j", EXPLORER_DOWN);
}
```

### UIComponent

Plugins implement and register `UIComponent` for input handling:

```rust
use reovim_core::{
    ui_component::{ComponentId, UIComponent},
    interactor::InputResult,
    modd::ModeState,
    component::RenderContext,
    frame::FrameBuffer,
    screen::{z_order, LayerBounds},
    plugin::PluginStateRegistry,
};

#[derive(Debug)]
pub struct ExplorerComponent;

impl UIComponent for ExplorerComponent {
    fn id(&self) -> ComponentId {
        ComponentId("explorer")
    }

    fn display_name(&self) -> &'static str {
        "EXPLORER"
    }

    fn z_order(&self) -> u8 {
        z_order::BASE
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        true
    }

    fn bounds(&self, _ctx: &RenderContext<'_>) -> LayerBounds {
        LayerBounds { x: 0, y: 0, width: 0, height: 0 }
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering handled by WindowProvider
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(
        &mut self,
        c: char,
        _mode_state: &ModeState,
        state: &PluginStateRegistry,
    ) -> InputResult {
        // Plugin components access state directly via PluginStateRegistry
        state.with_mut::<ExplorerState, _, _>(|explorer| {
            if !explorer.input_buffer.is_empty() || explorer.message.is_some() {
                explorer.input_buffer.push(c);
                InputResult::Handled
            } else {
                InputResult::NotHandled
            }
        }).unwrap_or(InputResult::NotHandled)
    }

    fn handle_delete_backward(
        &mut self,
        _mode_state: &ModeState,
        state: &PluginStateRegistry,
    ) -> InputResult {
        // Plugin components access state directly via PluginStateRegistry
        state.with_mut::<ExplorerState, _, _>(|explorer| {
            if !explorer.input_buffer.is_empty() {
                explorer.input_buffer.pop();
                InputResult::Handled
            } else {
                InputResult::NotHandled
            }
        }).unwrap_or(InputResult::NotHandled)
    }

    fn captures_input(&self) -> bool {
        true
    }
}

fn build(&self, ctx: &mut PluginContext) {
    ctx.register_component(Box::new(ExplorerComponent));
}
```

**Input Handling Patterns:**

Plugin components have two options for handling input:

1. **Direct State Access (Recommended)**: Return `InputResult::Handled` and manipulate state via `PluginStateRegistry` directly
   - Best for plugin components that own their state
   - Synchronous, no event bounce
   - Example: Explorer input handling shown above

2. **Not Handled**: Return `InputResult::NotHandled` for input the component doesn't handle
   - Input will be ignored or handled by other systems

**Built-in vs Plugin Components:**

- **Built-in components** (Editor, CommandLine): Handled via fast path in Runtime with direct access to Runtime state (buffers, command_line)
- **Plugin components** (Explorer, Telescope): Handled via `UIComponent` trait methods with access to `PluginStateRegistry`

This separation ensures built-in components can execute synchronously with full Runtime access while plugins maintain proper encapsulation through the state registry.

## Rendering

Reovim provides three rendering systems for different UI patterns:

- **Overlays**: Temporary popups (completion, telescope)
  - See [Rendering Guide](./plugin-rendering.md#overlays---temporary-popups)

- **Window Providers**: Persistent panels (explorer, outline)
  - See [Rendering Guide](./plugin-rendering.md#window-providers---persistent-panels)

- **UIComponent**: Fixed UI elements (status line, tab line)
  - See [Rendering Guide](./plugin-rendering.md#uicomponent-rendering---fixed-ui-elements)

For detailed guidance on choosing the right rendering system, see the [Plugin Rendering Guide](./plugin-rendering.md).

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
│  │  │  • LeapPlugin   │  │  • SettingsMenuPlugin           │││
│  │  │  • WindowPlugin │  │  • CompletionPlugin             │││
│  │  │  • UIComponents │  │  • ExplorerPlugin               │││
│  │  │                 │  │  • TelescopePlugin              │││
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

## Related Documentation

- [Architecture](./architecture.md) - System design overview
- [Event System](./event-system.md) - Event flow details
- [Commands](./commands.md) - Command system
