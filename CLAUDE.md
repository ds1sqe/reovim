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

# Run the main binary (default-members set to runner/)
cargo run

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

# Run benchmarks + generate report (unified command)
cargo run -p perf-report -- bench -v X.Y.Z

# Run benchmarks only
cargo bench -p reovim-bench

# Generate report from existing benchmark data
cargo run -p perf-report -- update -v X.Y.Z

# Run in server mode (TCP on 127.0.0.1:12521)
cargo run -- --server

# Run server with stdio transport
cargo run -- --stdio

# Run server on Unix socket
cargo run -- --listen-socket /tmp/reovim.sock

# Run server on custom TCP port
cargo run -- --listen-tcp 9000

# Run reo-cli client
cargo run -p reo-cli -- mode
cargo run -p reo-cli -- keys 'iHello<Esc>'

# View logs
tail -f ~/.local/share/reovim/reovim.log
```

## Logs and Debugging

Runtime logs are written to `~/.local/share/reovim/reovim.log` when running in server mode. Use `tail -f` to monitor logs in real-time during debugging.

## Architecture

Reovim is a Rust-based neovim-like text editor built with async tokio runtime and crossterm for terminal handling.

### Workspace Structure

- `runner/` - Main binary crate that bootstraps the editor and loads plugins
- `lib/core/` (reovim-core) - Core editor logic: runtime, buffers, events, screen rendering
- `lib/sys/` (reovim-sys) - Re-exports crossterm for terminal abstraction
- `plugins/features/` - Feature plugins (treesitter, fold, completion, explorer, telescope)
- `plugins/languages/` - Language support plugins (rust, c, javascript, python, json, toml, markdown)
- `tools/perf-report/` - Performance report generator CLI
- `tools/reo-cli/` - CLI client for server mode
- `tools/bench/` - Performance benchmarks (criterion)
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

**Rendering System**:
- `lib/core/src/frame/` - Double-buffer frame renderer for diff-based rendering (eliminates flickering)
  - `FrameBuffer` - 2D cell grid (char + style)
  - `FrameRenderer` - Double-buffer with cell-by-cell diff, swap pattern
  - `FrameBufferHandle` - Thread-safe capture for RPC clients
- `lib/core/src/overlay/` - Overlay compositing system (z-order popups)
- `lib/core/src/screen/mod.rs` - Contains `z_order` constants and `LayerBounds` for rendering layers

**Feature Modules**:
- `lib/core/src/leap/` - Two-character jump navigation (s/S)
- `lib/core/src/telescope/` - Fuzzy finder (Space f)
- `lib/core/src/explorer/` - File browser (Space e)
- `lib/core/src/completion/` - Async completion (Ctrl-Space)
- `lib/core/src/modd/` - Multi-dimensional mode state (Focus, EditMode, SubMode)

**Plugin System** (`lib/core/src/plugin/`):
- `Plugin` trait - Interface for all plugins
- `PluginStateRegistry` - Shared state between plugins
- `EventBus` - Publish/subscribe event system for plugin communication
- Plugins are loaded by `runner/src/plugins.rs`

**Treesitter Plugin** (`plugins/features/treesitter/`):
- Syntax highlighting via tree-sitter parsing
- Semantic text objects (functions, classes, parameters)
- Language support via `LanguageSupport` trait
- Languages register dynamically via `RegisterLanguage` event

**Language Plugins** (`plugins/languages/{lang}/`):
- Each language implements `LanguageSupport` trait
- Provides tree-sitter grammar, highlight queries, decoration queries
- Supported: Rust, C, JavaScript, Python, JSON, TOML, Markdown

**RPC / Server Mode** (`lib/core/src/rpc/`):
- `RpcServer` - Coordinates JSON-RPC 2.0 request handling
- `TransportConfig` - Transport selection: Stdio, UnixSocket, Tcp
- `TransportReader`/`TransportWriter` - Async I/O abstraction
- `TransportListener` - Accepts connections for socket/TCP
- `ChannelKeySource` - Injects keys from RPC into runtime
- `FrameBufferHandle` - Unified capture for all RPC formats (RawAnsi, PlainText, CellGrid)
- Default port: 12521 ('r'×100 + 'e'×10 + 'o')
- Server modes: `--server` (persistent, runs forever), `--server --test` (exit when all clients disconnect), `--stdio` (always one-shot)

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

### Performance (v0.6.0)

v0.6.0 uses diff-based rendering via FrameBuffer, trading per-render overhead for zero flickering:

| Metric | v0.5.0 | v0.6.0 |
|--------|--------|--------|
| Window render (10 lines) | 1.63 µs | 10.15 µs |
| Full screen render | 14.50 µs | 62.41 µs |
| Char insert RTT | 55 µs | 108 µs |
| Move down RTT | 751 µs | 806 µs |
| Throughput | ~144k/sec | ~18k/sec |

**Key benefit**: Only changed cells sent to terminal, eliminating flickering.

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
1. **Sync with upstream (CRUCIAL)**:
   ```bash
   git fetch origin develop && git rebase origin/develop
   ```
   - Carefully rebase - resolve any conflicts before landing
   - Contributors own their conflicts
   - Use `git stash` if needed to save work before rebasing

2. **Run full check script**:
   ```bash
   ./scripts/check.sh
   ```
   - Runs `cargo +nightly fmt --all` (format)
   - Runs `cargo build --workspace` (dev + release)
   - Runs `cargo clippy --workspace --all-targets` (zero warnings required)
   - Runs `cargo test --workspace` (all tests must pass)
   - Runs doc tests

3. **Generate performance report**:
   ```bash
   cargo run -p perf-report -- bench -v X.Y.Z
   ```
   - Creates `perf/PERF-X.Y.Z.md` with benchmark results
   - (Optional) Compare with previous version to check for regressions:
     - Window render, full screen render, char insert RTT, move down RTT
     - Throughput should remain stable (~17-18k renders/sec for v0.6.x)

4. **Update `CHANGELOG.md`** with new version entry

5. **Create proposal at `tmp/<version>-commit.md`** with:
   - Commit message
   - Files to stage (include `perf/PERF-X.Y.Z.md`)
   - Verification checklist
   - Performance summary
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
