# Reovim Testing

Zero-config, type-safe, just works.

## Quick Start

```python
from reovim import Editor, edit

# One-liner magic
result = edit("hello world", "dw")  # Returns "world"

# Full control with context manager
with Editor() as e:
    e.keys("iHello World<Esc>")
    print(e.mode)      # "NORMAL"
    print(e.buffer)    # "Hello World"
    print(e.cursor)    # Cursor(line=0, col=10)

    # Fluent chaining
    e.keys("gg0dw").assert_buffer("World").assert_mode("normal")

    # Rich capture for testing/debugging
    snap = e.capture("after delete")
    snap.print_frame()     # ANSI-colored TUI output
    print(snap.to_json())  # Structured JSON
```

## Features

- **Zero-config** - Auto-discovers binary, picks ports
- **Fluent API** - Every method returns self for chaining
- **Context manager** - Automatic server/TUI lifecycle
- **Type-safe** - Proper dataclasses, not dicts
- **Rich capture** - Frame buffer, mode, cursor, buffer, registers
- **Helpful errors** - Clear messages when things fail

## API

### Editor

The main class for interacting with reovim.

```python
with Editor() as e:
    # Properties (instant access)
    e.mode      # Current mode ("NORMAL", "INSERT", etc.)
    e.buffer    # Buffer content
    e.cursor    # Cursor(line, col)
    e.screen    # Raw ANSI frame

    # Actions (return self for chaining)
    e.keys("dw")           # Send vim keys
    e.type("Hello")        # Insert mode shortcut

    # Capture (full state snapshot)
    snap = e.capture("label")

    # Assertions (raise on failure, return self)
    e.assert_mode("normal")
    e.assert_buffer("expected")
    e.assert_cursor(0, 5)
    e.assert_register('"', "yanked")

    # Wait for conditions
    e.wait_for(lambda s: "INSERT" in s.mode)
```

### Capture

Immutable snapshot of editor state.

```python
snap = e.capture("after delete")

# Access fields
snap.frame       # Raw ANSI screen content
snap.mode        # "NORMAL", "INSERT", etc.
snap.cursor      # Cursor(line, col)
snap.buffer      # Buffer content
snap.registers   # dict[str, Register]
snap.timestamp   # float

# Methods
snap.to_json()          # JSON string for LLM
snap.to_dict()          # Python dict
snap.print_frame()      # Print ANSI to terminal
snap.save_frame(path)   # Save for diff
```

### Convenience Functions

```python
from reovim import edit, test

# One-liner
result = edit("hello world", "dw")  # "world"

# Test helper
with test("hello world") as e:
    e.keys("dw").assert_buffer("world")
```

## Installation

```bash
# From tools/reovim-testing directory
pip install -e .

# Or just run directly
python examples/showcase.py
```

## Prerequisites

Build reovim with gRPC support:

```bash
cargo build --release -p reovim-bin --features grpc
```

## Binary Discovery

The module auto-discovers binaries in this order:

1. `REOVIM_BINARY` environment variable
2. `./target/release/reovim-new` (gRPC, Phase 8)
3. `./target/debug/reovim-new`
4. `./target/release/reovim` (TCP, legacy)
5. `./target/debug/reovim`
6. System PATH

## Architecture

```
reovim/
├── __init__.py      # Clean exports
├── editor.py        # Editor class (the heart)
├── capture.py       # Capture, Cursor, Register
├── discovery.py     # Binary/port discovery
├── process.py       # Server/TUI management
├── client.py        # CLI wrapper
└── errors.py        # Custom exceptions
```

## Capture Relay Flow

Screen capture works via CLI→Server→TUI→Server→CLI relay:

```
1. Editor.capture() calls Client.capture()
2. Client runs: reovim-new cli capture --capture-format raw_ansi
3. CLI sends GetScreenContent RPC to Server
4. Server sends capture_request notification to TUI
5. TUI captures frame buffer, calls SubmitCaptureResponse RPC
6. Server returns frame content to CLI
7. Client parses output and returns
```

## License

AGPL-3.0 - Same as reovim
