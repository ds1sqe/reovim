# Window-Buffer Architecture

This document describes the relationship between windows and buffers in reovim, inspired by tmux's pane-session model.

## Overview

Reovim uses an **N:1 window-to-buffer relationship**: multiple windows can view the same buffer simultaneously, each with independent cursor positions and scroll states.

## Architecture Diagram

```
┌─────────────────────────────────────────┐
│ Screen                                   │
│ ┌──────────────────────────────────────┐│
│ │ windows: [                           ││
│ │   Window { id: 1, buffer_id: 0,     ││
│ │            is_active: true,         ││
│ │            cursor: (0, 0),          ││
│ │            buffer_anchor: (0, 0) }  ││
│ │   Window { id: 2, buffer_id: 0,     ││  ← Same buffer
│ │            is_active: false,        ││
│ │            cursor: (5, 10),         ││  ← Different cursor
│ │            buffer_anchor: (0, 5) }  ││  ← Different scroll
│ │ ]                                    ││
│ └──────────────────────────────────────┘│
└─────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ Buffer(id: 0) {              │
│   cur: (0, 0),               │  ← "Live" cursor from active window
│   contents: [...]            │
│ }                            │
└──────────────────────────────┘
```

## Dual Cursor State

The cursor state is split between two locations:

### Buffer Cursor (`Buffer.cur`)
- The **"live" editing cursor**
- Used by the currently active window
- Updated directly by editing commands
- Only one buffer cursor exists per buffer

### Window Cursor (`Window.cursor`)
- **Saved cursor position** for each window
- Preserved when a window becomes inactive
- Restored when a window becomes active again
- Each window maintains its own saved cursor

### Cursor Handoff

When switching windows, a **cursor handoff** occurs:

```rust
// Departing window: save cursor
window.cursor = buffer.cur;
window.desired_col = buffer.desired_col;

// Arriving window: restore cursor
buffer.cur = window.cursor;
buffer.desired_col = window.desired_col;
```

This ensures each window "remembers" where the cursor was when it was last active.

## Per-Window State

Each window maintains independent state:

| Field | Description |
|-------|-------------|
| `cursor` | Saved cursor position (x, y) |
| `desired_col` | Desired column for vertical movement |
| `buffer_anchor` | Scroll position (viewport offset) |
| `buffer_id` | Which buffer this window displays |
| `is_active` | Whether this is the focused window |

## Critical Implementation Rules

### Rule 1: Always Use handle_window_navigate()

**Never call `screen.navigate_window()` directly for user-initiated navigation.**

Always use `handle_window_navigate()` which performs proper cursor save/restore:

```rust
// WRONG - bypasses cursor handoff
self.screen.navigate_window(direction);

// CORRECT - handles cursor state
self.handle_window_navigate(direction);
```

### Rule 2: Only Calculate Cursor for Active Window

When rendering, **only calculate cursor position for the active window**.

```rust
// WRONG - calculates cursor for every window, last one wins
for win in &mut self.windows {
    if !self.layout.is_explorer_focused() {
        cursor_pos = Some((cursor_x, cursor_y));  // OVERWRITES!
    }
}

// CORRECT - only active window gets cursor position
for win in &mut self.windows {
    if win.is_active && !self.layout.is_explorer_focused() {
        cursor_pos = Some((cursor_x, cursor_y));
    }
}
```

This prevents the cursor highlight from appearing in the wrong window when multiple windows exist.

## Related Code

- `lib/core/src/screen/window.rs` - Window struct with per-window cursor
- `lib/core/src/buffer/mod.rs` - Buffer struct with live cursor
- `lib/core/src/runtime/handlers.rs` - `handle_window_navigate()` for cursor handoff
- `lib/core/src/screen/mod.rs` - Screen with window collection

## See Also

- tmux pane/window model
- vim's window/buffer relationship (`:help windows`)
