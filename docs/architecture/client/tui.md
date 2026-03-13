# TUI Client

Terminal user interface with full rendering.

## Source Location

`clients/tui/src/` (moved from `runner/src/client/tui/` in Phase 8)

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

## Notification Handling

The TUI subscribes to server notifications for real-time updates:

- `mode_changed` → Update status line
- `cursor_moved` → Update cursor display
- `buffer_modified` → Request screen refresh

## Related Documents

- [Client Overview](./overview.md) - Client architecture
- [CLI Client](./cli.md) - Command-line interface
- [Notifications](../server/notifications.md) - Server notifications
