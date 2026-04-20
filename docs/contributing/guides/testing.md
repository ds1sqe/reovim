# Testing Guide

This guide covers running and writing tests for reovim.

## Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p reovim-kernel
cargo test -p reovim-server

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run tests matching a pattern
cargo test buffer
```

## Test Organization

### File Layout Rules

Tests should be separated from implementation files. Three rules based on module
structure and test size:

**Rule 1 -- Single file module: `{name}_tests.rs` sibling**

```
src/
├── word.rs
├── word_tests.rs          # tests for word.rs
├── bracket.rs
└── bracket_tests.rs       # tests for bracket.rs
```

Included via `#[cfg(test)] mod word_tests;` in the parent `mod.rs` or `lib.rs`.

**Rule 2 -- Directory module, total tests < 500 lines: single `tests.rs`**

```
src/
├── operators/
│   ├── mod.rs
│   ├── delete.rs
│   ├── change.rs
│   └── tests.rs           # tests for all operators
```

Included via `#[cfg(test)] mod tests;` in `mod.rs`.

**Rule 3 -- Directory module, total tests >= 500 lines: `tests/` subdirectory**

```
src/
├── operators/
│   ├── mod.rs
│   ├── delete.rs
│   ├── change.rs
│   └── tests/
│       ├── mod.rs
│       ├── delete.rs       # tests for delete only
│       ├── change.rs       # tests for change only
│       └── yank.rs         # tests for yank only
```

Included via `#[cfg(test)] mod tests;` in `mod.rs`, with `tests/mod.rs`
re-exporting submodules.

| Situation | Layout | Inclusion |
|-----------|--------|-----------|
| Single file, any test size | `{name}_tests.rs` sibling | `#[cfg(test)] mod {name}_tests;` in parent mod |
| Directory, tests < 500 lines | `tests.rs` inside dir | `#[cfg(test)] mod tests;` in `mod.rs` |
| Directory, tests >= 500 lines | `tests/` subdir mirroring source | `#[cfg(test)] mod tests;` in `mod.rs` |
| E2E / cross-module | Cargo `tests/` directory | Automatic by Cargo |

Enforced by `scripts/check-test-layout.sh` which detects inline test blocks.
Run `./scripts/check-test-layout.sh --fix` for guidance on fixing violations.

### Shared Test Helpers

**`reovim_kernel::testing`** provides kernel-level test utilities:

```rust
use reovim_kernel::testing::{create_test_context, setup_buffer, TestBufferManager};

// Standard test context with real in-memory buffer manager
let ctx = create_test_context();

// Create a buffer with content
let id = setup_buffer(&ctx, "hello\nworld");
```

- `create_test_context()` -- `KernelContext` with a real `TestBufferManager`
  (stores and retrieves buffers, unlike `KernelContext::default()` which uses a
  stub)
- `setup_buffer(ctx, content)` -- convenience for `Buffer::from_string()` +
  `ctx.buffers.register()`
- `TestBufferManager` -- in-memory `BufferManager` implementation

For tests that need services (undo, search, etc.), create a context then
register services:

```rust
let ctx = create_test_context();
let undo_registry = Arc::new(UndoProviderRegistry::new());
undo_registry.register(UndoKey::Buffer, mock_undo.clone());
ctx.services.register(undo_registry);
```

**`reovim_driver_session::testing`** provides session-level test utilities:

```rust
use reovim_driver_session::testing::TestSessionRuntime;

let mut test = TestSessionRuntime::new();
test.with_runtime(|runtime| { /* use runtime */ });
test.assert_mode_name("normal");
test.assert_cursor(0, 0);
```

Use `reovim_kernel::testing` for pure kernel-level tests (motions, text objects,
operators). Use `TestSessionRuntime` for tests that need session state (mode
transitions, window management, command execution).

### Test Naming Convention

```
test_{action}_{scenario}[_{expected_outcome}]
```

Examples:
- `test_delete_word_at_line_start`
- `test_cursor_move_past_eof_clamps`
- `test_yank_register_stores_linewise`
- `test_visual_select_empty_line`

### Ignored Test Policy

Every `#[ignore]` must reference a tracking issue:

```rust
#[ignore = "Dot repeat not fully implemented (#NNN)"]
```

Process for deferring tests:
1. Create a deferral draft at `tmp/deferral-draft-{topic}.md`
2. Continue working
3. After user approval, create the issue and update the `#[ignore]` annotation

## Integration Testing

Reovim includes an end-to-end integration test system that uses server mode to verify key input → expected output behavior by spawning a real server process.

### Architecture

```
TestServerHarness
├── Spawns: reovim server --grpc 0
├── GrpcClient (TCP connection via gRPC)
└── Auto-cleanup on Drop

Test Flow:
┌─────────────┐      gRPC         ┌─────────────────────────┐
│ GrpcClient  │ ────────────────→ │    reovim server        │
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
# All integration tests (using shared testing infrastructure)
cargo test -p reovim-testing

# Run specific module tests
cargo test -p reovim-module-vim

# Single test
cargo test -p reovim-module-vim test_visual_mode
```

**Note:** Tests use the debug binary by default. Set `REOVIM_TEST_BINARY` env var to override.

### Writing Integration Tests

#### Basic Structure

```rust
use reovim_testing::IntegrationTest;

#[tokio::test]
async fn test_example() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")   // Initial buffer content
        .send_keys("jj")               // Key sequence (vim notation)
        .run()
        .await;

    result.assert_cursor(2, 0);
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
.send_keys("ihello<Esc>")    // Enter insert, type "hello", escape
.send_keys(":wq<CR>")        // Command mode, type "wq", enter
.send_keys("<C-d>")          // Ctrl+D
.send_keys("5j")             // Move down 5 lines
.send_keys("daw")            // Delete a word
```

#### TestResult Assertions

| Method | Description |
|--------|-------------|
| `assert_normal_mode()` | Editor is in normal mode |
| `assert_insert_mode()` | Editor is in insert mode |
| `assert_visual_mode()` | Editor is in visual mode |
| `assert_cursor(line, col)` | Cursor at position (0-indexed, line then column) |
| `assert_buffer_contains("text")` | Buffer contains substring |
| `assert_buffer_eq("text")` | Buffer equals exactly |

#### Accessing TestResult Fields

```rust
let result = IntegrationTest::new()
    .await
    .send_keys("ihello<Esc>")
    .run()
    .await;

// Direct field access
println!("Mode: {:?}", result.edit_mode);
println!("Buffer: {}", result.buffer_content);
println!("Cursor: line={}, col={}", result.cursor_line, result.cursor_column);
```

### Integration Test Organization

```
tools/testing/src/
├── lib.rs                  # Test harness exports
├── harness.rs              # TestServerHarness (subprocess management)
├── integration.rs          # IntegrationTest builder + TestResult
└── assertions.rs           # Assertion utilities

ext/server/modules/vim/tests/   # Vim module-specific tests
├── ex_commands.rs          # Ex-command tests
├── register_isolation.rs   # Register isolation tests
└── textobjects.rs          # iw, aw, i(, a{, etc.
```

### Test Harness Components

#### TestServerHarness (`tools/testing/src/harness.rs`)

Spawns and manages a reovim server process:

```rust
// Automatically spawns server on OS-assigned port
let harness = TestServerHarness::spawn().await?;

// Get a connected client
let client = harness.client().await?;

// Server is killed when harness is dropped (kill_on_drop)
```

#### TestClient (`tools/testing/src/`)

gRPC client used in multi-client tests (obtained from `MultiClientTest`):

```rust
// TestClient is accessed via the clients array in MultiClientTest::run()
clients[0].send_keys("ihello<Esc>").await.unwrap();  // Inject keys
let cursor = clients[0].get_cursor().await.unwrap(); // Get cursor position
let content = clients[0].get_buffer().await.unwrap(); // Get buffer content
clients[0].open_buffer("/path/to/file").await.unwrap(); // Open a file
```

#### IntegrationTest (`tools/testing/src/integration.rs`)

Fluent builder for tests:

```rust
IntegrationTest::new()
    .await
    .with_buffer("initial text")   // Optional: set initial buffer
    .send_keys("dd")               // Optional: inject keys (can chain)
    .send_keys("p")
    .run()
    .await
```

### Step-by-Step Testing

For tests that need to verify state at each keystroke, use `StepTest`:

```rust
use reovim_testing::StepTest;

#[tokio::test]
async fn test_delete_word_step_by_step() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .step("d")
            .expect_mode_contains("DELETE")
        .step("w")
            .expect_buffer("world")
        .run()
        .await;

    trace.assert_ok();
}
```

### Frame Assertions

For tests that need to verify TUI output:

```rust
use reovim_testing::frame::{assert_frame_contains, assert_statusline_mode};

// Assert frame contains text
assert_frame_contains(&frame, "Hello");

// Assert statusline shows mode
assert_statusline_mode(&frame, "NORMAL");

// Assert specific line content
assert_frame_line_contains(&frame, 0, "fn main");
```

### Best Practices

1. **Tests use the debug binary by default** - Set `REOVIM_TEST_BINARY` env var to override
2. **Keep key sequences short** - Long sequences are harder to debug
3. **Test one behavior per test** - Makes failures easier to diagnose
4. **Use `with_buffer()` for cursor tests** - Need text to move through
5. **Tests run in parallel** - Each spawns its own server on an OS-assigned port

## Phase 7+ Integration Tests

Phase 7+ uses a comprehensive integration test framework in `tools/testing/` that tests the complete editor through gRPC, using subprocess-based test isolation.

### Architecture

```
IntegrationTest (Builder)
├── TestServerHarness
│   ├── Spawns: reovim server --grpc 0 (OS-assigned port)
│   ├── Parses port from stderr
│   └── kill_on_drop(true) for cleanup
├── GrpcClient (TCP gRPC connection)
└── TestResult (captures final state)

Test Flow:
┌──────────────────┐       gRPC        ┌─────────────────────────┐
│ IntegrationTest  │ ────────────────→ │    reovim server        │
│                  │                   │                         │
│ with_buffer()    │  buffer/set       │  Buffer state           │
│ send_keys()      │  input/keys       │  Key processing         │
│ run() → Result   │  state/* queries  │  Mode, cursor, etc.     │
└──────────────────┘ ←──────────────── └─────────────────────────┘
                       TestResult
```

### IntegrationTest Builder

The fluent builder pattern makes tests readable and maintainable:

```rust
use reovim_testing::IntegrationTest;

#[tokio::test]
async fn test_delete_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")  // Set initial content
        .send_keys("jdd")                        // Move down, delete line
        .run()
        .await;

    result.assert_buffer_eq("line 1\nline 3");
    result.assert_cursor(1, 0);
}
```

#### Builder Methods

| Method | Description |
|--------|-------------|
| `new().await` | Spawn server, create test instance |
| `with_buffer(content)` | Set initial buffer content |
| `with_cursor_at(line, col)` | Set initial cursor position |
| `send_keys(keys)` | Queue key sequence (can chain multiple) |
| `run().await` | Execute test, return `TestResult` |

#### TestResult Assertions

| Method | Description |
|--------|-------------|
| `assert_buffer_eq(expected)` | Buffer exactly matches |
| `assert_buffer_contains(text)` | Buffer contains substring |
| `assert_cursor(line, col)` | Cursor at position (0-indexed) |
| `assert_mode!(mode)` | Editor in specified mode (macro) |
| `assert_register(name, content, yank_type)` | Register contains expected text and yank type |

### Multi-Client Tests

For concurrent client testing:

```rust
use reovim_testing::MultiClientTest;

#[tokio::test]
async fn test_two_clients_see_changes() {
    MultiClientTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 0 makes a change
            clients[0].send_keys("ihello<Esc>").await.unwrap();

            // Client 1 should see it
            let content = clients[1].get_buffer().await.unwrap();
            assert!(content.contains("hello"));
        })
        .await;
}
```

### Test File Organization

```
tools/testing/src/
├── lib.rs                  # Test harness exports
├── harness.rs              # TestServerHarness (subprocess management)
├── integration.rs          # IntegrationTest builder + TestResult
├── multi_client.rs         # MultiClientTest + TestClient
├── presence.rs             # Presence/connection utilities
├── step_test.rs            # StepTest builder for per-keystroke assertions
├── frame.rs                # Frame assertion helpers
└── assertions.rs           # Assertion macros

ext/server/modules/vim/tests/   # Vim module tests
├── ex_commands.rs          # Ex-command tests
├── register_isolation.rs   # Register isolation tests
└── textobjects.rs          # iw, aw, i(, a{, etc.
```

### Running Phase 7+ Tests

```bash
# Run all integration tests
cargo test -p reovim-testing

# Run specific module tests
cargo test -p reovim-module-vim --test ex_commands
cargo test -p reovim-module-vim --test textobjects

# Run single test
cargo test -p reovim-module-vim --test ex_commands test_write_command

# Run with output
cargo test -- --nocapture
```

### Writing New Tests

1. **Create test file** in module's `tests/` directory
2. **Import common utilities**:
   ```rust
   use reovim_testing::IntegrationTest;
   ```
3. **Use async test**:
   ```rust
   #[tokio::test]
   async fn test_your_feature() {
       let result = IntegrationTest::new()
           .await
           .with_buffer("test content")
           .send_keys("your keys")
           .run()
           .await;

       result.assert_buffer_eq("expected");
   }
   ```

### Test Infrastructure Overview

| Component | Location | Purpose |
|-----------|----------|---------|
| Test harness | `tools/testing/` | Subprocess management, client utilities |
| Integration builder | `tools/testing/src/integration.rs` | Fluent test API |
| Module tests | `ext/server/modules/*/tests/` | Per-module integration tests |
| Protocol | `shared/protocol/` | gRPC v2 definitions |

### Port Allocation

Integration tests use **OS-assigned ports** (port 0) for collision-free parallel execution:

- **Test servers**: OS-assigned ephemeral ports (let the kernel pick an available port)
- **Regular servers**: Port 12540 by default, auto-increments to 12541-12549 if in use

This approach ensures:
- **No port collisions** - OS guarantees port uniqueness
- **Parallel-safe** - Multiple test processes can run simultaneously
- **Cross-process safe** - Works correctly with `cargo test` parallelism
- Tests don't interfere with manually started debug servers

### How It Works

1. `IntegrationTest::new()` spawns `reovim server --grpc 0` (OS-assigned port)
2. Server binds to an ephemeral port and prints `Listening on 127.0.0.1:<port>` to stderr
3. Test harness reads stderr to discover the actual port
4. `GrpcClient` connects via gRPC and sends commands
5. Keys are injected with delays between each (for mode propagation)
6. After keys, delay allows processing before querying state
8. Test cleans up server via kill command

## Current Test Coverage

Test coverage is tracked via Codecov. The project enforces **100% coverage** on all PRs:

- **Non-server crates**: 100% MC/DC coverage (line + condition)
- **Server crate** (`reovim-server`): 100% line coverage

Coverage reports are uploaded as CI artifacts (`COVERAGE-workspace.md`, `COVERAGE-server.md`) and enforced via Codecov status checks. PRs that regress coverage are blocked.

### Phase 7+ Integration Tests

| Test File | Coverage Area |
|-----------|--------------|
| `ex_commands.rs` | Ex-command tests |
| `register_isolation.rs` | Register isolation tests |
| `textobjects.rs` | iw, aw, i(, a{, etc. |
| `multi_client_tests.rs` | Concurrent clients, shared state |

## Writing Tests

### Test Module Pattern

Tests are placed in a `tests` submodule within the source file:

```rust
// In server/lib/kernel/src/core/buffer.rs
#[cfg(test)]
mod tests;

// In server/lib/kernel/src/core/tests.rs
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

## Code Coverage

Reovim enforces **100% code coverage** in CI. Every PR must maintain full coverage.

### Running Coverage Locally

```bash
# MC/DC coverage (non-server crates) — primary metric
./scripts/coverage.sh mcdc --lcov

# Line coverage (full workspace including server)
./scripts/coverage.sh line --lcov

# Generate human-readable report
./scripts/coverage-report.sh target/llvm-cov/lcov.mcdc.info

# Open HTML report in browser
./scripts/coverage.sh mcdc --open
```

### Coverage Annotations

For genuinely untestable code, use the `coverage(off)` attribute:

```rust
#[cfg_attr(coverage_nightly, coverage(off))]
fn untestable_function() { /* ... */ }
```

The `coverage_attribute` feature requires nightly Rust. Enable it with `#![cfg_attr(coverage_nightly, feature(coverage_attribute))]` in your crate's `lib.rs`.

Use sparingly — only for code paths that cannot be exercised in unit tests (async runtime internals, tracing closures, panic handlers).

### Codecov Integration

Coverage is enforced via Codecov status checks on PRs:
- **Project target**: 100% (0.5% threshold for float rounding)
- **Patch target**: 100% (all new code must be covered)
- Reports uploaded as CI artifacts (`COVERAGE-workspace.md`, `COVERAGE-server.md`)

## Performance Benchmarks

Reovim uses Criterion for performance benchmarking.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p reovim-bench

# Run specific benchmark group
cargo bench -p reovim-bench -- window_render
cargo bench -p reovim-bench -- stress
cargo bench -p reovim-bench -- buffer_clone

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
tools/bench/
├── src/
│   └── lib.rs             # Benchmark utilities
└── benches/
    ├── render.rs          # Main entry point
    └── bench_modules/
        ├── common.rs      # Shared utilities
        ├── window.rs      # Window render benchmarks
        ├── screen.rs      # Screen I/O benchmarks
        ├── input.rs       # Input simulation
        ├── rtt.rs         # Round-trip time
        └── stress.rs      # Stress tests
```

## Continuous Integration

Before submitting a PR:

```bash
cargo test             # All tests must pass
cargo clippy           # No warnings allowed
cargo fmt -- --check   # Code must be formatted
```

## Related Documentation

- [Getting Started](../getting-started.md) - Build and code standards
- [Commands](../../user-guide/commands.md) - Command system (testable units)
