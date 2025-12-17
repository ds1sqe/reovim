# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Goals

- **Fastest-reaction editor**: Prioritize minimal latency and instant response to user input
- **Scalability**: Architecture designed to scale with large files and complex operations

## Code Standards

### Zero-Warning Policy

This project enforces a **zero-warning policy**. All code must compile without any warnings from:
- `cargo build`
- `cargo clippy`

No warnings are acceptable. Fix all warnings before committing.

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

# Run benchmarks
cargo bench -p reovim-core

# Generate performance report
cargo run -p perf-report -- update --version X.Y.Z
```

## Architecture

Reovim is a Rust-based neovim-like text editor built with async tokio runtime and crossterm for terminal handling.

### Workspace Structure

- `main/` - Main binary crate that bootstraps the editor
- `lib/core/` (reovim-core) - Core editor logic: runtime, buffers, events, screen rendering
- `lib/sys/` (reovim-sys) - Re-exports crossterm for terminal abstraction
- `tools/perf-report/` - Performance report generator CLI
- `perf/` - Versioned performance reports (PERF-{version}.md)

### Core Architecture

**Runtime** (`lib/core/src/runtime/`) - Central event loop that:
- Owns buffers and screen
- Processes events via mpsc channel (InnerEvent)
- Spawns input broker and event handlers as async tasks

**Event System** (`lib/core/src/event/`):
- `InputEventBroker` - Reads terminal events via crossterm EventStream, dispatches to brokers
- `KeyEventBroker` - Broadcasts key events to subscribed handlers via tokio broadcast channels
- Handlers implement `Subscribe<T>` trait to receive events
- `InnerEvent` enum for internal communication (BufferEvent, CommandEvent, ModeChangeEvent, PendingKeysEvent, WindowEvent, RenderSignal, KillSignal)

**Buffer** (`lib/core/src/buffer/`) - Text storage with lines and cursor position

**Command System** (`lib/core/src/command/`):
- `Command` enum - All editor actions (cursor movement, mode switching, text ops)
- `CommandContext` - Execution context (buffer_id, window_id, count)
- `BufferCommandExecutor` - Executes commands on buffers

**Command Line** (`lib/core/src/command_line/`):
- `CommandLine` - Handles `:` command input and cursor
- `ExCommand` - Parsed ex-commands (Quit, Write, WriteQuit, Set)

**Landing** (`lib/core/src/landing.rs`) - Splash screen when no file is opened

**Motion** (`lib/core/src/motion/`) - Cursor movement logic:
- `Motion` enum - All movement types (character, word, line, document)
- `apply()` method - Applies motion to buffer, returns new position

**Screen/Window** (`lib/core/src/screen/`):
- `Screen` - Terminal output management with window collection
- `Window` - View into a buffer with anchor positioning and line number modes (Absolute/Relative/Hybrid)

**Feature Modules**:
- `lib/core/src/leap/` - Two-character jump navigation (s/S)
- `lib/core/src/telescope/` - Fuzzy finder (Space f)
- `lib/core/src/explorer/` - File browser (Space e)
- `lib/core/src/completion/` - Async completion (Ctrl-Space)
- `lib/core/src/modd/` - Multi-dimensional mode state (Focus, EditMode, SubMode)

### Event Flow

1. `InputEventBroker` reads terminal events
2. Key events broadcast to handlers via `KeyEventBroker`
3. Handlers send `InnerEvent` to runtime via mpsc channel
4. Runtime processes events and triggers renders

### Key Dependencies

- `tokio` - Async runtime
- `crossterm` (via reovim-sys) - Terminal I/O
- `futures`/`futures-timer` - Async utilities
- `criterion` - Benchmarking framework
- `nucleo` - Fuzzy matching for telescope

### Minimum Rust Version

1.92 (Rust 2024 edition)

### Performance (v0.4.5)

- Window render: 473ns - 2.79µs
- Full screen render: ~6µs
- Input RTT: 28µs (char insert), 45µs (word forward)
- Movement RTT: 383µs (down), 45µs (right)
- Throughput: ~400k renders/sec
- Mode switch: 18µs

## Release Process

### Version Bump

Use the bump-version script to update version across the workspace:

```bash
./scripts/bump-version.sh 0.X.Y
```

This script:
- Updates `version` in workspace `Cargo.toml`
- Updates `reovim-core` and `reovim-sys` dependency versions
- Runs `cargo check` to verify

### Release Checklist

1. Bump version using `./scripts/bump-version.sh X.Y.Z`
2. Update `CHANGELOG.md` with new version entry
3. Run `cargo build` and `cargo clippy` (zero warnings required)
4. Run `cargo test`

### Git Commits

**IMPORTANT**: Claude should NOT handle git commits. The user will manage all git operations.

For releases, Claude should create a proposal file at `tmp/<version>-commit.md` with:
- Proposed commit message
- List of files changed
- Verification checklist
- Git commands for user to run

### Ready-to-Take-Off & Request-for-Landing

**Ready-to-Take-Off (Start of Work):**
1. Read `tmp/*` for context from previous sessions
2. Check `git worktree list` for active branches
3. Check `git status` for uncommitted work
4. Understand the task and plan implementation
5. **Suggest a runway (branch)** for the work:
   - `develop` - Upstream, main development branch
   - `feat/*` - Feature branches
   - `fix/*` - Bug fix branches

**Request-for-Landing (End of Work):**
1. **Sync with upstream (CRUCIAL)**: `git fetch origin develop && git rebase origin/develop`
   - Resolve any conflicts before landing - contributors own their conflicts
2. Run `cargo build` and `cargo clippy` (zero warnings)
3. Run `cargo test`
4. Generate perf report: `cargo run -p perf-report -- update --version X.Y.Z`
5. Update `CHANGELOG.md`
6. Create proposal at `tmp/<version>-commit.md` with:
   - Commit message
   - Files to stage
   - Verification checklist
   - Git commands for user

### Cross-Session Communication

The `tmp/` directory is used for communication between Claude sessions:
- **Read `tmp/*` at start** to see proposals/notes from previous Claude sessions
- **Write to `tmp/*.md`** to leave proposals or context for user/future sessions

### Git Worktrees

Check `git worktree list` to see active worktrees. Common setup:
- `/home/ds1sqe/proj/reovim` - Main development (`develop` branch)
- `/home/ds1sqe/proj/reovim_perf` - Performance work (`feat/perf` branch)

## Detailed Documentation

For in-depth information, see:
- [docs/architecture.md](./docs/architecture.md) - Full architecture overview
- [docs/event-system.md](./docs/event-system.md) - Event flow details
- [docs/commands.md](./docs/commands.md) - Command system
- [docs/DEVELOPMENT.md](./docs/DEVELOPMENT.md) - Development guide
- [docs/TESTING.md](./docs/TESTING.md) - Testing guide
