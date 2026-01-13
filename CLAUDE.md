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

**When debugging in background:** Always specify a log file output instead of stderr:
```bash
# GOOD: Use a specific log file
REOVIM_LOG=debug cargo run -- --server --log=/tmp/reovim-debug.log file.txt &

# BAD: Don't use stderr (--log=-) for background processes
REOVIM_LOG=debug cargo run -- --server --log=- file.txt &  # Output gets mixed up
```
Then monitor the log file:
```bash
tail -f /tmp/reovim-debug.log
```

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

**Safe workflow for testing:**
1. **Before starting a server**: Run `reo-cli list` to see existing servers
2. **Start your server**: Note the PID/port from stderr output or `reo-cli list`
3. **Track your servers**: Keep a list of PIDs you started in this session
4. **After testing**: Only kill the specific server(s) YOU started: `reo-cli --tcp 127.0.0.1:<PORT> kill`
5. **Never assume**: Don't kill servers just because they exist - they might be from another session

**Example safe workflow:**
```bash
# Check existing servers first
cargo run -p reo-cli -- list

# Start server and note its port from output
cargo run -- --server Cargo.toml &  # Note: prints "Listening on 127.0.0.1:12521"

# Do testing with reo-cli
cargo run -p reo-cli -- keys 'gg'  # Default: raw_ansi (colored output)
cargo run -p reo-cli -- keys --format plain_text 'gg'  # Plain text output

# Kill ONLY the server you just started
cargo run -p reo-cli -- --tcp 127.0.0.1:12521 kill
```

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
cargo run -p reo-cli -- keys 'iHello<Esc>'  # Inject keys, show status (colored output by default)
cargo run -p reo-cli -- --tcp 127.0.0.1:12522 keys 'j'  # Connect to specific server
cargo run -p reo-cli -- keys --format plain_text 'gg'  # Plain text output
cargo run -p reo-cli -- keys --format cell_grid 'gg'  # JSON cell grid
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
reovim --lsp-log=default myfile.rs              # Timestamped lsp-*.log in data dir (trace level)
reovim --lsp-log=/tmp/lsp.log myfile.rs         # Custom path (trace level)
reovim --lsp-log=default:debug myfile.rs        # Default path with debug level
reovim --lsp-log=/tmp/lsp.log:info myfile.rs    # Custom path with info level
# LSP log levels: error, warn, info, debug, trace (default: trace)
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

Reovim is a Rust-based neovim-like text editor following a **Linux kernel-inspired architecture** with clear separation between kernel mechanisms, drivers, and loadable modules.

### Design Principles

**Mechanism vs Policy**: The kernel provides WHAT can be done (syscalls, traits), modules decide HOW to do it (keybindings, behavior). See [mechanism-vs-policy.md](./docs/architecture/mechanism-vs-policy.md).

**Kernel Purity**: Zero external syntax dependencies in kernel. All tree-sitter lives in plugins, not kernel.

**API Boundary**: Modules use ONLY `reovim_kernel::api::*`. Kernel internals are `pub(crate)` (compile-time enforced).

### Workspace Structure

**New Architecture (v0.9.0+):**
- `lib/kernel/` (reovim-kernel) - Core mechanisms: mm/, ipc/, core/, block/, sched/, api/
- `lib/drivers/` - Service adapters: syntax/, input/, display/, lsp/, net/, vfs/, log/
- `lib/arch/` (reovim-arch) - Platform abstraction: unix/, windows/
- `lib/module-macros/` - `declare_module!` proc-macro for FFI entry points
- `runner/src/module/` - Module loader, registry, hot reload

**Legacy (v0.8.x, being migrated):**
- `lib/core/` (reovim-core) - Core editor logic: runtime, buffers, events, screen rendering
- `lib/sys/` (reovim-sys) - Re-exports crossterm for terminal abstraction

**Plugins and Tools:**
- `plugins/features/` - Feature plugins (range-finder, treesitter, completion, explorer, telescope)
- `plugins/languages/` - Language support plugins (rust, c, javascript, python, json, toml, markdown)
- `modules/` - Policy modules (keymap, motions, operators, layout, options)
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
- `ScopedKeyEvent` - Key event with optional `EventScope` for lifecycle tracking

**EventScope** (`lib/core/src/event_bus/scope.rs`) - GC-like tracking for deterministic event synchronization:
- `EventScope::new()` - Create scope with counter = 0
- `scope.increment()` - Track new event (counter++)
- `scope.decrement()` - Mark event complete (counter--), notify if zero
- `scope.wait()` - Async wait for counter to reach 0
- `scope.wait_timeout(duration)` - Wait with timeout, returns `false` if timed out
- Debug with `REOVIM_LOG=trace` to see scope lifecycle
- Used by RPC `input/keys` to wait for all key effects to complete

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

**Feature Plugins**:
- `plugins/features/range-finder/` - Jump navigation (s/f/F/t/T) and code folding (za/zo/zc/zR/zM)
- `plugins/features/telescope/` - Fuzzy finder (Space f)
- `plugins/features/explorer/` - File browser (Space e)
- `plugins/features/completion/` - Async completion (Ctrl-Space)
- `lib/core/src/modd/` - Multi-dimensional mode state (Focus, EditMode, SubMode)

**Plugin System** (`lib/core/src/plugin/`):
- `Plugin` trait - Interface for all plugins
- `PluginStateRegistry` - Shared state between plugins
- `EventBus` - Publish/subscribe event system for plugin communication
- Plugins are loaded by `runner/src/plugins.rs`

**Unified Command-Event Pattern** (v0.6.22+, legacy):
- Single type serves as both CommandTrait and Event implementation
- Macros: `declare_event_command!` (zero-sized), `declare_counted_event_command!` (with count)
- Benefits: 50% fewer types, ~20 lines less boilerplate per command
- All feature plugins migrated (Fold, Settings, Completion, Explorer, Telescope)
- Example: `ExplorerRefresh` (not `ExplorerRefreshCommand` + `ExplorerRefreshEvent`)
- Note: Will be superseded by the new Module system (v0.9.0+) with `declare_module!` macro
- See: `docs/reference/commands.md`, `docs/archive/plugins/system.md`

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

## Release Process


### Git Commits

**IMPORTANT**: Claude should NEVER handle git commits. The user will manage ALL git operations. Claude must not have any "opinion" about when or how to commit - only create proposals and let the user decide. And Never add emoji on below

**Changelog Convention**: Every feature, fix, or notable change MUST include an update to `CHANGELOG.md` under the `[Unreleased]` section before proposing a commit.

For any changes, Claude should create a proposal file at `tmp/<feature-name>-commit.md` with:
- Proposed commit message
- List of files changed (including `CHANGELOG.md`)
- Verification checklist
- Git commands for user to run

### Pull Requests

When creating PRs with `gh pr create`, do NOT add promotional footers like "Generated with Claude Code" or similar. Keep the PR body clean and professional with only relevant technical content.

### Ready-to-Take-Off & Request-for-Landing

**Ready-to-Take-Off (Start of Work):**
1. Use `gh issue` for view issues or Read `tmp/*` for context from previous sessions
2. Check `git worktree list` for active branches
3. Check `git status` for uncommitted work
4. Understand the task and plan implementation

**Request-for-Landing (End of Work):**
1. **Sync with upstream (CRUCIAL)**:
   ```bash
   git stash && git fetch origin develop && git rebase origin/develop
   ```
   - Save work with `git stash` first to avoid rebase conflicts with unstaged changes
   - **Check upstream changes carefully**: Review what changed with `git log HEAD..origin/develop` and `git diff HEAD..origin/develop` before rebasing
   - **Use gh cli to check context**: Check recent PRs/issues with `gh pr list`, `gh issue list`, or `gh pr view <number>` to understand upstream changes
   - Carefully rebase - resolve any conflicts before landing
   - Contributors own their conflicts
   - Restore with `git stash pop` after successful rebase

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

**Architecture (v0.9.0+ kernel-based):**
- [docs/architecture/overview.md](./docs/architecture/overview.md) - Layer diagram, Linux mapping, crate deps
- [docs/architecture/kernel.md](./docs/architecture/kernel.md) - Kernel subsystems: mm/, ipc/, core/, block/, sched/
- [docs/architecture/drivers.md](./docs/architecture/drivers.md) - Driver layer: syntax/, input/, display/, lsp/, net/
- [docs/architecture/modules.md](./docs/architecture/modules.md) - Module trait, declare_module!, loader, registry
- [docs/architecture/mechanism-vs-policy.md](./docs/architecture/mechanism-vs-policy.md) - Core design principle
- [docs/architecture/module-mode-inheritance.md](./docs/architecture/module-mode-inheritance.md) - Mode system with inheritance

**Guides and Reference:**
- [docs/guides/development.md](./docs/guides/development.md) - Development guide
- [docs/guides/testing.md](./docs/guides/testing.md) - Testing guide
- [docs/guides/configuration.md](./docs/guides/configuration.md) - Editor settings
- [docs/reference/commands.md](./docs/reference/commands.md) - Command system
- [docs/reference/server-mode.md](./docs/reference/server-mode.md) - RPC server mode

**Archive (v0.8.x legacy, being phased out):**
- [docs/archive/](./docs/archive/) - Legacy documentation for lib/core
