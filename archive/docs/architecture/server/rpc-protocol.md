# gRPC v2 Protocol

gRPC protocol over HTTP/2 with Protocol Buffers for client-server communication.

## Source Location

`uapi/protocol/proto/reovim/v2/` (13 proto files)

## Protocol Overview

Reovim uses **gRPC v2** (Protocol Buffers over HTTP/2) via `tonic`. All services are defined in `.proto` files under `uapi/protocol/proto/reovim/v2/`.

Clients connect via `--grpc HOST:PORT` (default port: 12540).

## Authentication

After joining via `PresenceService.Join`, clients receive a `session_token` in the `JoinResponse`. All subsequent authenticated requests must include this token as a gRPC metadata header:

```
x-reovim-token: <session_token>
```

The `DebugService` bypasses authentication, allowing the stateless CLI to target specific clients without joining.

## Services

12 gRPC services, ~55 RPCs total (codec RPCs added in #740):

### InputService (`input.proto`)

| RPC | Description |
|-----|-------------|
| `SendKeys` | Inject key sequence (vim notation). Caller identity from `x-reovim-token`. |

### StateService (`state.proto`)

| RPC | Description |
|-----|-------------|
| `GetMode` | Get current editor mode (per-client via `client_id`) |
| `GetCursor` | Get cursor position |
| `GetOptions` | Query editor options |
| `GetLayout` | Get window layout tree |
| `GetVisibleLines` | Get visible line range for a window |
| `GetSelection` | Get visual selection state |
| `GetScreenContent` | Capture screen via TUI relay (requires connected TUI) |
| `SubmitCaptureResponse` | TUI submits captured frame (part of capture relay) |
| `GetRegisters` | Query vim register contents (per-client since #515) |

### BufferService (`buffer.proto`)

| RPC | Description |
|-----|-------------|
| `GetRawContent` | Get raw buffer text lines |
| `GetLineCount` | Get line count (no content transfer) |
| `GetAnnotations` | Get line annotations (diagnostics, git signs) |
| `List` | List all open buffers |
| `OpenFile` | Open a file into a buffer |
| `WriteFile` | Write buffer to file |
| `SetContent` | Set buffer content |
| `MountCodec` | Mount a codec on a buffer (see MountMode below) |
| `UmountCodec` | Unmount a codec from a buffer |
| `ListMounts` | List all codec mounts active on a buffer |
| `ListAvailableCodecs` | List all registered codec factories |
| `SwitchCodecView` | **(Deprecated)** Switch codec view; kept for backward compatibility — use `MountCodec` instead |

#### MountMode

`MountCodec` accepts a `MountMode` field that controls how the codec attaches to the buffer:

| Mode | Description |
|------|-------------|
| `Summary` | Codec provides a summarised/collapsed view of the buffer content |
| `Structural` | Codec provides a fully structured, navigable decomposition of the content |

### EditorService (`editor.proto`)

| RPC | Description |
|-----|-------------|
| `Resize` | Resize terminal/viewport |
| `Quit` | Quit editor |
| `SetActiveBuffer` | Switch active buffer |
| `GetActiveBuffer` | Get active buffer ID |

### NotificationService (`notification.proto`)

| RPC | Description |
|-----|-------------|
| `Subscribe` | **Server streaming** - subscribe to real-time notifications (17 payload types) |

See [Notifications](./notifications.md) for payload details.

### ServerService (`server.proto`)

| RPC | Description |
|-----|-------------|
| `Kill` | Graceful server shutdown |
| `Info` | Get server info (version, uptime, counts) |
| `Ping` | Health check |

### DebugService (`debug.proto`)

No authentication required. Used by the stateless CLI to target specific clients.

| RPC | Description |
|-----|-------------|
| `LogTail` | Get recent log entries (with level/target/grep filters) |
| `LogLevel` | Get or set log level |
| `DebugSendKeys` | Send keys to a specific client |
| `DebugCapture` | Capture a client's screen |
| `DebugGetMode` | Get a client's mode |
| `DebugGetCursor` | Get a client's cursor position |
| `DebugListClients` | List connected clients |
| `DebugGetExtensionState` | Query extension state for a client |
| `DebugListExtensions` | List registered extensions |

### ModuleService (`module.proto`)

| RPC | Description |
|-----|-------------|
| `List` | List loaded modules |
| `Load` | Load a module |
| `Unload` | Unload a module |
| `Reload` | Hot-reload a module |

### CommandService (`command.proto`)

Added in #453 for cmdline tab completion.

| RPC | Description |
|-----|-------------|
| `SearchCommands` | Search commands by prefix (filters: all, ex-only, keybinding-only) |
| `CompleteArgs` | Complete arguments for an ex-command |

### SyntaxService (`syntax.proto`)

| RPC | Description |
|-----|-------------|
| `GetTokens` | Get syntax tokens for a buffer range |
| `StreamTokens` | **Server streaming** - real-time token updates |
| `GetLanguageInfo` | Get language metadata and parser availability |

### PresenceService (`presence.proto`)

Multi-client presence awareness (Epic #465).

| RPC | Description |
|-----|-------------|
| `Join` | Announce client, receive `client_id` and `session_token` |
| `Leave` | Clean disconnect |
| `StreamPresence` | **Server streaming** - presence updates from peers |
| `UpdatePresence` | Update viewport, mode, buffer |
| `SetSyncMode` | Set sync mode (Independent, Follow, Present) |
| `ListClients` | List all connected clients |
| `SetRole` | Set editing role (deprecated, use `SetRelation`) |
| `SetRelation` | Set client relation: Following or Sharing (#480) |

### ExtensionService (`extension.proto`)

Extension state bridge (#514).

| RPC | Description |
|-----|-------------|
| `GetState` | Query extension state by kind (e.g., "cmdline", "whichkey") |
| `ListExtensions` | List registered extensions and capabilities |

## Streaming RPCs

Three services use **server streaming** for real-time updates:

| Service | RPC | Purpose |
|---------|-----|---------|
| `NotificationService` | `Subscribe` | Editor state changes (17 event types) |
| `SyntaxService` | `StreamTokens` | Syntax token updates on buffer edits |
| `PresenceService` | `StreamPresence` | Peer presence changes (join/leave/update) |

## Common Types (`common.proto`)

Shared across all services:

| Type | Fields | Description |
|------|--------|-------------|
| `Position` | `line`, `column` | Buffer position (0-indexed) |
| `Selection` | `start`, `end` | Selection range |
| `WindowRect` | `x`, `y`, `width`, `height` | Window rectangle |
| `Status` | `ok`, `error` | Generic result |

## Related Documents

- [Server Overview](./overview.md) - Server architecture
- [Notifications](./notifications.md) - Notification payload types
- [Server Mode Reference](../../user-guide/server-mode.md) - Usage guide
