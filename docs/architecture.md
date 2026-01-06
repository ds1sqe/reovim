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
│   ├── completion/         # reovim-plugin-completion
│   ├── explorer/           # reovim-plugin-explorer
│   ├── health-check/       # reovim-plugin-health-check
│   ├── lsp/                # reovim-plugin-lsp
│   ├── microscope/         # reovim-plugin-microscope (fuzzy finder)
│   ├── notification/       # reovim-plugin-notification
│   ├── pair/               # reovim-plugin-pair
│   ├── pickers/            # reovim-plugin-pickers
│   ├── profiles/           # reovim-plugin-profiles
│   ├── range-finder/       # reovim-plugin-range-finder (jump/fold)
│   ├── settings-menu/      # reovim-plugin-settings-menu
│   ├── statusline/         # reovim-plugin-statusline
│   ├── treesitter/         # reovim-plugin-treesitter
│   └── which-key/          # reovim-plugin-which-key
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
│(reovim-core) │   │ (range-finder,   │   │ (rust, c, js,    │
└──────────────┘   │ microscope, lsp) │   │  python, etc.)   │
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
| `reovim-plugin-*` | External feature plugins (range-finder, microscope, lsp, completion, explorer, etc.) |
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
│   └── builtin/    # Built-in plugins (core)
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
├── highlight/      # Syntax highlighting types
├── animation/      # Animation system for visual effects
│   ├── mod.rs      # AnimationSystem, public API
│   ├── color.rs    # AnimatedColor (pulse, shimmer, transition)
│   ├── effect.rs   # Effect, EffectTarget, SweepConfig
│   ├── controller.rs # AnimationController (background task)
│   ├── easing.rs   # EasingFn (linear, ease-in/out, sine)
│   └── stage.rs    # AnimationRenderStage
├── syntax/         # Abstract syntax provider traits
│   └── mod.rs      # SyntaxProvider, SyntaxFactory, EditInfo
├── modd/           # Editor modes (ModeState, EditMode, SubMode)
├── modifier/       # Context-aware modifiers
│   ├── mod.rs
│   ├── traits.rs   # Modifier trait
│   ├── registry.rs # ModifierRegistry
│   ├── context.rs  # ModifierContext
│   ├── style.rs    # StyleModifiers
│   └── behavior.rs # BehaviorModifiers
├── bind/           # Key bindings
├── completion/     # Completion core types (CompletionContext, CompletionItem)
├── telescope/      # Fuzzy finder
├── explorer/       # File browser
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
    // Note: Treesitter is now a plugin, accessed via plugin_state.text_object_source()
    // Note: Jump navigation (range-finder) is now a plugin
}
```

**Responsibilities:**
- Process events sequentially through single-threaded loop
- Own all buffers and screen state
- Handle mode transitions via `set_mode()`
- Coordinate rendering
- Dispatch deferred actions to feature handlers

### Buffer

Text storage with cursor, selection, and syntax:

```rust
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
    pub history: UndoHistory,
    syntax: Option<Box<dyn SyntaxProvider>>,  // Buffer-owned syntax state
}
```

**Traits:**
- `TextOps` - insert, delete, content manipulation
- `SelectionOps` - visual mode selection
- `CursorOps` - word navigation

**Syntax Methods:**
- `attach_syntax()` - Attach syntax provider (called by runtime on file open)
- `syntax()` - Get immutable reference to syntax provider
- `syntax_mut()` - Get mutable reference for incremental parsing

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
- Range-Finder (jump labels): 110
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

### Render Pipeline Stages

The render pipeline processes buffer content through composable stages:

```
Buffer → Visibility → Highlighting → Decorations → Visual → Indent → Signs → VirtualText → FrameBuffer
```

Each stage enriches `RenderData` with layer-specific information:

| Stage | Priority | Description |
|-------|----------|-------------|
| Visibility | 50 | Apply folding (collapsed/hidden lines) |
| Highlighting | 100 | Treesitter syntax highlighting |
| Decorations | 120 | Language decorations (markdown headers, etc.) |
| Visual | 150 | Visual mode selection highlighting |
| Animation | 200 | Animated effects (yank, paste feedback) |
| Indent | 220 | Indent guide computation |
| Signs | 230 | Sign column (diagnostics, git, etc.) |
| VirtualText | 240 | Inline diagnostic messages |

See [render-pipeline.md](./render-pipeline.md) for detailed documentation.

### Animation System

Visual effects are managed by `AnimationController`:

```rust
pub struct AnimationController {
    effects: HashMap<AnimationId, ActiveEffect>,
    frame_rate: u32,  // Default: 30 fps
}
```

Built-in animations:
- **Yank**: Gold fade (400ms) on yanked region
- **Paste**: Cyan pulse (600ms, 2 cycles) on pasted region
- **Landing**: Breathing effect on ASCII art

See [animation-system.md](./animation-system.md) for API documentation.

### Decoration System

Language-aware visual rendering via `LanguageRenderer` trait:

```rust
pub trait LanguageRenderer: Send + Sync {
    fn render(&self, buffer: &Buffer) -> Vec<LineDecoration>;
}
```

Decorations can:
- Replace text (e.g., `#` → ` ◉ ` for markdown headings)
- Conceal characters (hide `**` around bold text)
- Add virtual text

See [decoration-system.md](./decoration-system.md) for implementation guide.

### Visibility Provider

The `VisibilityProvider` abstraction supports code folding:

```rust
pub enum LineVisibility {
    Visible,
    Collapsed { marker: String },
    Hidden,
}

pub trait VisibilityProvider {
    fn visibility_for_line(&self, line: usize) -> LineVisibility;
}
```

The Range-Finder plugin implements folding based on treesitter structure.

### Indent Guides

`IndentAnalyzer` computes indent guide positions:

```rust
pub struct IndentGuide {
    pub column: usize,
    pub is_active: bool,  // Cursor's indent level
}
```

Guides are rendered as vertical lines (`│`) showing code structure.

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
    pub const WINDOW: Self = Self("window");  // Window mode sub-mode
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
- `accepts_char_input()` - Check if mode accepts character input (excludes Window mode)

**Hierarchical Display:**

The status line shows mode in hierarchical format: `Kind | Mode | SubMode`

```rust
// Examples:
"Editor | Normal"           // Basic editor
"Editor | Normal | Window"  // Window mode active
"Editor | Insert"           // Insert mode
"Explorer | Normal"         // Explorer focused
"Editor | Normal | Operator" // Operator pending (d, y, c)
```

Generated by `ModeState::hierarchical_display()` method.

### Keybinding System

Keybindings are organized by `KeymapScope`:

```rust
pub enum KeymapScope {
    Component { id: ComponentId, mode: EditModeKind },  // e.g., editor_normal, explorer_normal
    SubMode(SubModeKind),                               // e.g., Command, OperatorPending, Window
    DefaultNormal,                                       // Fallback for all components in Normal mode
}
```

**Scope Resolution:**

When looking up a keybinding, the system checks scopes in order:
1. **Primary scope** - Component-specific (e.g., `editor_normal`)
2. **DefaultNormal fallback** - Only for Component + Normal mode

```rust
// Example: Ctrl-W from Explorer
// 1. Check explorer_normal scope -> not found
// 2. Check DefaultNormal scope -> found! (enter_window_mode)
```

**DefaultNormal Scope:**

The `DefaultNormal` scope provides fallback keybindings that work in all plugin windows:
- `Ctrl-W` - Enter window mode (works from Editor, Explorer, etc.)

This eliminates the need for each plugin to register common bindings.

**Window Mode Scope:**

Window mode uses `SubMode::Interactor(ComponentId::WINDOW)`:

| Key | Command |
|-----|---------|
| `h/j/k/l` | Focus window in direction |
| `H/J/K/L` | Move window in direction |
| `x` + `h/j/k/l` | Swap with adjacent window |
| `s/v` | Split horizontal/vertical |
| `c/o/=` | Close/only/equalize |
| `Escape` | Exit window mode |

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

**Example Plugin:**
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

### Built-in Plugins (DefaultPlugins)

| Plugin | Purpose |
|--------|---------|
| `CorePlugin` | Essential commands, keybindings |
| `WindowPlugin` | Window splits and navigation |
| `UIComponentsPlugin` | UI component infrastructure |

### External Feature Plugins

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

### Completion Plugin (`plugins/features/completion/`)

Auto-completion with background processing, following the treesitter decoupling pattern.

**Plugin Structure:**
```
plugins/features/completion/src/
├── lib.rs          # CompletionPlugin only
├── state.rs        # SharedCompletionManager
├── window.rs       # CompletionPluginWindow
├── cache.rs        # CompletionCache (ArcSwap)
├── saturator.rs    # Background completion task
├── registry.rs     # SourceRegistry, SourceSupport
├── events.rs       # RegisterSource event
├── commands.rs     # Unified command-event types
└── source/
    └── buffer.rs   # BufferWordsSource
```

**Core Traits (in `lib/core/src/completion/`):**
```rust
pub struct CompletionContext {
    pub buffer_id: usize,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub line: String,
    pub prefix: String,
    pub word_start_col: u32,
}

pub struct CompletionItem {
    pub label: String,
    pub source_id: String,
    pub kind: CompletionKind,
    pub insert_text: Option<String>,
    pub detail: Option<String>,
}
```

**Plugin Architecture:**
- `SharedCompletionManager` - Thread-safe wrapper holding registry, cache, saturator
- `CompletionCache` - ArcSwap-based lock-free cache for render access
- `CompletionSaturator` - Background tokio task for non-blocking completion
- `SourceRegistry` - Dynamic source registration with priority ordering
- `SourceSupport` trait - Interface for completion sources to implement

**Data Flow:**
```
┌─────────────────┐     ┌───────────────────────┐     ┌─────────────────┐
│  Trigger Event  │────▶│ CompletionSaturator   │────▶│ CompletionCache │
│   (Alt-Space)   │     │     (background)      │     │    (ArcSwap)    │
└─────────────────┘     └───────────────────────┘     └─────────────────┘
                                                              │
                                                              ▼
                                                  ┌────────────────────────┐
                                                  │ CompletionPluginWindow │
                                                  │    (lock-free read)    │
                                                  └────────────────────────┘
```

**Source Registration:**
External plugins register completion sources via `RegisterSource` event:
```rust
bus.emit(RegisterSource {
    source: Arc::new(LspCompletionSource::new(client)),
});
```

**Built-in Source:** `BufferWordsSource` - Extracts words from current buffer

**Keybindings:**
- `Alt-Space` (insert mode) - Trigger completion
- `Ctrl-n`/`Ctrl-p` - Navigate suggestions
- `Enter`/`Tab` - Confirm selection
- `Escape` - Dismiss popup

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

### Syntax System (`lib/core/src/syntax/`)

Buffer-centric syntax highlighting architecture inspired by Helix. Each buffer owns its syntax state.

**Core Traits (no tree-sitter dependency):**

```rust
/// Abstract syntax provider for buffer highlighting
pub trait SyntaxProvider: Send + Sync {
    fn language_id(&self) -> &str;
    fn highlight_range(&self, content: &str, start: u32, end: u32) -> Vec<Highlight>;
    fn parse(&mut self, content: &str);
    fn parse_incremental(&mut self, content: &str, edit: &EditInfo);
    fn is_parsed(&self) -> bool;
}

/// Factory for creating syntax providers
pub trait SyntaxFactory: Send + Sync {
    fn create_syntax(&self, file_path: &str, content: &str) -> Option<Box<dyn SyntaxProvider>>;
    fn supports_file(&self, file_path: &str) -> bool;
}
```

**Buffer Integration:**

```rust
pub struct Buffer {
    // ...existing fields...
    syntax: Option<Box<dyn SyntaxProvider>>,
}

impl Buffer {
    pub fn attach_syntax(&mut self, syntax: Box<dyn SyntaxProvider>);
    pub fn syntax(&self) -> Option<&dyn SyntaxProvider>;
    pub fn syntax_mut(&mut self) -> Option<&mut dyn SyntaxProvider>;
}
```

**Data Flow:**

```
1. File Opened
   └── Runtime detects language via SyntaxFactory
       └── Creates SyntaxProvider (e.g., TreeSitterSyntax)
           └── Attaches to buffer.syntax

2. Buffer Edit
   └── buffer.syntax_mut().parse_incremental(content, edit)

3. Render
   └── RenderData::from_buffer()
       └── buffer.syntax().highlight_range(content, start, end)
           └── Highlights applied to framebuffer
```

**TreeSitterSyntax (in plugin):**

```rust
// plugins/features/treesitter/src/syntax.rs
pub struct TreeSitterSyntax {
    language_id: String,
    parser: Parser,
    tree: Option<Tree>,
    query: Arc<Query>,  // Pre-compiled highlights query
    highlighter: Highlighter,
}

impl SyntaxProvider for TreeSitterSyntax {
    fn highlight_range(&self, content: &str, start: u32, end: u32) -> Vec<Highlight> {
        // Use tree-sitter to generate highlights
    }
    // ...
}
```

**Benefits:**
- **Clear ownership**: Buffer owns its syntax state
- **No HashMap lookups**: Direct access via `buffer.syntax()`
- **Automatic cleanup**: Syntax dropped when buffer dropped
- **Extensible**: Easy to add other backends (regex, LSP, etc.)

### Background Saturator (Performance Optimization)

To eliminate scroll lag caused by synchronous syntax highlighting, reovim uses a **per-buffer saturator** pattern. The saturator is a background tokio task that computes highlights/decorations asynchronously while render reads from a lock-free cache.

**Problem Solved:**
- Synchronous `syntax.highlight_range()` blocked render for ~46ms
- Caused visible stuttering during markdown scrolling

**Solution:**
- Background task owns syntax/decoration providers
- Computes highlights in parallel with render
- Lock-free `ArcSwap` cache for instant reads (~6µs)
- `RenderSignal` triggers re-render when cache updates

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│   Render    │────▶│ ArcSwap Cache    │◀────│  Saturator  │
│  (instant)  │     │   (lock-free)    │     │ (background)│
└─────────────┘     └──────────────────┘     └─────────────┘
      │                                              │
      │ ~6µs read                                    │ ~46ms compute
      └──────────────────────────────────────────────┘
                    Total: ~0.5ms render
```

**Key Design:**
- `mpsc::channel(1)` with `try_send()` - only latest viewport matters
- Cache stores `(hash, data)` per line for validation
- `ArcSwap::store()` for atomic swap (no locks)

See [Saturator Documentation](./saturator.md) for full details.

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

**Multi-Instance Support** (`runner/src/dirs.rs`, `runner/src/server.rs`):
- **Port fallback**: If default port (12521) is in use, tries 12522, 12523, ... up to 12530
- **Port files**: Each server writes `~/.local/share/reovim/servers/<pid>.port` for discovery
- **Startup output**: Server prints `Listening on <host>:<port>` to stderr
- **Discovery**: `reo-cli list` scans port files to find running servers
- **Auto-connect**: `reo-cli` auto-connects to single server, prompts when multiple running
- **Clean shutdown**: Port files removed automatically when server exits

**Edge Cases:**
| Case | Behavior |
|------|----------|
| Client disconnects mid-request | Response send fails silently, server continues |
| New client while old connected | Old connection orphaned, new one takes over |
| No client connected | Requests process, responses dropped |
| `--test` all clients disconnect | Server exits cleanly |
| Default port in use | Tries next port (12522, 12523, ...) |
| All ports 12521-12530 in use | Server fails with clear error |
| Server crashes | Stale port file cleaned up on next `reo-cli list` |

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
              ├── ExplorerEvent → handle explorer
              ├── RangeFinderEvent → handle jump/fold
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
- [Saturator](./saturator.md) - Background syntax highlighting architecture
