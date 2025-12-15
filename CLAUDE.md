# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build all crates
cargo build

# Build release
cargo build --release

# Run the main binary
cargo run -p reovim

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p reovim-core

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy
```

## Architecture

Reovim is a Rust-based neovim-like text editor built with async tokio runtime and crossterm for terminal handling.

### Workspace Structure

- `main/` - Main binary crate that bootstraps the editor
- `lib/core/` (reovim-core) - Core editor logic: runtime, buffers, events, screen rendering
- `lib/sys/` (reovim-sys) - Re-exports crossterm for terminal abstraction

### Core Architecture

**Runtime** (`lib/core/src/runtime/`) - Central event loop that:
- Owns buffers and screen
- Processes events via mpsc channel (InnerEvent)
- Spawns input broker and event handlers as async tasks

**Event System** (`lib/core/src/event/`):
- `InputEventBroker` - Reads terminal events via crossterm EventStream, dispatches to brokers
- `KeyEventBroker` - Broadcasts key events to subscribed handlers via tokio broadcast channels
- Handlers implement `Subscribe<T>` trait to receive events
- `InnerEvent` enum for internal communication (BufferEvent, CommandEvent, ModeChangeEvent, WindowEvent, RenderSignal, KillSignal)

**Buffer** (`lib/core/src/buffer/`) - Text storage with lines and cursor position

**Command System** (`lib/core/src/command/`):
- `Command` enum - All editor actions (cursor movement, mode switching, text ops)
- `CommandContext` - Execution context (buffer_id, window_id, count)
- `BufferCommandExecutor` - Executes commands on buffers

**Command Line** (`lib/core/src/command_line/`):
- `CommandLine` - Handles `:` command input and cursor
- `ExCommand` - Parsed ex-commands (Quit, Write, WriteQuit, Set)

**Landing** (`lib/core/src/landing.rs`) - Splash screen when no file is opened

**Screen/Window** (`lib/core/src/screen/`):
- `Screen` - Terminal output management with window collection
- `Window` - View into a buffer with anchor positioning and line number modes (Absolute/Relative/Hybrid)

### Event Flow

1. `InputEventBroker` reads terminal events
2. Key events broadcast to handlers via `KeyEventBroker`
3. Handlers send `InnerEvent` to runtime via mpsc channel
4. Runtime processes events and triggers renders

### Key Dependencies

- `tokio` - Async runtime
- `crossterm` (via reovim-sys) - Terminal I/O
- `futures`/`futures-timer` - Async utilities

### Minimum Rust Version

1.80
