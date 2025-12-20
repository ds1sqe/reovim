# Reovim Architecture

This document provides an overview of the reovim editor architecture.

## Design Goals

- **Fastest-reaction editor**: Minimal latency and instant response to user input
- **Scalability**: Architecture designed to handle large files and complex operations
- **Async-first**: Non-blocking I/O using tokio runtime

## Workspace Structure

```
reovim/
├── main/           # Binary crate - editor entry point
├── lib/core/       # reovim-core - core editor logic
├── lib/sys/        # reovim-sys - terminal abstraction (crossterm)
└── tools/reo-cli/  # CLI client for server mode
```

### Dependency Graph

```
┌──────────┐     ┌──────────────┐     ┌─────────────┐
│   MAIN   │────▶│     CORE     │────▶│     SYS     │
│ (reovim) │     │ (reovim-core)│     │ (reovim-sys)│
└──────────┘     └──────────────┘     └─────────────┘
     │                  │                    │
     │                  │                    │
     ▼                  ▼                    ▼
  Binary           Runtime, Buffer      crossterm
  Entry            Events, Screen       re-exports
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `main` | Bootstrap editor, parse CLI args, invoke runtime |
| `reovim-core` | Runtime, buffers, events, screen, commands, features |
| `reovim-sys` | Re-exports crossterm for terminal I/O |

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
├── buffer/         # Text storage and cursor
├── compositor/     # Compositing system
│   ├── mod.rs      # ZOrder, ZGroup
│   └── types.rs    # ComposableId, Bounds
├── filetype/       # File type detection
│   └── mod.rs      # FiletypeRegistry, FiletypeInfo
├── focus/          # Focus target system
│   └── mod.rs      # FocusId, FocusTarget, FocusRegistry
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
│   │   ├── which_key.rs  # Which-key panel (z=5)
│   │   ├── telescope.rs  # Fuzzy finder (z=6)
│   │   └── settings_menu.rs # Settings overlay (z=7)
│   ├── status_line.rs
│   └── which_key.rs
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
├── modd/           # Editor modes (ModeState)
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
├── treesitter/     # Syntax highlighting engine
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

    // Command system
    pub command_registry: Arc<CommandRegistry>,
    pub registers: Registers,

    // Features
    pub explorer_state: Option<ExplorerState>,
    pub jump_list: JumpList,
    pub which_key_panel: WhichKeyPanel,
    pub completion_engine: Arc<CompletionEngine>,
    pub completion_state: CompletionState,
    pub telescope_state: TelescopeState,
    pub telescope_matcher: TelescopeMatcher,
    pub telescope_pickers: HashMap<String, Arc<dyn Picker>>,
    pub leap_state: LeapState,
    pub treesitter: TreesitterManager,
    pub fold_manager: FoldManager,
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

| Layer | Z-Order | Description |
|-------|---------|-------------|
| BaseLayer | 0 | Tab line, status line |
| ExplorerLayer | 1 | File browser sidebar |
| EditorLayer | 2 | Main buffer windows |
| LeapLayer | 3 | Two-character jump labels |
| CompletionLayer | 4 | Completion popup |
| WhichKeyLayer | 5 | Key binding hints |
| TelescopeLayer | 6 | Fuzzy finder overlay |
| SettingsMenuLayer | 7 | Settings configuration |

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
    ├── Render layers in z-order to back buffer:
    │   ├── Tab line, status line           (z=0)
    │   ├── Explorer sidebar                (z=1)
    │   ├── Editor windows                  (z=2)
    │   ├── Leap labels                     (z=3)
    │   ├── Completion popup                (z=4)
    │   ├── Which-key panel                 (z=5)
    │   ├── Telescope overlay               (z=6)
    │   └── Settings menu                   (z=7)
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
    pub focus: Focus,        // Editor, Explorer, Telescope
    pub edit_mode: EditMode, // Normal, Insert, Visual
    pub sub_mode: SubMode,   // None, Command, OperatorPending, Leap
}
```

**Convenience constructors:**
- `ModeState::normal()` - Editor + Normal mode
- `ModeState::insert()` - Editor + Insert mode
- `ModeState::visual()` - Editor + Visual mode
- `ModeState::command()` - Editor + Command sub-mode
- `ModeState::explorer()` - Explorer focus
- `ModeState::telescope()` - Telescope focus
- `ModeState::operator_pending(op, count)` - Operator-pending mode
- `ModeState::leap(direction, op, count)` - Leap motion mode

**State checks:**
- `is_normal()`, `is_insert()`, `is_visual()` - Edit mode checks
- `is_command()`, `is_operator_pending()`, `is_leap()` - Sub-mode checks
- `is_editor_focus()`, `is_explorer_focus()`, `is_telescope_focus()` - Focus checks

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

### Which-Key (`lib/core/src/screen/which_key.rs`)

Popup panel showing available keybindings after prefix keys.

**Behavior:** Appears after timeout when prefix key is pressed (e.g., `g`, `Space`)

### Jump List (`lib/core/src/jump_list/`)

Navigation history for Ctrl-O/Ctrl-I.

**Behavior:** Records cursor position before jump commands, allows backtracking

### Registers (`lib/core/src/registers/`)

Multi-register copy/paste storage.

**Features:** Default register `"`, named registers `a-z`

### Treesitter (`lib/core/src/treesitter/`)

Syntax highlighting and semantic features powered by tree-sitter.

**Components:**
- `TreesitterManager` - Central manager for all buffers
- `BufferParser` - Per-buffer incremental parser
- `Highlighter` - Query execution and highlight generation
- `GrammarRegistry` - Language detection from file extension
- `QueryCache` - Compiled query caching
- `TextObjectResolver` - Semantic text object bounds

**Supported languages:** Rust, C, JavaScript, Python, JSON, TOML, Markdown

**Features:**
- Incremental parsing with 50ms debounce
- Visible-range-only highlighting for performance
- Semantic text objects (function, class, etc.)
- Fold range computation

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

### Focus System (`lib/core/src/focus/`)

Manages which component receives input events.

**Components:**
- `FocusId` - Unique identifier for focus targets (Editor, Telescope, Explorer, Settings, Custom)
- `FocusTarget` trait - Interface for input-receiving components
- `FocusRegistry` - Manages registered focus targets and tracks active focus

**Focus IDs:**
| FocusId | Component |
|---------|-----------|
| `EDITOR` | Main editor windows |
| `TELESCOPE` | Fuzzy finder |
| `EXPLORER` | File browser |
| `SETTINGS` | Settings menu |

**Input Routing:**
```rust
// Runtime routes FocusInputEvent based on active focus
match mode_state.focus_id {
    FocusId::TELESCOPE => self.handle_telescope_input(event),
    FocusId::EXPLORER => self.focus_registry.handle_input(focus_id, event),
    FocusId::EDITOR => self.handle_editor_input(event),
    _ => self.focus_registry.handle_input(focus_id, event),
}
```

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
