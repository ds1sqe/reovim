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
| `reovim-core` | Runtime, buffers, events, screen, commands |
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
  ├── mpsc channel (InnerEvent)                   │
  │                                               │
  └── spawned async tasks:                        │
      ├── InputEventBroker (reads terminal)       │
      ├── KeyEventBroker (broadcasts keys)        │
      ├── CommandHandler (keys → commands)        │
      └── TerminateHandler (Ctrl+D)               │
                                                  │
      ◄─────────── event loop ────────────────────┘
```

## Module Overview

```
lib/core/src/
├── runtime/        # Central event loop
├── buffer/         # Text storage and cursor
├── screen/         # Terminal rendering
│   └── window.rs   # Buffer viewport
├── command/        # Command definitions
│   ├── action.rs   # Command enum
│   ├── context.rs  # Execution context
│   └── executor.rs # Command execution
├── command_line/   # Ex-command parsing (:w, :q)
├── event/          # Event system
│   ├── input.rs    # Terminal event reader
│   ├── key/        # Key broadcast channel
│   ├── handler/    # Event handlers
│   └── inner/      # InnerEvent types
├── motion/         # Cursor movement logic
├── highlight/      # Syntax highlighting
├── modd/           # Editor modes
├── bind/           # Key bindings (private)
└── landing.rs      # Splash screen
```

## Key Components

### Runtime

The central event loop that owns all editor state:

```rust
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub current_mode: Mod,
    pub clipboard: String,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
}
```

**Responsibilities:**
- Process events sequentially through single-threaded loop
- Own all buffers and screen state
- Handle mode transitions
- Coordinate rendering

### Buffer

Text storage with cursor and selection:

```rust
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
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

### Modes

Editor state machine:

```rust
pub enum Mod {
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
    Command,
}
```

## Architecture Patterns

### Single-Threaded Event Loop
- Runtime processes events sequentially
- No locks needed - prevents race conditions
- Responsive via async I/O

### Command Pattern
- KeyEvents translate to `Command` enum
- `CommandContext` carries metadata (buffer_id, count)
- `CommandResult` indicates side effects

### Trait-Based Subscriptions
```rust
pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
```

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
              ▼
           Render
```

## Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime |
| `crossterm` | Terminal I/O |
| `futures` | Async utilities |

## Related Documentation

- [Event System](./event-system.md) - Detailed event flow
- [Command System](./commands.md) - Commands and execution
