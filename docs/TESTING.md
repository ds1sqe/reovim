# Testing Guide

This guide covers running and writing tests for reovim.

## Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p reovim-core

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run tests matching a pattern
cargo test buffer
```

## Test Organization

Tests are organized as inline module tests within source files:

```
lib/core/src/
├── buffer/
│   └── tests.rs           # Buffer operations tests
├── event/handler/command/
│   ├── count_parser.rs    # Count parsing tests
│   └── key_parser.rs      # Key parsing tests
├── highlight/
│   ├── store.rs           # Highlight store tests
│   └── span.rs            # Highlight span tests
└── types.rs               # Core type tests
```

## Current Test Coverage

| Module | Coverage Area | Tests |
|--------|--------------|-------|
| `buffer` | Text operations, cursor movement, selection | 25 |
| `completion` | Filter, item, source, state, trigger | 20 |
| `count_parser` | Numeric prefix parsing (5j, 10w, etc.) | 6 |
| `explorer` | Node, render, state, tree | 15 |
| `highlight` | Color, span, store, theme | 18 |
| `jumplist` | Jump navigation | 4 |
| `leap` | Two-character jump labels | 3 |
| `screen/layout` | Layout calculations | 4 |
| `telescope` | Item, matcher, state | 11 |
| `types` | Core data types | 4 |
| `folding` | Fold state, toggle, markers | 4 |

**Total: 133 tests**

## Writing Tests

### Test Module Pattern

Tests are placed in a `tests` submodule within the source file:

```rust
// In lib/core/src/buffer/mod.rs
#[cfg(test)]
mod tests;

// In lib/core/src/buffer/tests.rs
use super::*;

#[test]
fn test_buffer_insert() {
    let mut buffer = Buffer::new(0);
    buffer.insert_char('a');
    assert_eq!(buffer.contents[0].content, "a");
}
```

### Test Naming

- Use descriptive names: `test_cursor_moves_down_by_count`
- Prefix with `test_`
- Group related tests in the same file

### Testing Buffer Operations

```rust
#[test]
fn test_delete_selection() {
    let mut buffer = Buffer::from_content(0, "hello world");
    buffer.start_selection();
    buffer.cur = Position { row: 0, col: 5 };
    buffer.delete_selection();
    assert_eq!(buffer.contents[0].content, " world");
}
```

### Testing Event Handlers

Event handlers can be tested by simulating key events:

```rust
#[test]
fn test_count_parser() {
    let result = parse_count("5j");
    assert_eq!(result.count, Some(5));
    assert_eq!(result.remaining, "j");
}
```

## Test Best Practices

1. **Isolate tests**: Each test should be independent
2. **Test edge cases**: Empty buffers, single characters, large files
3. **Use descriptive assertions**: `assert_eq!` over `assert!`
4. **Keep tests fast**: Avoid I/O in unit tests

## Performance Benchmarks

Reovim uses Criterion for performance benchmarking.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p reovim-core

# Run specific benchmark group
cargo bench -p reovim-core -- window_render
cargo bench -p reovim-core -- stress
cargo bench -p reovim-core -- buffer_clone

# List available benchmarks
cargo run -p perf-report -- list
```

### Benchmark Categories

| Category | Benchmarks | Description |
|----------|------------|-------------|
| `window_render` | 4 | Window::render() with various buffer sizes |
| `viewport_size` | 4 | Different viewport heights |
| `screen_io` | 3 | Full screen I/O with mock writer |
| `screen_viewport_io` | 3 | Viewport with I/O |
| `file_io` | 2 | Real file I/O (buffered vs unbuffered) |
| `file_io_viewport` | 3 | File I/O at different viewports |
| `input_typing` | 2 | Single char and burst typing |
| `input_scrolling` | 3 | Line, 10-line, half-page scroll |
| `input_mode_switch` | 1 | Normal/Insert/Normal cycle |
| `input_completion` | 2 | With/without completion popup |
| `input_sustained` | 1 | 100 keystrokes with render |
| `rtt_explorer` | 3 | Open, close, toggle explorer |
| `rtt_input_lag` | 2 | Char insert, backspace RTT |
| `rtt_movement_lag` | 5 | Movement operations RTT |
| `stress_editing` | 3 | Edit/navigate cycles (1k, 10k, 50k) |
| `stress_scroll` | 3 | Rapid scrolling tests |
| `stress_mode_ops` | 1 | Insert/escape/move cycles |
| `stress_completion` | 1 | Completion scroll 100 items |
| `stress_worst_case` | 1 | All features, 50k file |
| `buffer_clone` | 4 | Clone overhead by size |
| `buffer_vec` | 4 | Vec<Buffer> creation |

**Total: 39 benchmarks**

### Generating Performance Reports

```bash
# Generate report for current version
cargo run -p perf-report -- update --version X.Y.Z

# Check for regressions
cargo run -p perf-report -- check

# Compare versions
cargo run -p perf-report -- compare OLD_VER NEW_VER
```

Reports are stored in `perf/PERF-{version}.md`.

### Recent Performance Results (v0.4.2)

Key improvements over v0.3.0 baseline:

| Benchmark | Improvement |
|-----------|-------------|
| window_render | 50-62% faster |
| viewport_size | 68-71% faster |
| screen_io | 66-79% faster |
| file_io | 79% faster |
| rtt_movement_lag | 64-91% faster |
| stress_editing | 54-65% faster |
| throughput | 71% faster |

See `perf/PERF-0.4.2.md` for detailed results.

### Benchmark Location

```
lib/core/benches/
├── render.rs              # Main entry point
└── bench_modules/
    ├── common.rs          # Shared utilities
    ├── window.rs          # Window render benchmarks
    ├── screen.rs          # Screen I/O benchmarks
    ├── input.rs           # Input simulation
    ├── rtt.rs             # Round-trip time
    └── stress.rs          # Stress tests
```

## Continuous Integration

Before submitting a PR:

```bash
cargo test           # All tests must pass
cargo clippy         # No warnings allowed
cargo fmt -- --check # Code must be formatted
```

## Related Documentation

- [Development](./DEVELOPMENT.md) - Build and code standards
- [Commands](./commands.md) - Command system (testable units)
