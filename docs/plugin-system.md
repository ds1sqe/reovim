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

In-editor settings configuration:
- 19 commands for navigation and editing
- Live preview of setting changes
- Profile management

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
