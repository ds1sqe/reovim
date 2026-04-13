# TUI Client

Terminal user interface with full rendering.

## Source Location

`clients/tui/src/`

Includes adapter module for Common Client Model: `clients/tui/src/adapter/`

## Usage

```bash
# Auto-discover server
reovim tui

# Connect to specific server
reovim tui --grpc 127.0.0.1:12540
```

## Features

- Full terminal rendering
- Real-time screen updates via notifications
- Keyboard/mouse input forwarding
- Automatic terminal resize handling

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│  TUI Client                                                 │
│  ├── Input Loop (keyboard, mouse, resize events)           │
│  ├── Notification Listener (mode changes, cursor, etc.)    │
│  └── Render Loop (screen refresh from server state)        │
└────────────────────────────────────────────────────────────┘
          │                    │
          ▼                    ▼
    [Send to Server]     [Receive Notifications]
```

## Client Modules

The TUI client loads 20 client modules that provide chrome (UI panels) and buffer
contributions. Modules implement the `ClientModule` trait from `reovim-client-driver`.

Source: `clients/tui/modules/`

| Module | Kind | Purpose |
|--------|------|---------|
| bufferline | chrome | Buffer tab line |
| cmdline | chrome | Command-line input |
| completion | chrome | Completion popup |
| diagnostics | chrome | Diagnostics panel |
| explorer | chrome | File explorer sidebar |
| fold | buffer-contrib | Code folding |
| hover | chrome | Hover information popup |
| illuminate | buffer-contrib | Word highlighting under cursor |
| jump | chrome | Jump label overlay |
| landing | chrome | Landing/welcome screen |
| line-numbers | buffer-contrib | Line number gutter |
| markdown | buffer-contrib | Markdown rendering |
| microscope | chrome | Fuzzy finder UI |
| notification | chrome | Notification display |
| pair | buffer-contrib | Auto-pair bracket highlighting |
| signature-help | chrome | Function signature popup |
| statusline | chrome | Status line |
| tetromino | chrome | Easter egg game |
| which-key | chrome | Key binding hints |
| yank-flash | buffer-contrib | Yank highlight flash |

Dynamic client modules can be loaded from `~/.local/share/reovim/client-modules/`.
See [Client Extensibility](./extensibility.md).

## Client Drivers

The TUI has two platform-specific drivers:

- `display` (`clients/tui/lib/drivers/display/`) — Display driver: FrameBuffer,
  WindowRenderer, AnnotationStore, compositor, render backend, layout, highlight, and
  style systems.
- `tui` (`clients/tui/lib/drivers/tui/`) — Terminal session management: raw mode,
  alternate screen, InputReader, Screen, FrameRenderer.

## Notification Handling

The TUI subscribes to server notifications for real-time updates:

- `mode_changed` → Update status line
- `cursor_moved` → Update cursor display
- `buffer_modified` → Request screen refresh

## Related Documents

- [Client Overview](./overview.md) - Client architecture
- [CLI Client](./cli.md) - Command-line interface
- [Notifications](../server/notifications.md) - Server notifications
