# Driver Layer

Drivers (`server/lib/drivers/*`) implement traits defined by the kernel. Each driver is a separate crate.

## Driver Overview

| Driver | Crate | Purpose | Documentation |
|--------|-------|---------|---------------|
| `command/` | `reovim-driver-command` | Command traits and execution | [command/overview.md](./command/overview.md) |
| `syntax/` | `reovim-driver-syntax` | Syntax highlighting abstraction | [syntax/overview.md](./syntax/overview.md) |
| `input/` | `reovim-driver-input` | Keyboard, mouse, clipboard | [input/overview.md](./input/overview.md) |
| `display/` | `reovim-driver-display` | Frame buffer, compositor | [display/overview.md](./display/overview.md) |
| `lsp/` | `reovim-driver-lsp` | LSP client infrastructure | [lsp/overview.md](./lsp/overview.md) |
| `net/` | `reovim-driver-net` | RPC server, transports | [net/overview.md](./net/overview.md) |
| `vfs/` | `reovim-driver-vfs` | Virtual filesystem | [vfs/overview.md](./vfs/overview.md) |
| `session/` | `reovim-driver-session` | Session management traits | [session/overview.md](./session/overview.md) |
| `log/` | `reovim-driver-log` | Logger implementation (tracing) | [log/overview.md](./log/overview.md) |

## Layer Position

```
┌─────────────────────────────────────────────────────────────┐
│  Clients (clients/tui/, clients/cli/)                       │
├─────────────────────────────────────────────────────────────┤
│  Server (server/lib/server/)                                │
├─────────────────────────────────────────────────────────────┤
│  Modules (server/modules/) - policy                         │
├─────────────────────────────────────────────────────────────┤
│  Drivers (server/lib/drivers/)  ← YOU ARE HERE              │
│  ├── command/      - Command execution                      │
│  ├── input/        - Key/mouse input                        │
│  ├── syntax/       - Syntax highlighting                    │
│  ├── lsp/          - Language server                        │
│  ├── vfs/          - Filesystem                             │
│  ├── session/      - Session management                     │
│  ├── buffer/       - Buffer operations                      │
│  ├── undo/         - Undo/redo system                       │
│  ├── search/       - Search and replace                     │
│  ├── clipboard/    - Clipboard operations                   │
│  └── ffi/          - Foreign function interface             │
├─────────────────────────────────────────────────────────────┤
│  Kernel (server/lib/kernel/)                                │
└─────────────────────────────────────────────────────────────┘
```

**Note:** Display and TUI drivers are in `clients/tui/lib/drivers/` since they are client-specific.

## Driver Registration

Drivers are registered at startup in the server:

```rust
// server/lib/server/src/server.rs

let vfs = StandardVfs::new();
let syntax = TreesitterSyntax::new();
let session = SessionManager::new();

let server = ServerBuilder::new()
    .vfs(vfs)
    .syntax(syntax)
    .session(session)
    .build();
```

**Note:** Display drivers are TUI-specific and registered in `clients/tui/`.

## Related Documents

- [Architecture Overview](../architecture/overview.md) - System-wide architecture
- [Kernel Subsystems](../kernel/overview.md) - Kernel internals
- [Module System](../modules/overview.md) - Dynamic modules
