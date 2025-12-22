# Reovim Architecture

This document provides an overview of the reovim editor architecture.

## Design Goals

- **Fastest-reaction editor**: Minimal latency and instant response to user input
- **Scalability**: Architecture designed to handle large files and complex operations
- **Async-first**: Non-blocking I/O using tokio runtime

## Workspace Structure

```
reovim/
├── runner/                 # Binary crate - editor entry point
├── lib/core/               # reovim-core - core editor logic
├── lib/sys/                # reovim-sys - terminal abstraction (crossterm)
├── plugins/features/       # External feature plugins
│   ├── fold/               # reovim-plugin-fold
│   ├── settings-menu/      # reovim-plugin-settings-menu
│   ├── completion/         # reovim-plugin-completion
│   ├── explorer/           # reovim-plugin-explorer
│   ├── telescope/          # reovim-plugin-telescope
│   └── treesitter/         # reovim-plugin-treesitter
├── plugins/languages/      # Language plugins
│   ├── rust/               # reovim-lang-rust
│   ├── c/                  # reovim-lang-c
│   ├── javascript/         # reovim-lang-javascript
│   ├── python/             # reovim-lang-python
│   ├── json/               # reovim-lang-json
│   ├── toml/               # reovim-lang-toml
│   └── markdown/           # reovim-lang-markdown
└── tools/
    ├── perf-report/        # Performance report generator
    ├── reo-cli/            # CLI client for server mode
    └── bench/              # Performance benchmarks (criterion)
```

### Dependency Graph

```
┌──────────────────────────────────────────────────────────────┐
│                          RUNNER                               │
│                         (reovim)                              │
└──────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│    CORE      │◄──│ Feature Plugins  │   │ Language Plugins │
│(reovim-core) │   │ (fold, telescope,│   │ (rust, c, js,    │
└──────────────┘   │  explorer, etc.) │   │  python, etc.)   │
        │          └──────────────────┘   └──────────────────┘
        ▼
┌──────────────┐
│     SYS      │
│(reovim-sys)  │
└──────────────┘
        │
        ▼
   crossterm
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `runner` | Bootstrap editor, parse CLI args, configure plugins (AllPlugins), invoke runtime |
| `reovim-core` | Runtime, buffers, events, screen, commands, plugin system, DefaultPlugins |
| `reovim-sys` | Re-exports crossterm for terminal I/O |
| `reovim-plugin-*` | External feature plugins (fold, completion, explorer, telescope, etc.) |
| `reovim-lang-*` | Language support plugins (syntax highlighting, queries) |

## Core Architecture Overview

The editor follows a **central runtime event loop** pattern with async tokio tasks:

```
main.rs
  │
  ▼
Runtime::init() ──────────────────────────────────┐
  │                                               │
  ├── Screen (terminal output)                    │
  ├── Buffers (text storage)                      │
  ├── CommandRegistry (trait-based commands)      │
  ├── mpsc channel (InnerEvent)                   │
  ├── watch channel (ModeState broadcast)         │
  │                                               │
  └── spawned async tasks:                        │
      ├── InputEventBroker (reads terminal)       │
      ├── KeyEventBroker (broadcasts keys)        │
      ├── CommandHandler (keys → commands)        │
      ├── CompletionHandler (async completion)    │
      └── TerminateHandler (Ctrl+C)               │
                                                  │
      ◄─────────── event loop ────────────────────┘
```

## Module Overview

```
lib/core/src/
├── runtime/        # Central event loop
│   ├── core.rs     # Runtime struct
│   ├── event_loop.rs
│   └── handlers.rs
├── event_bus/      # Type-erased event system
│   ├── mod.rs      # Event trait, DynEvent
│   └── bus.rs      # EventBus implementation
├── plugin/         # Plugin system
│   ├── mod.rs      # Public exports
│   ├── traits.rs   # Plugin trait definition
│   ├── context.rs  # PluginContext for registration
│   ├── loader.rs   # PluginLoader for dependency resolution
│   ├── state.rs    # PluginStateRegistry
│   ├── runtime_context.rs  # RuntimeContext for plugins
│   └── builtin/    # Built-in plugins (core, leap)
├── buffer/         # Text storage and cursor
├── compositor/     # Compositing system
│   ├── mod.rs      # ZOrder, ZGroup
│   └── types.rs    # ComposableId, Bounds
├── filetype/       # File type detection
│   └── mod.rs      # FiletypeRegistry, FiletypeInfo
├── interactor/     # Interactor system (input handlers)
│   └── mod.rs      # InteractorId, Interactor trait, InteractorRegistry
├── frame/          # Frame buffer for diff-based rendering
│   ├── mod.rs      # Public API exports
│   ├── buffer.rs   # FrameBuffer - 2D cell grid
│   ├── cell.rs     # Cell - char + fg + bg + modifiers
│   └── renderer.rs # FrameRenderer - double-buffer diff rendering
├── overlay/        # Overlay compositing system
│   ├── mod.rs      # Overlay trait definition
│   ├── compositor.rs # OverlayCompositor - z-order management
│   ├── geometry.rs # OverlayBounds, positioning helpers
│   ├── render.rs   # OverlayRender trait
│   └── selectable.rs # Scrollable, Selectable traits
├── screen/         # Terminal rendering
│   ├── mod.rs
│   ├── window.rs
│   ├── border.rs   # Border system (BorderStyle, BorderConfig)
│   ├── layer.rs    # Layer trait + z-order constants
│   ├── layers/     # Layer implementations
│   │   ├── base.rs       # Tab line, status line (z=0)
│   │   ├── explorer.rs   # File browser sidebar (z=1)
│   │   ├── editor.rs     # Buffer windows (z=2)
│   │   ├── leap.rs       # Jump labels (z=3)
│   │   ├── completion.rs # Completion popup (z=4)
│   │   ├── telescope.rs  # Fuzzy finder (z=5)
│   │   └── settings_menu.rs # Settings overlay (z=6)
│   └── status_line.rs
├── command/        # Command system
│   ├── traits.rs   # CommandTrait, ExecutionContext
│   ├── registry.rs # CommandRegistry
│   ├── id.rs       # CommandId constants
│   ├── deferred.rs # DeferredAction
│   └── builtin/    # Command implementations
├── command_line/   # Ex-command parsing (:w, :q, :e)
├── event/          # Event system
│   ├── input.rs    # Terminal event reader
│   ├── key/        # Key broadcast channel
│   ├── handler/    # Event handlers
│   └── inner/      # InnerEvent types
├── motion/         # Cursor movement logic
├── highlight/      # Syntax highlighting
├── modd/           # Editor modes (ModeState, EditMode, SubMode)
├── modifier/       # Context-aware modifiers
│   ├── mod.rs
│   ├── traits.rs   # Modifier trait
│   ├── registry.rs # ModifierRegistry
│   ├── context.rs  # ModifierContext
│   ├── style.rs    # StyleModifiers
│   └── behavior.rs # BehaviorModifiers
├── bind/           # Key bindings
├── completion/     # Text completion engine
├── telescope/      # Fuzzy finder
├── explorer/       # File browser
├── leap/           # Two-character motion
├── jump_list/      # Navigation history
├── registers/      # Copy/paste storage
├── theme/          # Color themes
├── rpc/            # JSON-RPC server mode
│   ├── server.rs   # RpcServer, request handling
│   ├── transport.rs# Transport layer (Stdio, Socket, TCP)
│   ├── types.rs    # RpcRequest, RpcResponse
│   └── state.rs    # State snapshot types
├── folding.rs      # Code folding state
├── indent.rs       # Indentation guides
└── landing.rs      # Splash screen
```

## Key Components

### Runtime

The central event loop that owns all editor state:

```rust
pub struct Runtime {
    // Buffer management
    pub buffers: BTreeMap<usize, Buffer>,
    pub active_buffer_id: usize,
    pub next_buffer_id: usize,

    // Display
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub color_mode: ColorMode,
    pub theme: Theme,

    // Mode and state
    pub mode_state: ModeState,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub last_command: String,

    // Event channels
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
    pub mode_tx: watch::Sender<ModeState>,
    pub mode_rx: watch::Receiver<ModeState>,

    // Plugin infrastructure (NEW)
    pub event_bus: Arc<EventBus>,
    pub plugin_state: Arc<PluginStateRegistry>,

    // Command system
    pub command_registry: Arc<CommandRegistry>,
    pub registers: Registers,

    // Features (some migrated to plugins, accessed via plugin_state)
    pub explorer_state: Option<ExplorerState>,
    pub jump_list: JumpList,
    pub completion_engine: Arc<CompletionEngine>,
    pub completion_state: CompletionState,
    pub telescope_state: TelescopeState,
    pub telescope_matcher: TelescopeMatcher,
    pub telescope_pickers: HashMap<String, Arc<dyn Picker>>,
    pub leap_state: LeapState,
    pub fold_manager: FoldManager,
    // Note: Treesitter is now a plugin, accessed via plugin_state.text_object_source()
}
```

**Responsibilities:**
- Process events sequentially through single-threaded loop
- Own all buffers and screen state
- Handle mode transitions via `set_mode()`
- Coordinate rendering
- Dispatch deferred actions to feature handlers

### Buffer

Text storage with cursor and selection:

```rust
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
    pub history: UndoHistory,
}
```

**Traits:**
- `TextOps` - insert, delete, content manipulation
- `SelectionOps` - visual mode selection
- `CursorOps` - word navigation

### Screen & Window

Terminal output management:

```rust
pub struct Screen {
    size: ScreenSize,
    out_stream: Box<dyn Write>,
    windows: Vec<Window>,
}

pub struct Window {
    pub anchor: Anchor,           // Position on screen
    pub buffer_id: usize,
    pub buffer_anchor: Anchor,    // Scroll position
    pub line_number: LineNumber,
}
```

### Frame Buffer System

The frame buffer provides diff-based rendering to eliminate terminal flickering using a double-buffer architecture:

```rust
pub struct FrameBuffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
}

pub struct Cell {
    pub char: char,
    pub style: Style,       // fg, bg, attributes
    pub width: u8,          // 1 for ASCII, 2 for wide chars
}

pub struct FrameRenderer {
    front: FrameBuffer,     // Latest complete frame (external readers)
    back: FrameBuffer,      // Currently being rendered to
    capture: Option<Arc<RwLock<FrameBuffer>>>,  // For RPC CellGrid
}
```

**Components:**
- `FrameBuffer` - 2D grid of cells with `get()`/`set()` accessors
- `Cell` - Individual terminal cell (char, style, display width)
- `FrameRenderer` - Double-buffer renderer with cell-by-cell diff
- `FrameBufferHandle` - Thread-safe handle for external readers (RPC)

**Rendering Flow:**
1. Content is rendered to `back` buffer via `buffer_mut()`
2. `flush()` computes diff between `back` and `front`
3. Only changed cells are written to terminal
4. Buffers swap: `front ↔ back`
5. Capture buffer (if enabled) is updated for RPC clients

**Frame Buffer Capture:**
For RPC `CellGrid` format, the renderer can provide a thread-safe capture handle:
```rust
let handle = renderer.enable_capture();  // Returns FrameBufferHandle
let snapshot = handle.snapshot();        // Clone of current frame
```

### Layer System

Layers provide structured z-order rendering:

```rust
pub trait Layer {
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode);
    fn z_order(&self) -> u8;
}
```

**Z-Order Constants:**

Core defines only base z-order constants. Plugins define their own via `OverlayRenderer::z_order()`:

| Layer | Z-Order | Source |
|-------|---------|--------|
| BaseLayer | 0 | Core (`z_order::BASE`) |
| EditorLayer | 2 | Core (`z_order::EDITOR`) |
| Plugin overlays | 100-400 | `OverlayRenderer::z_order()` |

Plugins register overlays with their own z-orders:
- Leap: 100
- Completion: 200
- Telescope: 300
- Settings: 400

`Screen::render_buffered()` renders all layers in z-order to the frame buffer.

### Overlay System

Overlays are composable popup components:

```rust
pub trait Overlay {
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme);
    fn bounds(&self) -> OverlayBounds;
    fn z_order(&self) -> u16;
}
```

**Components:**
- `OverlayCompositor` - Manages overlay stack, renders in z-order
- `OverlayBounds` - Position and size (x, y, width, height)
- `OverlayGeometry` - Helpers for centered/anchored positioning

### Rendering Flow

```
Runtime::render()
    │
    ▼
Screen::render_buffered()
    │
    ├── FrameRenderer::buffer_mut() ──► get back buffer
    │
    ├── Render core layers:
    │   ├── Base layer (tab line, status line)    (z=0)
    │   └── Editor windows                        (z=2)
    │
    ├── Render plugin overlays (via OverlayRegistry):
    │   └── For each visible overlay in z-order:
    │       overlay.render_to_buffer(buffer, theme)
    │       (z-order defined by plugin's OverlayRenderer::z_order())
    │
    └── FrameRenderer::flush()
        │
        ├── Diff: back vs front (cell-by-cell)
        ├── Generate ANSI codes for changed cells only
        ├── Swap buffers: front ↔ back
        └── Update capture buffer (for RPC clients)
        │
        ▼
    Terminal Output (minimal I/O)
```

### Mode State System

Editor mode is represented by a multi-dimensional `ModeState`:

```rust
pub struct ModeState {
    pub interactor_id: ComponentId,  // Editor, CommandLine, or plugin-defined
    pub edit_mode: EditMode,         // Normal, Insert, Visual
    pub sub_mode: SubMode,           // None, Command, OperatorPending, Interactor(id)
}

// ComponentId is extensible (plugins define their own IDs)
pub struct ComponentId(pub &'static str);

// Core defines only essential IDs
impl ComponentId {
    pub const EDITOR: Self = Self("editor");
    pub const COMMAND_LINE: Self = Self("command_line");
    pub const STATUS_LINE: Self = Self("status_line");
    pub const TAB_LINE: Self = Self("tab_line");
}

// Plugins define their own IDs in their crates:
// pub const COMPONENT_ID: ComponentId = ComponentId("explorer");
// pub const COMPONENT_ID: ComponentId = ComponentId("telescope");
```

**Core constructors:**
- `ModeState::normal()` - Editor + Normal mode
- `ModeState::insert()` - Editor + Insert mode
- `ModeState::visual()` - Editor + Visual mode
- `ModeState::command()` - Editor + Command sub-mode
- `ModeState::operator_pending(op, count)` - Operator-pending mode
- `ModeState::with_interactor(id)` - Generic interactor focus
- `ModeState::with_interactor_insert(id)` - Interactor + Insert mode

**State checks:**
- `is_normal()`, `is_insert()`, `is_visual()` - Edit mode checks
- `is_command()`, `is_operator_pending()` - Sub-mode checks
- `is_interactor(id)` - Check if current interactor matches given ID
- `is_editor_focus()` - Check if focused on editor

## Plugin System

The plugin system enables modular features with full lifecycle management.

### Plugin Trait

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

**Lifecycle:**
1. `build()` - Register commands, keybindings
2. `init_state()` - Initialize plugin state in registry
3. `finish()` - Post-registration setup
4. `subscribe()` - Subscribe to events via event bus

### PluginStateRegistry

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

### Event Bus

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

**Example - LeapPlugin:**
```rust
impl Plugin for LeapPlugin {
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

### Built-in Plugins (DefaultPlugins)

| Plugin | Purpose |
|--------|---------|
| `CorePlugin` | Essential commands, keybindings |
| `LeapPlugin` | Two-character jump navigation |
| `WindowPlugin` | Window splits and navigation |
| `UIComponentsPlugin` | UI component infrastructure |

### External Feature Plugins

| Plugin | Crate | Purpose |
|--------|-------|---------|
| `FoldPlugin` | `reovim-plugin-fold` | Code folding |
| `SettingsMenuPlugin` | `reovim-plugin-settings-menu` | In-editor settings |
| `CompletionPlugin` | `reovim-plugin-completion` | Text completion |
| `ExplorerPlugin` | `reovim-plugin-explorer` | File browser |
| `TelescopePlugin` | `reovim-plugin-telescope` | Fuzzy finder |
| `TreesitterPlugin` | `reovim-plugin-treesitter` | Syntax highlighting |

### Language Plugins

| Plugin | Crate | Extensions |
|--------|-------|------------|
| `RustPlugin` | `reovim-lang-rust` | `.rs` |
| `CPlugin` | `reovim-lang-c` | `.c`, `.h` |
| `JavaScriptPlugin` | `reovim-lang-javascript` | `.js`, `.jsx` |
| `PythonPlugin` | `reovim-lang-python` | `.py` |
| `JsonPlugin` | `reovim-lang-json` | `.json` |
| `TomlPlugin` | `reovim-lang-toml` | `.toml` |
| `MarkdownPlugin` | `reovim-lang-markdown` | `.md` |

### Decoupling Patterns

When extracting plugins to separate crates, avoid tight coupling between core and plugin types.

**BAD: Concrete type dependency**
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

**GOOD: Trait-based abstraction**
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

**Decoupling hierarchy:**
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

**Key principles:**
1. Core defines traits, not concrete types
2. Plugins implement traits
3. Core uses `&dyn Trait` for dynamic dispatch
4. Provide `NoOp` implementations as defaults

**Available abstraction traits:**

| Trait | Purpose | Query Types |
|-------|---------|-------------|
| `VisibilityProvider` | Line/cell visibility | `Line(u32)`, `Cell{line,col}`, `LineRange{start,end}` |

## Feature Modules

### Telescope (`lib/core/src/telescope/`)

Fuzzy finder for files, buffers, grep, and commands. Uses nucleo for high-performance fuzzy matching.

**Components:**
- `TelescopeState` - Current UI state (query, selected index)
- `TelescopeMatcher` - nucleo-based fuzzy matching
- `Picker` trait - Extensible picker system
- 7 built-in pickers: files, buffers, live_grep, recent, commands, help, keymaps

**Keybindings:** `Space f` prefix

### Explorer (`lib/core/src/explorer/`)

Tree-view file browser with file operations.

**Components:**
- `ExplorerState` - Tree structure and cursor position
- `ExplorerNode` - File/directory representation
- 25 commands for navigation, tree ops, file ops

**Keybinding:** `Space e` to toggle

### Completion (`lib/core/src/completion/`)

Async word completion with popup menu.

**Components:**
- `CompletionEngine` - Async item fetcher
- `CompletionState` - Active completion session
- Word-based completion from buffer content

**Keybindings:** `Ctrl-Space` to trigger, `Ctrl-n/p` to navigate, `Tab` to confirm

### Leap (`lib/core/src/leap/`)

Two-character motion for quick cursor jumps (inspired by leap.nvim).

**Components:**
- `LeapState` - Current leap session
- `LeapTarget` - Jump target with label
- Bi-directional search with `s`/`S`

**Integration:** Works with operators (`ds{char}{char}` to delete to target)

### Jump List (`lib/core/src/jump_list/`)

Navigation history for Ctrl-O/Ctrl-I.

**Behavior:** Records cursor position before jump commands, allows backtracking

### Registers (`lib/core/src/registers/`)

Multi-register copy/paste storage.

**Features:** Default register `"`, named registers `a-z`

### Treesitter Plugin (`plugins/features/treesitter/`)

Syntax highlighting and semantic features powered by tree-sitter. Now a separate plugin crate.

**Plugin Structure:**
- `TreesitterPlugin` - Main plugin, handles events and lifecycle
- `SharedTreesitterManager` - Shared state for all buffers
- `LanguageRegistry` - Dynamic language registration
- `BufferParser` - Per-buffer incremental parser
- `Highlighter` - Query execution and highlight generation
- `TextObjectResolver` - Semantic text object bounds

**Language Plugins** (`plugins/languages/{lang}/`):
- Each language implements `LanguageSupport` trait
- Registers with treesitter via `RegisterLanguage` event
- Provides grammar, highlight queries, decoration queries

**Supported languages:** Rust, C, JavaScript, Python, JSON, TOML, Markdown

**Features:**
- Dynamic language registration via event bus
- Incremental parsing with debouncing
- Visible-range-only highlighting for performance
- Semantic text objects (function, class, etc.)
- Core accesses via `plugin_state.text_object_source()`

### Code Folding (`lib/core/src/folding.rs`)

Code folding with treesitter-computed ranges.

**Components:**
- `FoldManager` - Per-buffer fold state
- `FoldState` - Collapsed/expanded tracking
- `FoldRange` - Foldable region with preview

**Keybindings:**
- `za` - Toggle fold
- `zo`/`zc` - Open/close fold
- `zR`/`zM` - Open/close all folds

### Interactor System (`lib/core/src/interactor/`)

Manages input-receiving components (UIComponents) that can receive focus and handle input.

**Components:**
- `ComponentId` - Unique identifier for interactors (core defines Editor, CommandLine only)
- `UIComponent` trait - Interface for input-receiving components
- `UIComponentRegistry` - Manages registered UI components and tracks active focus
- `InputResult` - Result enum: `NotHandled`, `Handled`, `SendEvent(InnerEvent)`

**Core ComponentIds:**
| ComponentId | Component |
|-------------|-----------|
| `EDITOR` | Main editor windows |
| `COMMAND_LINE` | Ex-command input |

Plugins define their own `ComponentId` constants and register UIComponent implementations:
```rust
// In plugins/features/explorer/src/lib.rs
pub const COMPONENT_ID: ComponentId = ComponentId("explorer");

impl UIComponent for Explorer {
    fn id(&self) -> ComponentId { COMPONENT_ID }
    fn handle_input(&mut self, input: InputEvent) -> InputResult { ... }
}
```

**Registration Pattern:**
```rust
// Plugins register their UIComponents during build()
fn build(&self, ctx: &mut PluginContext) {
    ctx.register_ui_component(Explorer::new());
}

// Runtime dispatches generically via ComponentId lookup
fn handle_focus_input(&mut self, input: InputEvent) {
    if let Some(component) = self.ui_registry.get_mut(&self.mode_state.interactor_id) {
        component.handle_input(input);
    }
}
```

**Benefits:**
- Core has no knowledge of specific plugins
- Plugins self-register their UIComponents
- Runtime dispatch is fully generic via ComponentId lookup

### Modifier System (`lib/core/src/modifier/`)

Context-aware styling and behavior modifications.

**Components:**
- `Modifier` trait - Interface for context-aware modifications
- `ModifierRegistry` - Manages modifiers with priority ordering and caching
- `ModifierContext` - Context passed to modifiers (window, buffer, mode, filetype)
- `StyleModifiers` - Visual style overrides (border, gutter style, decorations)
- `BehaviorModifiers` - Keybinding overrides and feature flags

**Modifier Priority:**
Modifiers are evaluated in priority order. Higher priority modifiers override lower ones.

```rust
pub trait Modifier: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> i32;
    fn matches(&self, ctx: &ModifierContext<'_>) -> bool;
    fn style_modifiers(&self) -> Option<&StyleModifiers>;
    fn behavior_modifiers(&self) -> Option<&BehaviorModifiers>;
}
```

**Use Cases:**
- Active window highlighting (border style, gutter emphasis)
- Insert mode visual feedback
- Filetype-specific settings
- Disabled commands per mode

### Border System (`lib/core/src/screen/border.rs`)

Configurable window borders for visual separation.

**Components:**
- `BorderStyle` - Predefined styles (None, Single, Double, Rounded, Heavy, Custom)
- `BorderSides` - Selective borders (top, bottom, left, right)
- `BorderConfig` - Builder pattern for border configuration
- `BorderMode` - Layout behavior (Collide for shared borders, Float for individual)

**Border Styles:**
| Style | Characters |
|-------|------------|
| Single | `─│┌┐└┘` |
| Double | `═║╔╗╚╝` |
| Rounded | `─│╭╮╰╯` |
| Heavy | `━┃┏┓┗┛` |

**Usage:**
```rust
let config = BorderConfig::new()
    .style(BorderStyle::Rounded)
    .sides(BorderSides::all())
    .mode(BorderMode::Collide);
```

### Filetype Detection (`lib/core/src/filetype/`)

Automatic file type detection for syntax highlighting and settings.

**Components:**
- `FiletypeRegistry` - Global registry of file type mappings
- `FiletypeInfo` - File type metadata (name, icon, color)

**Detection Methods:**
1. Filename match (e.g., `Makefile`, `Dockerfile`)
2. Extension match (e.g., `.rs`, `.py`, `.js`)

### RPC / Server Mode (`lib/core/src/rpc/`)

JSON-RPC 2.0 server for programmatic control of the editor.

**Server Modes:**
| Mode | Behavior |
|------|----------|
| `--server` (default) | Persistent - runs forever, accepts new connections |
| `--server --test` | Exit when all clients disconnect (for testing/CI) |
| `--stdio` | Always one-shot (stdin closes = done) |

**Components:**
- `RpcServer` - Request coordinator, handles JSON-RPC methods
- `TransportConfig` - Transport selection enum (Stdio, UnixSocket, Tcp)
- `TransportReader`/`TransportWriter` - Async I/O abstraction
- `TransportListener` - Accepts incoming connections (socket/TCP)
- `TransportClient` - Client-side connection helper

**Transport Options:**
| Transport | Use Case |
|-----------|----------|
| TCP (default) | Network access, default port 12521 |
| Unix Socket | Local IPC, lower latency |
| Stdio | Process piping, spawned by parent |

**Integration with Runtime:**
- `ChannelKeySource` - Injects keys from RPC into runtime's key channel
- `FrameBufferHandle` - Unified capture for all RPC formats (RawAnsi, PlainText, CellGrid)
- `InnerEvent::RpcRequest` - Forwards requests to runtime for state queries

**Screen Content Formats:**
| Format | Description |
|--------|-------------|
| `RawAnsi` | Terminal output with ANSI escape codes |
| `PlainText` | ANSI codes stripped, plain text only |
| `CellGrid` | Structured cell data (char + style per cell) |

All formats are derived from the unified `FrameBufferHandle` capture system.

**Server Mode Flow:**
```
main.rs --server
     │
     ▼
TransportListener::bind()
     │
     ▼
┌─► accept() loop ◄───────────────────────────┐
│        │                                    │
│        ▼                                    │
│   TransportConnection                       │
│        │                                    │
│        ├── TransportReader ──► run_reader() │
│        │                            │       │
│        └── TransportWriter ◄── run_writer() │
│                                     │       │
│   (client disconnects) ─────────────┼───────┘
│                                     │
└─────────────────────────────────────┘
                                      │
                                      ▼
                              request channel
                                      │
                                      ▼
               RpcServer::handle_request()
                          │
                          ├── input/keys → ChannelKeySource → Runtime
                          ├── state/* → InnerEvent::RpcRequest → Runtime
                          └── editor/* → InnerEvent::RpcRequest → Runtime
```

**Default Port:** 12521 (derived from ASCII: 'r'×100 + 'e'×10 + 'o' = 11400 + 1010 + 111)

**Edge Cases:**
| Case | Behavior |
|------|----------|
| Client disconnects mid-request | Response send fails silently, server continues |
| New client while old connected | Old connection orphaned, new one takes over |
| No client connected | Requests process, responses dropped |
| `--test` all clients disconnect | Server exits cleanly |

## Architecture Patterns

### Single-Threaded Event Loop
- Runtime processes events sequentially
- No locks needed - prevents race conditions
- Responsive via async I/O

### Trait-Based Command System
- All commands implement `CommandTrait`
- Commands registered in `CommandRegistry`
- Commands return `CommandResult` indicating side effects
- Complex actions deferred via `DeferredAction`

### Trait-Based Subscriptions
```rust
pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
```

### Mode Broadcasting
- `watch` channel broadcasts `ModeState` changes
- Handlers subscribe to mode changes for behavior adaptation

### Layered Rendering
- Buffer stores text
- Window calculates viewport
- Screen manages terminal
- HighlightStore provides styling layers

## State Flow

```
Terminal Input
      │
      ▼
InputEventBroker (crossterm EventStream)
      │
      ▼
KeyEventBroker (tokio broadcast)
      │
      ├──────────────────┐
      ▼                  ▼
CommandHandler      TerminateHandler
      │                  │
      ▼                  ▼
CommandEvent         KillSignal
      │                  │
      └───────┬──────────┘
              ▼
         Runtime (mpsc)
              │
              ▼
         Process Event
              │
              ├── CommandEvent → execute command
              ├── ModeChangeEvent → update mode
              ├── CompletionEvent → update completion
              ├── TelescopeEvent → update telescope
              ├── LeapEvent → handle leap
              ├── ExplorerEvent → handle explorer
              └── ...
              │
              ▼
           Render
```

## Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime |
| `crossterm` | Terminal I/O |
| `futures` | Async utilities |
| `nucleo` | Fuzzy matching (for telescope) |
| `parking_lot` | Synchronization primitives |
| `tree-sitter` | Incremental parsing |

## Related Documentation

- [Event System](./event-system.md) - Detailed event flow
- [Command System](./commands.md) - Commands and execution
