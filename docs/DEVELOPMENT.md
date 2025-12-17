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

## Project Structure

```
reovim/
├── main/              # Binary crate - entry point
├── lib/core/          # reovim-core - core editor logic
│   └── benches/       # Performance benchmarks
├── lib/sys/           # reovim-sys - terminal abstraction
├── tools/perf-report/ # Performance report generator
└── perf/              # Versioned performance reports
```

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
cargo bench -p reovim-core

# Run specific benchmark group
cargo bench -p reovim-core -- window_render

# Generate performance report
cargo run -p perf-report -- update --version X.Y.Z

# Compare versions
cargo run -p perf-report -- compare 0.3.0 0.4.2
```

### Current Performance (v0.4.2)

| Operation | Latency |
|-----------|---------|
| Window render (10 lines) | 639 ns |
| Window render (10k lines) | 2.48 µs |
| Full screen render | 5.91 µs |
| Character insert RTT | 34 µs |
| Movement RTT | 397-410 µs |
| Throughput | ~400k renders/sec |

### Latency Goals

- Key press to screen update: < 16ms (60fps) - **Achieved: <1ms**
- File operations: async, non-blocking
- Rendering: incremental when possible

## Related Documentation

- [Architecture](./architecture.md) - System design overview
- [Event System](./event-system.md) - Input handling and event flow
- [Commands](./commands.md) - Command system and execution
- [Testing](./TESTING.md) - Running and writing tests
