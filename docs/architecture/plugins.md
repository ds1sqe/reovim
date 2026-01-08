# Plugin System

The plugin system enables modular features with full lifecycle management.

## Module Location

```
lib/core/src/plugin/
├── mod.rs              # Public exports
├── traits.rs           # Plugin trait definition
├── context.rs          # PluginContext for registration
├── loader.rs           # PluginLoader for dependency resolution
├── state.rs            # PluginStateRegistry
├── runtime_context.rs  # RuntimeContext for plugins
└── builtin/            # Built-in plugins (core)
```

## Plugin Trait

```rust
pub trait Plugin: Send + Sync + 'static {
    fn id(&self) -> PluginId;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn dependencies(&self) -> Vec<TypeId> { vec![] }

    // Lifecycle methods
    fn build(&self, ctx: &mut PluginContext);
    fn init_state(&self, registry: &PluginStateRegistry) {}
    fn finish(&self, ctx: &mut PluginContext) {}
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {}
}
```

## Plugin Lifecycle

1. `build()` - Register commands, keybindings
2. `init_state()` - Initialize plugin state in registry
3. `finish()` - Post-registration setup
4. `subscribe()` - Subscribe to events via event bus

## PluginStateRegistry

Type-erased state storage for plugins:

```rust
pub struct PluginStateRegistry {
    states: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl PluginStateRegistry {
    pub fn register<T: Send + Sync + 'static>(&self, state: T);
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<T>;
    pub fn with_mut<T, R, F>(&self, f: F) -> Option<R>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&mut T) -> R;
}
```

## Event Bus

Type-erased event system for plugin communication:

```rust
pub trait Event: Send + Sync + 'static {
    fn priority(&self) -> u32 { 100 }
}

pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<EventHandler>>>,
}

impl EventBus {
    pub fn subscribe<E: Event, F>(&self, priority: u32, handler: F)
    where
        F: Fn(&E, &EventContext) -> EventResult + Send + Sync + 'static;

    pub fn emit<E: Event>(&self, event: E);
}
```

## Example Plugin

```rust
impl Plugin for ExamplePlugin {
    fn init_state(&self, registry: &PluginStateRegistry) {
        registry.register(PluginState::new());
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExampleEvent, _>(100, move |event, _ctx| {
            state_clone.with_mut::<PluginState, _, _>(|plugin_state| {
                plugin_state.handle(event);
            });
            EventResult::Handled
        });
    }
}
```

## Built-in Plugins (DefaultPlugins)

| Plugin | Purpose |
|--------|---------|
| `CorePlugin` | Essential commands, keybindings |
| `WindowPlugin` | Window splits and navigation |
| `UIComponentsPlugin` | UI component infrastructure |

## External Feature Plugins

| Plugin | Crate | Purpose |
|--------|-------|---------|
| `CompletionPlugin` | `reovim-plugin-completion` | Text completion |
| `ExplorerPlugin` | `reovim-plugin-explorer` | File browser |
| `HealthCheckPlugin` | `reovim-plugin-health-check` | System health diagnostics |
| `LspPlugin` | `reovim-plugin-lsp` | Language server protocol |
| `MicroscopePlugin` | `reovim-plugin-microscope` | Fuzzy finder |
| `NotificationPlugin` | `reovim-plugin-notification` | User notifications |
| `PairPlugin` | `reovim-plugin-pair` | Auto-pairing brackets |
| `PickersPlugin` | `reovim-plugin-pickers` | Picker extensions |
| `ProfilesPlugin` | `reovim-plugin-profiles` | Configuration profiles |
| `RangeFinderPlugin` | `reovim-plugin-range-finder` | Jump navigation and code folding |
| `SettingsMenuPlugin` | `reovim-plugin-settings-menu` | In-editor settings |
| `StatuslinePlugin` | `reovim-plugin-statusline` | Custom status line |
| `TreesitterPlugin` | `reovim-plugin-treesitter` | Syntax highlighting |
| `WhichKeyPlugin` | `reovim-plugin-which-key` | Keybinding hints |

## Language Plugins

| Plugin | Crate | Extensions |
|--------|-------|------------|
| `RustPlugin` | `reovim-lang-rust` | `.rs` |
| `CPlugin` | `reovim-lang-c` | `.c`, `.h` |
| `JavaScriptPlugin` | `reovim-lang-javascript` | `.js`, `.jsx` |
| `PythonPlugin` | `reovim-lang-python` | `.py` |
| `JsonPlugin` | `reovim-lang-json` | `.json` |
| `TomlPlugin` | `reovim-lang-toml` | `.toml` |
| `MarkdownPlugin` | `reovim-lang-markdown` | `.md` |

## Decoupling Patterns

When extracting plugins to separate crates, avoid tight coupling between core and plugin types.

### BAD: Concrete type dependency

```rust
// Core depends on concrete plugin type - TIGHT COUPLING
// lib/core/src/screen/window.rs
use crate::folding::FoldState;  // Core imports plugin type

pub fn render(&self, fold_state: Option<&FoldState>) {
    if fold_state.is_line_hidden(line) { ... }
}
```

This prevents `FoldState` from moving to a plugin crate because:
- Core directly imports the concrete type
- Moving the type breaks core's compilation
- Circular dependency: plugin depends on core, core depends on plugin type

### GOOD: Trait-based abstraction

```rust
// Core defines abstract trait - LOOSE COUPLING
// lib/core/src/visibility.rs
pub trait VisibilityProvider: Send + Sync {
    fn is_hidden(&self, query: VisibilityQuery) -> bool;
    fn get_marker(&self, query: VisibilityQuery) -> Option<VisibilityMarker>;
}

// lib/core/src/screen/window.rs
pub fn render(&self, visibility: &dyn VisibilityProvider) {
    if visibility.is_hidden(VisibilityQuery::Line(line)) { ... }
}

// Plugin implements the trait - can live in separate crate
// plugins/features/fold/src/state.rs
impl VisibilityProvider for FoldState {
    fn is_hidden(&self, query: VisibilityQuery) -> bool {
        match query {
            VisibilityQuery::Line(line) => self.is_line_hidden(line),
            _ => false,
        }
    }
}
```

### Decoupling Hierarchy

```
┌───────────────────────────────────────────────────────────┐
│                          Core                             │
│  ┌─────────────────────┐   ┌───────────────────────────┐  │
│  │ VisibilityProvider  │◄──│ Window/Screen (uses trait)│  │
│  │       (trait)       │   └───────────────────────────┘  │
│  └─────────────────────┘                                  │
└───────────────────────────────────────────────────────────┘
              ▲
              │ implements (no core dependency on impl)
              │
┌───────────────────────────────────────────────────────────┐
│                    Fold Plugin Crate                      │
│  ┌─────────────────────┐   ┌───────────────────────────┐  │
│  │     FoldState       │──▶│ impl VisibilityProvider   │  │
│  │    FoldManager      │   └───────────────────────────┘  │
│  └─────────────────────┘                                  │
└───────────────────────────────────────────────────────────┘
```

### Key Principles

1. Core defines traits, not concrete types
2. Plugins implement traits
3. Core uses `&dyn Trait` for dynamic dispatch
4. Provide `NoOp` implementations as defaults

### Available Abstraction Traits

| Trait | Purpose | Query Types |
|-------|---------|-------------|
| `VisibilityProvider` | Line/cell visibility | `Line(u32)`, `Cell{line,col}`, `LineRange{start,end}` |

## Related Documentation

- [Plugin Tutorial](../plugins/tutorial.md) - Getting started
- [Plugin Advanced](../plugins/advanced.md) - Advanced patterns
- [Event System](../events/overview.md) - Event bus details
