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

| Module | Coverage Area |
|--------|--------------|
| `buffer` | Text operations, cursor movement, selection |
| `count_parser` | Numeric prefix parsing (5j, 10w, etc.) |
| `key_parser` | Key sequence parsing |
| `highlight` | Highlight spans and store operations |
| `types` | Core data types |

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
