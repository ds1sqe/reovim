# Development Guide

This guide covers setting up your development environment and contributing to reovim.

## Prerequisites

| Requirement | Version |
|-------------|---------|
| Rust | 1.92+ (2024 edition) |
| cargo | Latest stable |
| rustfmt | Latest stable |
| clippy | Latest stable |

## Build Commands

Essential commands for development:

```bash
# Build all crates
cargo build

# Run the editor
cargo run

# Run tests
cargo test

# Format and lint
cargo fmt && cargo clippy
```

For the full command reference including server mode, CLI client, benchmarks, and debugging options, see [CLAUDE.md](../../CLAUDE.md#build-commands).

## Code Standards

### Zero-Warning Policy

This project enforces a **zero-warning policy**. All code must compile without any warnings from:

```bash
cargo build   # Must produce zero warnings
cargo clippy  # Must produce zero warnings
```

No warnings are acceptable. This is non-negotiable.

Before committing:
1. Run `cargo build` - verify zero warnings
2. Run `cargo clippy` - verify zero warnings
3. Run `cargo fmt` - ensure consistent formatting

### Code Quality

- Follow existing code patterns
- Keep functions focused and small
- Prefer clarity over cleverness
- Avoid unnecessary abstractions

### Module Decoupling

- Never add module-specific code to kernel
- If API is insufficient, propose an extension (see [module-development.md](./guides/module-development.md))
- Modules must be fully self-contained

## Project Structure (Phase 8)

The project follows a server/client architecture:

```
reovim/
├── apps/bin/              # Main binary entry point (reovim-app)
├── server/                # Server-side components
│   ├── lib/
│   │   ├── kernel/        # reovim-kernel - core mechanisms (mm/, ipc/, core/)
│   │   ├── server/        # reovim-server - gRPC handlers, session management
│   │   └── drivers/       # Server drivers (27 crates)
│   │       ├── command/       # Command trait and registry
│   │       ├── input/         # Key events and input parsing
│   │       ├── syntax/        # Tree-sitter integration
│   │       ├── lsp/           # Language server protocol
│   │       ├── vfs/           # Virtual filesystem
│   │       ├── session/       # Session management
│   │       ├── buffer/        # Buffer operations
│   │       └── ...            # (undo, search, clipboard, ffi, etc.)
│   └── modules/           # Policy modules (73 loadable modules)
│       ├── vim/               # Core Vim-like behavior
│       ├── editor/            # Editor core operations
│       ├── motions/           # Movement commands
│       ├── textobjects/       # Text object definitions
│       ├── keymap/            # Keymap definitions
│       ├── mode-manager/      # Mode system
│       └── ...                # (options, buffer-ops, clipboard, search, etc.)
├── clients/               # Client applications
│   ├── lib/               # Platform-agnostic shared client infra
│   │   ├── driver/        # reovim-client-driver
│   │   └── model/         # reovim-client-model
│   ├── cli/               # CLI client core (gRPC v3)
│   ├── tui/               # TUI client core (gRPC v3)
│   └── web/               # Web client (gRPC-Web, WASM)
├── ext/                   # Extension tree (pluggable)
│   ├── server/
│   │   ├── domain/text/   # Concrete text domain
│   │   ├── drivers/       # 19 driver implementations
│   │   ├── modules/       # 73 policy modules
│   │   └── providers/text/ # reovim-provider-text
│   └── client/tui/
│       ├── drivers/       # TUI platform drivers (tui, display)
│       └── modules/       # 20 TUI client modules
├── arch/                  # Platform abstraction (Linux-inspired)
├── lib/                   # Core libraries (zero reovim-specific policy)
│   ├── bench/             # reovim-bench-utils
│   ├── depgraph/          # reovim-depgraph + layer tests
│   └── domain/            # reovim-domain: generic Domain trait
├── uapi/                  # Stable wire contracts
│   ├── protocol/          # gRPC v3 proto definitions
│   ├── render-codec/      # Render surface wire format
│   ├── input-codec/       # InputEvent wire format (TEMP, Plan 15)
│   └── module-macros/     # declare_module! proc-macro
├── tools/                 # Dev-time tooling (composition-root-like)
│   ├── bench/             # Criterion benchmark suite
│   ├── perf-report/       # Performance report generator
│   ├── python-test-sdk/   # Python test SDK
│   └── testing/           # reovim-testing harness
└── perf/                  # Versioned performance reports
```

### Module Architecture

- **Kernel** (`server/lib/kernel/`): Core mechanisms, policy-agnostic
- **Drivers** (`ext/server/drivers/`): Service providers with trait contracts
- **Modules** (`ext/server/modules/`): Policy implementations (vim behavior, keymaps, etc.)
- **Server** (`server/lib/server/`): gRPC handlers, session management, module registry

Modules implement traits from kernel/drivers and are loaded dynamically.

For detailed architecture, see [architecture overview](../architecture/overview.md).

## Debugging

### Enable Backtraces

```bash
RUST_BACKTRACE=1 cargo run -p reovim
RUST_BACKTRACE=full cargo run -p reovim  # Full backtrace
```

### Debugging Tips

- Use `dbg!()` macro for quick value inspection
- Check `server/lib/server/src/` for server-side event loop debugging
- Event flow: gRPC Input → Key resolver → Module handlers → State changes

### LSP Debugging

When troubleshooting LSP integration issues (rust-analyzer, etc.):

> **Note:** LSP logging flags (`--lsp-log`) and `:LspLog` command are planned features, not yet implemented.

#### Enable LSP Logging

```bash
# Log to timestamped file in data directory
reovim --lsp-log=default myfile.rs

# Log to custom file
reovim --lsp-log=/tmp/lsp.log myfile.rs

# Log with specific level (error, warn, info, debug, trace)
reovim --lsp-log=default:debug myfile.rs
reovim --lsp-log=/tmp/lsp.log:info myfile.rs
```

Log format shows direction (`->` outgoing, `<-` incoming) and JSON-RPC messages:
```
[2026-01-06T12:00:00Z] -> initialize { ... }
[2026-01-06T12:00:01Z] <- initialized { ... }
[2026-01-06T12:00:02Z] -> textDocument/didOpen { ... }
```

#### View LSP Log in Editor

```vim
:LspLog
```

This opens the current LSP log file in a new buffer.

#### LSP Health Check

```vim
:health
```

Navigate to the "LSP" section to see:
- Server status (running/stopped)
- Document count
- Diagnostic statistics
- Timestamps for initialization and last activity

#### Common LSP Issues

**Diagnostics not appearing:**
- rust-analyzer uses LSP 3.17 "pull diagnostics"
- Reovim requests diagnostics after `textDocument/didOpen`
- Check `:health` to verify server is running

**Server not starting:**
- Verify LSP server is installed (`which rust-analyzer`)
- Check LSP log for error messages
- Ensure file is in a project directory (has `Cargo.toml` for Rust)

**Slow completions:**
- LSP completions are async and debounced
- Large projects may have slower initial indexing
- Check LSP log for timeout messages

## Performance Considerations

Reovim prioritizes **minimal latency**:

- Keep the main event loop fast
- Avoid blocking operations in handlers
- Use async I/O for all terminal operations
- Profile with `cargo flamegraph` for hot paths

### Benchmarking

```bash
# Run all benchmarks
cargo bench -p reovim-bench

# Run specific benchmark group
cargo bench -p reovim-bench -- window_render

# Generate performance report
cargo run -p perf-report -- update --version X.Y.Z

# Compare versions
cargo run -p perf-report -- compare 0.3.0 0.4.2
```

### Current Performance (v0.7.10 vs v0.6.0)

> **Note:** These benchmarks are from an early version. See `perf/` directory for current performance reports.

| Metric | v0.6.0 | v0.7.10 | Change |
|--------|--------|---------|--------|
| Window render (10 lines) | 10 µs | 5.3 µs | **-47%** |
| Window render (10k lines) | 56 µs | 26 µs | **-54%** |
| Full scroll cycle | 85 µs | 55 µs | **-35%** |
| Large file (5k lines) | 174 µs | 87 µs | **-50%** |
| Throughput | 18k/sec | 38k/sec | **+111%** |

**v0.7.x Improvements**: Optimized render pipeline while maintaining frame buffer benefits:
- **Zero flickering** - Only changed cells sent to terminal
- **2x faster rendering** - Optimized pipeline stages
- **Composable layers** - Clean separation of UI components
- **Saturator architecture** - Background async computation

See `perf/` directory for detailed versioned performance reports.

### Latency Goals

- Key press to screen update: < 16ms (60fps) - **Achieved: <1ms**
- File operations: async, non-blocking
- Rendering: incremental when possible

## Related Documentation

- [Architecture](../architecture/overview.md) - System design overview
- [Commands](../user-guide/commands.md) - Command system and execution
- [Testing](./guides/testing.md) - Running and writing tests
