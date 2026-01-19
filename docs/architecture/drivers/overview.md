# Driver Layer

Drivers (`lib/drivers/*`) implement traits defined by the kernel. Each driver is a separate crate.

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
│  Runner (application orchestration)                         │
├─────────────────────────────────────────────────────────────┤
│  Modules (policy)                                           │
├─────────────────────────────────────────────────────────────┤
│  Drivers (mechanism services)  ← YOU ARE HERE               │
│  ├── command/  - Command execution                          │
│  ├── syntax/   - Syntax highlighting                        │
│  ├── input/    - Key/mouse input                            │
│  ├── display/  - Frame buffer                               │
│  ├── lsp/      - Language server                            │
│  ├── net/      - Network/RPC                                │
│  ├── vfs/      - Filesystem                                 │
│  ├── session/  - Session management                         │
│  └── log/      - Logging                                    │
├─────────────────────────────────────────────────────────────┤
│  Kernel (core primitives)                                   │
└─────────────────────────────────────────────────────────────┘
```

## Driver Registration

Drivers are registered at startup in the runner:

```rust
// runner/src/main.rs

let display = CrosstermDisplay::new();
let input = CrosstermInput::new();
let vfs = StandardVfs::new();
let logger = TracingLogger::new();

let ctx = KernelContext::builder()
    .display(display)
    .input(input)
    .vfs(vfs)
    .logger(logger)
    .build();
```

## Related Documents

- [Architecture Overview](../architecture/overview.md) - System-wide architecture
- [Kernel Subsystems](../kernel/overview.md) - Kernel internals
- [Module System](../modules/overview.md) - Dynamic modules
