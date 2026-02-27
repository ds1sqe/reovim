# Frame Capture

Capture the current TUI frame programmatically via RPC relay.

## Overview

Frame capture enables external tools (LLMs, test harnesses, accessibility tools) to retrieve the current editor display. The capture flows through a relay:

```
CLI ─────► Server ─────► TUI ─────► Server ─────► CLI
     request      route      render      relay      response
```

This architecture ensures:
- **Server stays policy-free** - routes requests, doesn't render
- **TUI owns rendering** - consistent output whether interactive or headless
- **CLI stays simple** - single RPC call

## Quick Start

```bash
# Terminal 1: Start server
reovim server --grpc 12530

# Terminal 2: Start TUI (interactive or headless)
reovim tui --grpc 127.0.0.1:12530
# OR for CI/scripting:
reovim tui --headless --grpc 127.0.0.1:12530

# Terminal 3: Capture frame (--client specifies target TUI's client ID)
reovim cli --grpc 127.0.0.1:12530 capture --client 1
```

## Output Formats

### `raw_ansi` (default)

Full ANSI-escaped output with colors, suitable for terminal replay:

```bash
reovim cli capture --client 1 --capture-format raw_ansi
```

Output:
```
=== FRAME CAPTURE ===
Timestamp: 2026-01-26T12:34:56Z
Size: 80x24
Mode: NORMAL
Cursor: 0,0
---
[ANSI-colored frame content]
```

### `plain_text`

Plain text without ANSI codes, ideal for LLM consumption:

```bash
reovim cli capture --client 1 --capture-format plain_text
```

Output:
```
=== FRAME CAPTURE ===
Timestamp: 2026-01-26T12:34:56Z
Size: 80x24
Mode: NORMAL
Cursor: 0,0
---
Hello world
~
~
```

### `cell_grid`

JSON structure with per-cell styling, for programmatic analysis:

```bash
reovim cli capture --client 1 --capture-format cell_grid
```

Output:
```json
{
  "width": 80,
  "height": 24,
  "cells": [
    [{"char": "H", "fg": 15, "bg": 0, "bold": false}, ...]
  ]
}
```

## Headless Mode

For CI/testing without a terminal, use headless TUI:

```bash
# Start headless TUI (no terminal required)
reovim tui --headless --grpc 127.0.0.1:12530
```

Headless mode:
- Connects to server like interactive TUI
- Maintains editor state from notifications
- Responds to capture requests with rendered frames
- Uses default 80x24 size (configurable via layout notifications)

## gRPC Method

For direct gRPC integration (see `shared/protocol/proto/reovim/v2/state.proto`):

```protobuf
// Request
message GetScreenContentRequest {
  string format = 1;     // "plain_text", "raw_ansi", "cell_grid"
  uint64 client_id = 2;  // Target TUI client ID (required)
}

// Response
message GetScreenContentResponse {
  uint64 width = 1;
  uint64 height = 2;
  string format = 3;
  string content = 4;
}
```

The `client_id` field routes the capture request to a specific TUI client,
enabling multi-client scenarios where multiple TUIs are connected.

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| `NO_CAPTURE_CLIENT` | No TUI connected | Start TUI with `reovim tui` |
| `CAPTURE_TIMEOUT` | TUI didn't respond in 5s | Check if TUI is frozen |
| `CLIENT_DISCONNECTED` | TUI disconnected during capture | Reconnect TUI |
| `INVALID_PARAMS` | Unknown format | Use: raw_ansi, plain_text, cell_grid |

## Use Cases

### LLM Integration

Capture plain text for AI analysis:

```bash
SCREEN=$(reovim cli capture --client 1 --capture-format plain_text)
echo "$SCREEN" | llm "What mode is the editor in?"
```

### Automated Testing

Verify editor state in tests:

```bash
reovim cli --grpc 127.0.0.1:12530 keys 'iHello<Esc>' --client 1
CAPTURE=$(reovim cli --grpc 127.0.0.1:12530 capture --client 1 --capture-format plain_text)
echo "$CAPTURE" | grep -q "Hello" && echo "PASS"
```

### Accessibility

Stream content for screen readers:

```bash
while true; do
  reovim cli capture --client 1 --capture-format plain_text
  sleep 1
done
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  CLI Client                                             │
│  reovim cli capture --client 1 --capture-format plain_text      │
└─────────────────────┬───────────────────────────────────┘
                      │ gRPC: StateService.GetScreenContent
                      ▼
┌─────────────────────────────────────────────────────────┐
│  Server                                                 │
│  - Routes request to target TUI (by client_id)          │
│  - Tracks pending captures with timeout                 │
│  - Relays response back to CLI                          │
└─────────────────────┬───────────────────────────────────┘
                      │ Notification: CaptureRequest (target_client_id)
                      ▼
┌─────────────────────────────────────────────────────────┐
│  TUI Client (Interactive or Headless)                   │
│  - Checks target_client_id matches own client_id        │
│  - Builds frame with RenderState + FrameBuffer          │
│  - Sends CaptureResponse notification                   │
└─────────────────────────────────────────────────────────┘
```

## Related

- [Server Mode](./server-mode.md) - Running reovim as a server
- [CLI Commands](./commands.md) - All CLI commands
