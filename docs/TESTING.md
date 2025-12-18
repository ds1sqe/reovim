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

## Integration Testing

Reovim includes an end-to-end integration test system that verifies key input → expected output behavior without requiring a real terminal.

### Architecture

```
TestRuntime
├── MockKeySource (injected key events)
├── MockOutput (captured screen output)
└── Runtime (full editor runtime)
         ↓
    KeyEventBroker → CommandHandler → Runtime → Screen → MockOutput
         ↑                                              ↓
    MockKeySource                              Assertions on TestResult
```

### Running Integration Tests

```bash
# All integration tests
cargo test -p reovim-core --test basic_editing --test mode_switching

# Specific test file
cargo test -p reovim-core --test basic_editing

# Single test
cargo test -p reovim-core --test mode_switching test_visual_mode
```

### Writing Integration Tests

#### Basic Structure

```rust
mod common;
use common::*;

#[tokio::test]
async fn test_example() {
    let rt = TestRuntime::builder()
        .with_size(80, 24)           // Screen dimensions
        .with_content("hello")       // Initial buffer content
        .with_keys(keys_from_str("jj")) // Key sequence
        .with_timeout_ms(5000)       // Test timeout
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_cursor(0, 2);
}
```

#### Key Notation (`keys_from_str`)

| Notation | Key Event |
|----------|-----------|
| `a`-`z`, `0`-`9` | Character keys |
| `<Esc>` | Escape key |
| `<CR>` or `<Enter>` | Enter key |
| `<BS>` | Backspace |
| `<Tab>` | Tab |
| `<Space>` | Space |
| `<C-x>` | Ctrl+X |
| `<S-x>` | Shift+X |
| `<Up>`, `<Down>`, `<Left>`, `<Right>` | Arrow keys |
| `<Home>`, `<End>` | Navigation keys |
| `<PageUp>`, `<PageDown>` | Page keys |

**Examples:**
```rust
keys_from_str("ihello<Esc>")    // Enter insert, type "hello", escape
keys_from_str(":wq<CR>")        // Command mode, type "wq", enter
keys_from_str("<C-d>")          // Ctrl+D
keys_from_str("5j")             // Move down 5 lines
keys_from_str("daw")            // Delete a word
```

#### TestResult Assertions

| Method | Description |
|--------|-------------|
| `assert_no_timeout()` | Test completed without timeout |
| `assert_normal_mode()` | Editor is in normal mode |
| `assert_insert_mode()` | Editor is in insert mode |
| `assert_mode(&ModeState)` | Check specific mode state |
| `assert_cursor(x, y)` | Cursor at position |
| `assert_buffer_contains("text")` | Buffer contains substring |
| `assert_buffer_eq("text")` | Buffer equals exactly |
| `assert_output_contains("text")` | Screen output contains (ANSI stripped) |

#### Accessing TestResult Fields

```rust
let result = rt.run().await;

// Direct field access
println!("Mode: {:?}", result.mode);
println!("Buffer: {}", result.buffer_content);
println!("Cursor: {:?}", result.cursor_position);
println!("Timed out: {}", result.timed_out);

// Screen output (with ANSI stripped)
let screen_text = result.output.strip_ansi();
```

### Integration Test Organization

```
lib/core/tests/
├── common/
│   └── mod.rs              # Shared utilities (standard_runtime, etc.)
├── basic_editing.rs        # Insert, delete, cursor movement
└── mode_switching.rs       # Mode transitions (i, a, v, :, Esc)
```

### Shared Test Utilities (`common/mod.rs`)

```rust
pub use reovim_core::runtime::test::{TestRuntime, TestRuntimeBuilder};
pub use reovim_core::testing::keys_from_str;

/// Standard 80x24 runtime with default settings.
pub fn standard_runtime() -> TestRuntimeBuilder {
    TestRuntime::builder().with_size(80, 24)
}

/// Runtime with initial buffer content.
pub fn runtime_with_content(content: &str) -> TestRuntimeBuilder {
    standard_runtime().with_content(content)
}
```

### Best Practices

1. **Use `assert_no_timeout()` first** - Ensures test completed before checking state
2. **Keep key sequences short** - Long sequences are harder to debug
3. **Test one behavior per test** - Makes failures easier to diagnose
4. **Use `runtime_with_content()` for cursor tests** - Need text to move through
5. **Avoid timing-dependent assertions** - Use mode/buffer state, not output timing

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
