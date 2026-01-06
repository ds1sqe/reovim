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

### Debugging Flaky Tests

**Think Twice Before Assuming Race Conditions**: When tests pass/fail intermittently:

1. **Don't immediately add delays** - This masks the real problem
2. **Add debug logging** first to trace execution flow
3. **Look for mode/state synchronization issues** - Often what appears to be a timing race is actually a missing state update

**Example**: `<C-w>h` tests failed randomly because `mode_for_command()` didn't recognize `enter_window_mode`, so the local mode wasn't updated before the next key was looked up. This looked like a race condition but was actually a **missing match arm**.

Use `REOVIM_LOG=debug` to enable debug logging in spawned server processes.

### Plugin Decoupling Policy

**NEVER add plugin-specific code to reovim-core.** Core must remain plugin-agnostic.

**If the current API doesn't support your plugin's needs:**
1. **Propose an API extension** - Design a general-purpose API that solves the problem
2. **Document the proposal** in `tmp/<plugin>-api-proposal.md`
3. **Get approval** before implementing

**Examples of violations (DON'T DO):**
- Adding `WhichKeyOpen` event in core
- Adding `disable_which_key` behavior flag
- Adding `ComposableId::WhichKey` enum variant

**Correct approach:**
- Plugin defines its own events in plugin code
- Use generic APIs: `ComposableId::Custom("my_plugin")`
- Extend core APIs only when multiple plugins would benefit

### Process Safety Policy

**NEVER kill other reovim instances.** Multiple reovim servers can run concurrently for debugging.

**Forbidden commands:**
- `pkill reovim` or `pkill -f reovim`
- `killall reovim`
- Any command that terminates reovim processes you didn't start

**Safe alternatives:**
- Use `reo-cli list` to see running servers
- Use `reo-cli --tcp 127.0.0.1:<PORT> kill` to kill a specific server
- Only kill servers you started in the current session

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

# Run in server mode (TCP on 127.0.0.1:12521, or next available port)
cargo run -- --server
# Server prints "Listening on 127.0.0.1:<PORT>" to stderr

# Run server with stdio transport
cargo run -- --stdio

# Run server on Unix socket
cargo run -- --listen-socket /tmp/reovim.sock

# Run server on custom TCP port
cargo run -- --listen-tcp 9000

# Run reo-cli client
cargo run -p reo-cli -- list              # List running servers
cargo run -p reo-cli -- keys 'iHello<Esc>'  # Inject keys, show status (screen + mode + cursor)
cargo run -p reo-cli -- --tcp 127.0.0.1:12522 keys 'j'  # Connect to specific server
cargo run -p reo-cli -- -i                # Interactive REPL mode

# View logs (timestamped files)
tail -f ~/.local/share/reovim/reovim-*.log

# View latest log
tail -f $(ls -t ~/.local/share/reovim/reovim-*.log | head -1)
```

## Logs and Debugging

Runtime logs are written to timestamped files in `~/.local/share/reovim/` by default (e.g., `reovim-2025-12-22-15-36-20.log`). Each instance creates a new log file with format `reovim-YYYY-MM-DD-HH-MM-SS.log`.

### Log Output Options

```bash
# Default: timestamped file in ~/.local/share/reovim/
reovim myfile.txt

# Custom log file path
reovim --log=/path/to/custom.log myfile.txt

# Log to stderr (useful for debugging)
reovim --log=- myfile.txt
reovim --log=stderr myfile.txt

# Disable logging entirely
reovim --log=none myfile.txt
reovim --log=off myfile.txt

# Server mode with stderr logging (for debugging)
cargo run -- --server --log=-

# Server mode with custom log file
cargo run -- --server --log=/tmp/reovim-server.log

# LSP JSON-RPC message logging (for debugging LSP issues)
reovim --lsp-log=default myfile.rs           # Timestamped lsp-*.log in data dir
reovim --lsp-log=/tmp/lsp.log myfile.rs      # Custom path
cargo run -- --server --log=/tmp/main.log --lsp-log=/tmp/lsp.log  # Both logs
```

### Log Level

Control log verbosity via `REOVIM_LOG` environment variable:

```bash
REOVIM_LOG=debug reovim myfile.txt
REOVIM_LOG=trace reovim --log=- myfile.txt  # Debug to stderr
```

To monitor the latest default log in real-time:
```bash
tail -f $(ls -t ~/.local/share/reovim/reovim-*.log | head -1)
```

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

**Unified Command-Event Pattern** (v0.6.22+):
- Single type serves as both CommandTrait and Event implementation
- Macros: `declare_event_command!` (zero-sized), `declare_counted_event_command!` (with count)
- Benefits: 50% fewer types, ~20 lines less boilerplate per command
- All feature plugins migrated (Fold, Settings, Completion, Explorer, Telescope)
- Example: `ExplorerRefresh` (not `ExplorerRefreshCommand` + `ExplorerRefreshEvent`)
- See: `docs/commands.md`, `docs/plugin-system.md`, `docs/event-system.md`

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

**Multi-Instance Support** (`runner/src/dirs.rs`, `runner/src/server.rs`):
- Port fallback: If default port (12521) is in use, tries 12522, 12523, ... up to 12530
- Port files: `~/.local/share/reovim/servers/<pid>.port` stores the bound port for discovery
- Server prints `Listening on <host>:<port>` to stderr on startup
- `reo-cli list` discovers running servers by scanning port files
- Auto-discovery: `reo-cli` auto-connects to single server, prompts when multiple running
- Clean shutdown removes port file automatically

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

**IMPORTANT**: Claude should NEVER handle git commits. The user will manage ALL git operations. Claude must not have any "opinion" about when or how to commit - only create proposals and let the user decide.

**Changelog Convention**: Every feature, fix, or notable change MUST include an update to `CHANGELOG.md` under the `[Unreleased]` section before proposing a commit.

For any changes, Claude should create a proposal file at `tmp/<feature-name>-commit.md` with:
- Proposed commit message
- List of files changed (including `CHANGELOG.md`)
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
- [docs/event-system.md](./docs/event-system.md) - Event flow details and unified pattern
- [docs/commands.md](./docs/commands.md) - Command system and unified pattern macros
- [docs/plugin-system.md](./docs/plugin-system.md) - Plugin development with unified pattern
- [docs/DEVELOPMENT.md](./docs/DEVELOPMENT.md) - Development guide
- [docs/TESTING.md](./docs/TESTING.md) - Testing guide

### Creating Plugin Commands (Modern Pattern)

When adding new plugin commands, use the unified command-event pattern:

**Zero-sized commands:**
```rust
use reovim_core::declare_event_command;

declare_event_command! {
    MyAction,
    id: "my_action",
    description: "Perform my action",
}
```

**Counted commands (with repeat count):**
```rust
use reovim_core::declare_counted_event_command;

declare_counted_event_command! {
    MyMove,
    id: "my_move",
    description: "Move by count",
}
```

**Commands with custom data:**
```rust
#[derive(Debug, Clone, Copy)]
pub struct MyInputChar {
    pub c: char,
}

impl Event for MyInputChar {
    fn priority(&self) -> u32 { 100 }
}
```

See `plugins/features/explorer/src/command.rs` for a complete example with all three patterns.

### Registering Plugin Settings (Extensible Settings System)

Plugins can register settings to appear in the settings menu using `RegisterSettingSection` and `RegisterOption` events:

```rust
use reovim_core::option::{
    RegisterSettingSection, RegisterOption, OptionSpec, OptionValue,
    OptionCategory, OptionConstraint,
};

fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
    // Register a settings section (optional - auto-created from category if omitted)
    bus.emit(RegisterSettingSection::new("my_plugin", "My Plugin")
        .with_description("Plugin settings")
        .with_order(100));  // Core sections use 0-50

    // Register options
    bus.emit(RegisterOption::new(
        OptionSpec::new("enabled", "Enable feature", OptionValue::Bool(true))
            .with_section("My Plugin")
            .with_display_order(10),
    ));
}
```

See `lib/core/src/plugin/builtin/core.rs` for a complete example of registering core settings.
