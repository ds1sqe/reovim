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
reovim server --tcp 12530

# Terminal 2: Start TUI (interactive or headless)
reovim tui --tcp 127.0.0.1:12530
# OR for CI/scripting:
reovim tui --headless --tcp 127.0.0.1:12530

# Terminal 3: Capture frame
reovim cli --tcp 127.0.0.1:12530 capture
```

## Output Formats

### `raw_ansi` (default)

Full ANSI-escaped output with colors, suitable for terminal replay:

```bash
reovim cli capture --format raw_ansi
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
reovim cli capture --format plain_text
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
reovim cli capture --format cell_grid
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
reovim tui --headless --tcp 127.0.0.1:12530
```

Headless mode:
- Connects to server like interactive TUI
- Maintains editor state from notifications
- Responds to capture requests with rendered frames
- Uses default 80x24 size (configurable via layout notifications)

## RPC Method

For direct RPC integration:

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tui/capture",
  "params": {
    "format": "plain_text"
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "width": 80,
    "height": 24,
    "format": "plain_text",
    "content": "=== FRAME CAPTURE ===\n..."
  }
}
```

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
SCREEN=$(reovim cli capture --format plain_text)
echo "$SCREEN" | llm "What mode is the editor in?"
```

### Automated Testing

Verify editor state in tests:

```bash
reovim cli keys 'iHello<Esc>'
CAPTURE=$(reovim cli capture --format plain_text)
echo "$CAPTURE" | grep -q "Hello" && echo "PASS"
```

### Accessibility

Stream content for screen readers:

```bash
while true; do
  reovim cli capture --format plain_text
  sleep 1
done
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  CLI Client                                             │
│  reovim cli capture --format plain_text                 │
└─────────────────────┬───────────────────────────────────┘
                      │ RPC: tui/capture
                      ▼
┌─────────────────────────────────────────────────────────┐
│  Server                                                 │
│  - Routes request to TUI client                         │
│  - Tracks pending captures with timeout                 │
│  - Relays response back to CLI                          │
└─────────────────────┬───────────────────────────────────┘
                      │ Notification: tui/capture-request
                      ▼
┌─────────────────────────────────────────────────────────┐
│  TUI Client (Interactive or Headless)                   │
│  - Fetches state/screen_content from server             │
│  - Builds frame with RenderState + FrameBuffer          │
│  - Sends tui/capture-response notification              │
└─────────────────────────────────────────────────────────┘
```

## Related

- [Server Mode](./server-mode.md) - Running reovim as a server
- [CLI Commands](./commands.md) - All CLI commands
