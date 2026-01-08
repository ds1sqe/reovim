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

Reovim includes an end-to-end integration test system that uses the server mode (`--server --test`) to verify key input → expected output behavior by spawning a real server process.

### Architecture

```
ServerTestHarness
├── Spawns: reovim --server --test --listen-tcp <port>
├── TestClient (TCP connection via JSON-RPC)
└── Auto-cleanup on Drop

Test Flow:
┌─────────────┐     JSON-RPC      ┌─────────────────────────┐
│ TestClient  │ ────────────────→ │ reovim --server --test  │
│             │                   │                         │
│ keys()      │  input/keys       │  ChannelKeySource       │
│ mode()      │  state/mode       │  Runtime                │
│ cursor()    │  state/cursor     │  Buffer, Screen         │
│ buffer()    │  buffer/content   │                         │
│ windows()   │  state/windows    │  Screen (scroll/cursor) │
└─────────────┘ ←──────────────── └─────────────────────────┘
                   Response
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

**Note:** Integration tests require the release binary. Run `cargo build --release` before running integration tests.

### Writing Integration Tests

#### Basic Structure

```rust
mod common;
use common::*;

#[tokio::test]
async fn test_example() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")  // Initial buffer content
        .with_keys("jj")               // Key sequence (vim notation)
        .run()
        .await;

    result.assert_cursor(0, 2);
    result.assert_normal_mode();
}
```

#### Key Notation

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
.with_keys("ihello<Esc>")    // Enter insert, type "hello", escape
.with_keys(":wq<CR>")        // Command mode, type "wq", enter
.with_keys("<C-d>")          // Ctrl+D
.with_keys("5j")             // Move down 5 lines
.with_keys("daw")            // Delete a word
```

#### ServerTestResult Assertions

| Method | Description |
|--------|-------------|
| `assert_normal_mode()` | Editor is in normal mode |
| `assert_insert_mode()` | Editor is in insert mode |
| `assert_visual_mode()` | Editor is in visual mode |
| `assert_command_mode()` | Editor is in command mode |
| `assert_cursor(x, y)` | Cursor at position |
| `assert_buffer_contains("text")` | Buffer contains substring |
| `assert_buffer_eq("text")` | Buffer equals exactly |

#### Accessing ServerTestResult Fields

```rust
let result = ServerTest::new()
    .await
    .with_keys("ihello<Esc>")
    .run()
    .await;

// Direct field access
println!("Mode: {:?}", result.mode);
println!("Buffer: {}", result.buffer_content);
println!("Cursor: {:?}", result.cursor);
```

### Integration Test Organization

```
lib/core/tests/
├── common/
│   └── mod.rs              # Re-exports ServerTest
├── basic_editing.rs        # Insert, delete, cursor movement
├── mode_switching.rs       # Mode transitions (i, a, v, :, Esc)
├── resize.rs               # Layout and explorer resize tests
├── visual_snapshot.rs      # Visual debugging and assertions
└── which_key.rs            # Which-key popup tests
```

### Test Harness Components

#### ServerTestHarness (`lib/core/src/testing/server.rs`)

Spawns and manages a reovim server process:

```rust
// Automatically spawns server on OS-assigned port
let harness = ServerTestHarness::spawn().await?;

// Get a connected client
let client = harness.client().await?;

// Server is killed when harness is dropped (kill_on_drop)
```

#### TestClient (`lib/core/src/testing/client.rs`)

JSON-RPC client for testing:

```rust
let mut client = TestClient::connect("127.0.0.1", port).await?;

client.keys("ihello<Esc>").await?;   // Inject keys
let mode = client.mode().await?;      // Get mode
let cursor = client.cursor().await?;  // Get cursor (x, y)
let content = client.buffer_content().await?;  // Get buffer
client.set_buffer_content("text").await?;      // Set buffer
client.resize(100, 40).await?;        // Resize editor
client.kill().await?;                 // Kill server
```

#### ServerTest (`lib/core/src/testing/assertions.rs`)

Fluent builder for tests:

```rust
ServerTest::new()
    .await
    .with_content("initial text")  // Optional: set initial buffer
    .with_size(80, 24)             // Optional: set screen dimensions
    .with_keys("dd")               // Optional: inject keys (can chain)
    .with_keys("p")
    .run()
    .await
```

### Visual Testing

Reovim includes visual debugging infrastructure for testing TUI output. This enables:
- ASCII art snapshots for human-readable debugging
- Structured visual snapshots (JSON) for programmatic assertions
- Layer visibility testing
- Cell-level content verification

#### Screen Size Configuration

Set explicit screen dimensions for deterministic visual tests:

```rust
let mut result = ServerTest::new()
    .await
    .with_size(80, 24)  // Width x Height
    .with_content("Hello World")
    .run()
    .await;

let snap = result.visual_snapshot().await;
assert_eq!(snap.width, 80);
assert_eq!(snap.height, 24);
```

#### Visual Snapshot Methods

| Method | Description |
|--------|-------------|
| `visual_snapshot()` | Get structured snapshot with cells, cursor, layers |
| `ascii_art(false)` | Get plain ASCII representation |
| `ascii_art(true)` | Get annotated ASCII with borders and line numbers |
| `layer_info()` | Get layer visibility and bounds |

**Example: Visual Snapshot**

```rust
let mut result = ServerTest::new()
    .await
    .with_size(40, 10)
    .with_content("Hello World")
    .run()
    .await;

let snap = result.visual_snapshot().await;

// Check dimensions
assert_eq!(snap.cells.len(), 10);  // 10 rows
assert_eq!(snap.cells[0].len(), 40);  // 40 columns each

// Check cursor
if let Some(cursor) = &snap.cursor {
    println!("Cursor at ({}, {})", cursor.x, cursor.y);
}

// Check plain text content
assert!(snap.plain_text.contains("Hello World"));
```

**Example: ASCII Art Debugging**

```rust
let mut result = ServerTest::new()
    .await
    .with_size(40, 10)
    .with_content("fn main() {\n    println!(\"Hello\");\n}")
    .run()
    .await;

// Plain ASCII
let plain = result.ascii_art(false).await;
println!("{}", plain);
// Output:
//   1 fn main() {
//   2     println!("Hello");
//   3 }
// ~
// ...

// Annotated ASCII (with borders and column numbers)
let annotated = result.ascii_art(true).await;
println!("{}", annotated);
// Output:
//    0123456789...
//   +----------------------------------------+
//  0|  1 fn main() {                         |
//  1|  2     println!("Hello");              |
//  2|  3 }                                   |
//   +----------------------------------------+
```

**Example: Layer Visibility Testing**

```rust
// Test that which-key layer appears after timeout
let mut result = ServerTest::new()
    .await
    .with_size(80, 24)
    .with_keys("g")
    .with_delay(600)  // Wait for which-key popup
    .run()
    .await;

let layers = result.layer_info().await;
assert!(layers.iter().any(|l| l.name == "which_key" && l.visible));

// Test telescope layer
let mut result = ServerTest::new()
    .await
    .with_size(80, 24)
    .with_keys(" ff")  // Open telescope
    .with_delay(100)
    .run()
    .await;

let layers = result.layer_info().await;
assert!(layers.iter().any(|l| l.name == "telescope" && l.visible));
```

#### Visual Assertions Trait

The `VisualAssertions` trait provides assertion methods on `VisualSnapshot`:

```rust
use reovim_core::testing::VisualAssertions;

let snap = result.visual_snapshot().await;

// Assert specific cell character
snap.assert_cell_char(0, 0, 'H');

// Assert row content (trimmed)
snap.assert_row_content(0, "  1 Hello World");

// Assert row starts with prefix
snap.assert_row_starts_with(0, "  1");

// Assert screen contains text
snap.assert_contains("Hello World");

// Assert region contains text
snap.assert_region_contains(0, 0, 20, 5, "Hello");
```

#### Full Screen Cell Verification

Verify that the screen has the expected dimensions:

```rust
let mut result = ServerTest::new()
    .await
    .with_size(40, 10)
    .run()
    .await;

let snap = result.visual_snapshot().await;

// Verify exact row count
assert_eq!(snap.cells.len(), 10, "Should have 10 rows");

// Verify each row has correct width
for (y, row) in snap.cells.iter().enumerate() {
    assert_eq!(row.len(), 40, "Row {} should have 40 cells", y);
}
```

#### VisualSnapshot Structure

```rust
pub struct VisualSnapshot {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Vec<CellSnapshot>>,  // cells[y][x]
    pub cursor: Option<CursorInfo>,
    pub layers: Vec<LayerInfo>,
    pub plain_text: String,
}

pub struct CellSnapshot {
    pub char: char,
    pub fg: Option<String>,   // "#RRGGBB"
    pub bg: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct CursorInfo {
    pub x: u16,
    pub y: u16,
    pub layer: String,  // "editor", "telescope", etc.
}

pub struct LayerInfo {
    pub name: String,
    pub z_order: u8,
    pub visible: bool,
    pub bounds: BoundsInfo,  // x, y, width, height
}
```

#### Visual Test Organization

```
lib/core/tests/
├── visual_snapshot.rs      # Visual testing tests
│   ├── Screen size tests   # Explicit dimensions, full screen cells
│   ├── Basic snapshot      # Content verification, ASCII art
│   ├── Layer tests         # Editor, telescope, which-key
│   └── Visual assertions   # Cell/row/content checks
└── ...
```

### Best Practices

1. **Build release first** - Tests spawn `./target/release/reovim`
2. **Keep key sequences short** - Long sequences are harder to debug
3. **Test one behavior per test** - Makes failures easier to diagnose
4. **Use `with_content()` for cursor tests** - Need text to move through
5. **Tests run in parallel** - Each spawns its own server on an OS-assigned port

### Port Allocation

Integration tests use **OS-assigned ports** (port 0) for collision-free parallel execution:

- **Test servers**: OS-assigned ephemeral ports (let the kernel pick an available port)
- **Regular servers**: Port 12521 by default, auto-increments to 12522, 12523, ... if in use

This approach ensures:
- **No port collisions** - OS guarantees port uniqueness
- **Parallel-safe** - Multiple test processes can run simultaneously
- **Cross-process safe** - Works correctly with `cargo test` parallelism
- Tests don't interfere with manually started debug servers
- `reo-cli list` only shows regular servers (not test instances)

### How It Works

1. `ServerTest::new()` spawns `reovim --server --test --listen-tcp 0`
2. Server binds to port 0, OS assigns an available port
3. Server prints `Listening on 127.0.0.1:<port>` to stderr
4. Test harness reads stderr to discover the actual port
5. `TestClient` connects via TCP and sends commands
6. Keys are injected with 1ms delay between each (for mode propagation)
7. After keys, 50ms delay allows processing before querying state
8. Server auto-exits when client disconnects (test mode)

## Current Test Coverage

| Module | Coverage Area | Tests |
|--------|--------------|-------|
| `buffer` | Text operations, cursor movement, selection | 25 |
| `completion` | Filter, item, source, state, trigger | 20 |
| `count_parser` | Numeric prefix parsing (5j, 10w, etc.) | 6 |
| `explorer` | Node, render, state, tree | 15 |
| `highlight` | Color, span, store, theme | 18 |
| `jumplist` | Jump navigation | 4 |
| `range-finder` | Jump navigation and code folding | 15+ |
| `screen/layout` | Layout calculations | 4 |
| `telescope` | Item, matcher, state | 11 |
| `types` | Core data types | 4 |
| `folding` | Fold state, toggle, markers | 4 |
| `rpc` | Server config, transport, types | 15 |
| `testing` | Key parsing, harness spawn | 10 |

**Unit Tests: 213**
**Integration Tests: 40** (basic_editing: 10, mode_switching: 8, resize: 5, visual_snapshot: 17)

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
cargo build --release  # Required for integration tests
cargo test             # All tests must pass
cargo clippy           # No warnings allowed
cargo fmt -- --check   # Code must be formatted
```

## Related Documentation

- [Development](./DEVELOPMENT.md) - Build and code standards
- [Commands](./commands.md) - Command system (testable units)
