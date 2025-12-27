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

```bash
# Build all crates
cargo build

# Build release
cargo build --release

# Run the editor
cargo run -p reovim

# Run with a file
cargo run -p reovim -- path/to/file.txt

# Run in server mode (TCP on 127.0.0.1:12521)
cargo run -p reovim -- --server

# Run server with stdio transport
cargo run -p reovim -- --stdio

# Run server on Unix socket
cargo run -p reovim -- --listen-socket /tmp/reovim.sock

# Run reo-cli client
cargo run -p reo-cli -- mode
cargo run -p reo-cli -- keys 'iHello<Esc>'

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy
```

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

### Plugin Decoupling

- Never add plugin-specific code to core
- If API is insufficient, propose an extension (see [plugin-system.md](./plugin-system.md))
- Plugins must be fully self-contained

## Project Structure

```
reovim/
├── runner/                 # Binary crate - entry point, plugin configuration
│   └── src/plugins.rs      # AllPlugins - combines DefaultPlugins + external plugins
├── lib/
│   ├── core/               # reovim-core - core editor logic, plugin system
│   └── sys/                # reovim-sys - terminal abstraction
├── plugins/
│   ├── features/           # External feature plugins
│   │   ├── fold/           # Code folding
│   │   ├── settings-menu/  # In-editor settings
│   │   ├── completion/     # Text completion
│   │   ├── explorer/       # File browser
│   │   ├── telescope/      # Fuzzy finder
│   │   └── treesitter/     # Syntax highlighting infrastructure
│   └── languages/          # Language plugins
│       ├── rust/           # Rust support
│       ├── c/              # C support
│       ├── javascript/     # JavaScript support
│       ├── python/         # Python support
│       ├── json/           # JSON support
│       ├── toml/           # TOML support
│       └── markdown/       # Markdown support
├── tools/
│   ├── perf-report/        # Performance report generator
│   ├── reo-cli/            # CLI client for server mode
│   └── bench/              # Performance benchmarks (criterion)
└── perf/                   # Versioned performance reports
```

### Plugin Architecture

- **DefaultPlugins** (in `lib/core`): Built-in plugins shipped with core
- **AllPlugins** (in `runner`): Combines DefaultPlugins + external plugins
- External plugins depend on `reovim-core` and register commands

For detailed architecture, see [architecture.md](./architecture.md).

## Debugging

### Enable Backtraces

```bash
RUST_BACKTRACE=1 cargo run -p reovim
RUST_BACKTRACE=full cargo run -p reovim  # Full backtrace
```

### Debugging Tips

- Use `dbg!()` macro for quick value inspection
- Check `lib/core/src/runtime/mod.rs` for event loop debugging
- Event flow: InputEventBroker → KeyEventBroker → Handlers → Runtime

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

### Current Performance (v0.6.0 vs v0.5.0)

| Operation | v0.5.0 | v0.6.0 | Change |
|-----------|--------|--------|--------|
| Window render (10 lines) | 1.63 µs | 10.15 µs | +6.2x (frame buffer overhead) |
| Window render (10k lines) | 6.91 µs | 56.08 µs | +8.1x (frame buffer overhead) |
| Full screen render | 14.50 µs | 62.41 µs | +4.3x (diff calculation) |
| Char insert RTT | 55.58 µs | 108.17 µs | +1.9x |
| Word forward RTT | 89.31 µs | 135.91 µs | +1.5x |
| Move down RTT | 751.31 µs | 806.35 µs | +7% |
| Mode switch | 46.85 µs | 186.77 µs | +4x |
| Throughput | ~144k/sec | ~18k/sec | -87% (expected) |

**v0.6.0 Trade-offs**: The frame buffer architecture adds per-render overhead but provides:
- **Zero flickering** - Only changed cells sent to terminal
- **Consistent performance** - Buffer size doesn't affect I/O
- **Composable layers** - Clean separation of UI components
- **Future optimization potential** - Dirty region tracking ready

### Latency Goals

- Key press to screen update: < 16ms (60fps) - **Achieved: <1ms**
- File operations: async, non-blocking
- Rendering: incremental when possible

## Related Documentation

- [Architecture](./architecture.md) - System design overview
- [Event System](./event-system.md) - Input handling and event flow
- [Commands](./commands.md) - Command system and execution
- [Testing](./TESTING.md) - Running and writing tests
