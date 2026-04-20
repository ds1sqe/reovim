# Driver Layer

Drivers (`server/lib/drivers/*`) implement traits defined by the kernel. Each driver is a separate crate.

## Driver Overview (27 crates)

| Driver | Crate | Purpose | Documentation |
|--------|-------|---------|---------------|
| `annotation/` | `reovim-driver-annotation` | Annotation system | - |
| `buffer/` | `reovim-driver-buffer` | Buffer manager registry | [buffer/overview.md](./buffer/overview.md) |
| `clipboard/` | `reovim-subsys-clipboard` | System clipboard interface | [clipboard/overview.md](./clipboard/overview.md) |
| `codec/` | `reovim-driver-codec` | Codec framework, file type detection | - |
| `codec-xxd/` | `reovim-driver-codec-xxd` | XXD hex codec | - |
| `command/` | `reovim-driver-command` | Command traits and execution | [command/overview.md](./command/overview.md) |
| `command-types/` | `reovim-driver-command-types` | CommandContext, CommandResult | [command-types/overview.md](./command-types/overview.md) |
| `completion/` | `reovim-driver-completion` | Completion framework | - |
| `ffi/` | `reovim-driver-ffi` | C FFI interface + ABI versioning | [ffi/overview.md](./ffi/overview.md) |
| `ffi-python/` | `reovim-driver-ffi-python` | Python bindings via PyO3 | [ffi-python/overview.md](./ffi-python/overview.md) |
| `formatter/` | `reovim-driver-formatter` | Code formatting interface | - |
| `git/` | `reovim-driver-git` | Git integration | - |
| `input/` | `reovim-driver-input` | Keyboard, mouse input | [input/overview.md](./input/overview.md) |
| `layout/` | `reovim-subsys-layout` | Layout traits and policies | - |
| `lsp/` | `reovim-driver-lsp` | LSP client infrastructure | [lsp/overview.md](./lsp/overview.md) |
| `manifest/` | `reovim-subsys-manifest` | Module manifest definitions | - |
| `module-config/` | `reovim-subsys-module-config` | Module configuration | - |
| `module-loader/` | `reovim-subsys-module-loader` | Dynamic module loading | - |
| `module-registry/` | `reovim-subsys-module-registry` | Module registry | - |
| `picker/` | `reovim-driver-picker` | Picker/fuzzy-find framework | - |
| `search/` | `reovim-driver-search` | Search provider interface | [search/overview.md](./search/overview.md) |
| `session/` | `reovim-driver-session` | Session management traits | [session/overview.md](./session/overview.md) |
| `statusline/` | `reovim-driver-statusline` | Statusline traits | - |
| `syntax/` | `reovim-driver-syntax` | Syntax highlighting abstraction | [syntax/overview.md](./syntax/overview.md) |
| `syntax-treesitter/` | `reovim-driver-syntax-treesitter` | Tree-sitter implementation | [syntax-treesitter/overview.md](./syntax-treesitter/overview.md) |
| `undo/` | `reovim-driver-undo` | Undo provider interface | [undo/overview.md](./undo/overview.md) |
| `vfs/` | `reovim-subsys-vfs` | Virtual filesystem | [vfs/overview.md](./vfs/overview.md) |

### Non-Server Drivers

These drivers live outside `server/lib/drivers/`:

| Driver | Location | Purpose | Documentation |
|--------|----------|---------|---------------|
| `display/` | `clients/tui/lib/drivers/display/` | Frame buffer, compositor | [display/overview.md](./display/overview.md) |
| `tui/` | `clients/tui/lib/drivers/tui/` | Terminal I/O | - |

### Shared Libraries (not drivers)

These are sometimes referenced alongside drivers but are shared infrastructure:

| Crate | Location | Purpose | Documentation |
|-------|----------|---------|---------------|
| `reovim-subsys-net` | `server/lib/subsys/net/` | Network transport contracts | [net/overview.md](./net/overview.md) |

## Layer Position

```
┌─────────────────────────────────────────────────────────────┐
│  Clients (clients/tui/, clients/cli/)                       │
├─────────────────────────────────────────────────────────────┤
│  Server (server/lib/server/)                                │
├─────────────────────────────────────────────────────────────┤
│  Modules (server/modules/) - policy                         │
├─────────────────────────────────────────────────────────────┤
│  Drivers (server/lib/drivers/) — 27 crates  ← YOU ARE HERE  │
│  ├── command/      - Command execution                      │
│  ├── input/        - Key/mouse input                        │
│  ├── syntax/       - Syntax highlighting                    │
│  ├── lsp/          - Language server                        │
│  ├── vfs/          - Filesystem                             │
│  ├── session/      - Session management                     │
│  ├── buffer/       - Buffer operations                      │
│  ├── codec/        - File type detection, encoding          │
│  ├── module-loader/- Dynamic module loading                 │
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

- [Architecture Overview](../overview.md) - System-wide architecture
- [Kernel Subsystems](../kernel/overview.md) - Kernel internals
- [Module System](../modules/overview.md) - Dynamic modules
