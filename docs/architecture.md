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
└── lib/sys/        # reovim-sys - terminal abstraction (crossterm)
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
├── screen/         # Terminal rendering
│   ├── mod.rs
│   ├── window.rs
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
├── bind/           # Key bindings
├── completion/     # Text completion engine
├── telescope/      # Fuzzy finder
├── explorer/       # File browser
├── leap/           # Two-character motion
├── jump_list/      # Navigation history
├── registers/      # Copy/paste storage
├── theme/          # Color themes
├── treesitter/     # Syntax highlighting engine
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
