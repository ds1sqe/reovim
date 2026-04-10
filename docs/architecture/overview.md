# Architecture Overview

Reovim follows a **Linux kernel-inspired architecture** with clear separation between kernel mechanisms, drivers, and loadable modules.

## Layer Diagram (Phase 8)

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLIENTS                                 │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐  │
│  │ TUI (clients/tui)│ │ CLI (clients/cli)│ │ Web (clients/web)│  │
│  └────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘  │
│           │                    │                     │            │
│           └────────────────────┼─────────────────────┘            │
│                         │ gRPC v2 (shared/protocol/)            │
└─────────────────────────┼───────────────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────────────┐
│                    SERVER (server/lib/server/)                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  gRPC Services (12): Input, State, Buffer, Editor,      │    │
│  │  Notification, Server, Debug, Module, Command,          │    │
│  │  Syntax, Presence, Extension                            │    │
│  │  Session Management, Module Registry                    │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────┼───────────────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────────────┐
│                    MODULES (server/modules/)                    │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │
│  │   vim   │ │ motions │ │textobj  │ │ keymap  │ │ editor  │    │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘    │
│       │           │           │           │           │         │
│       └───────────┴───────────┼───────────┴───────────┘         │
│                               │                                 │
│                    use reovim_kernel::api::*                    │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                    KERNEL (server/lib/kernel/)                  │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐         │
│  │  mm/   │ │  ipc/  │ │ core/  │ │ block/ │ │ sched/ │         │
│  │ Buffer │ │EventBus│ │ Motion │ │UndoTree│ │Runtime │         │
│  │Position│ │ Scope  │ │TextObj │ │  Txn   │ │WorkQue │         │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘         │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                    DRIVERS (server/lib/drivers/)                │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐         │
│  │ input/ │ │syntax/ │ │  lsp/  │ │  vfs/  │ │session/│         │
│  │Keyboard│ │Syntax  │ │  LSP   │ │ Files  │ │Session │         │
│  │ Resolve│ │Driver  │ │ Client │ │  Ops   │ │ State  │         │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘         │
│                                                                 │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐         │
│  │buffer/ │ │  undo/ │ │search/ │ │  ffi/  │ │ffi-py/ │         │
│  │ Ops    │ │ Redo   │ │Replace │ │  FFI   │ │ Python │         │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘         │
│                                                                 │
│  ┌────────┐ ┌────────┐ ┌──────────────┐ ┌───────────────────┐   │
│  │command/│ │cmdtype/│ │  clipboard/  │ │syntax-treesitter/ │   │
│  │Registry│ │ Types  │ │  Clipboard   │ │  Tree-sitter Impl │   │
│  └────────┘ └────────┘ └──────────────┘ └───────────────────┘   │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                        SHARED (shared/)                         │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  protocol/ │ arch/ │ net/ │ log/ │ trace/ │ module-macros │   │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Session Model

reovim uses a tmux-like session model:

```
┌──────────────────────────────────────────────────────────────┐
│  Server                                                       │
│  ├── Session "default"                                        │
│  │   ├── driver::Session (SSOT for editing state)             │
│  │   ├── Kernel (buffers, [events](event-layers.md))           │
│  │   └── Clients: [ClientId(1), ClientId(2)]                  │
│  └── Session "project-a"                                      │
│      └── ...                                                  │
└──────────────────────────────────────────────────────────────┘
```

See: [Session Model](./session-model.md)

## Linux Kernel Mapping

| Linux | Reovim | Purpose |
|-------|--------|---------|
| `arch/` | `shared/arch/` | Platform abstraction (Unix, Windows) |
| `kernel/` | `server/lib/kernel/` | Core mechanisms (no policy) |
| `drivers/` | `server/lib/drivers/*` | Hardware/service adapters |
| `fs/` | `server/lib/drivers/vfs/` | Virtual filesystem |
| Loadable Modules | `server/modules/` | Dynamic policy modules |

## Design Principles

### 1. Mechanism vs Policy

- **Kernel** provides WHAT can be done (service objects via `sys.*`, traits)
- **Modules** decide HOW to do it (keybindings, behavior)

See: [Mechanism vs Policy](./mechanism-vs-policy.md)

### 2. Kernel Purity

- Zero external syntax dependencies in kernel
- Zero logging dependencies (uses internal printk)
- All tree-sitter in plugins, not kernel

### 3. API Boundary

- Modules use ONLY `reovim_kernel::api::*`
- Kernel internals are `pub(crate)` (private)
- Compile-time enforcement

### 4. Driver Abstraction

- Kernel defines traits, drivers implement
- Multiple implementations possible (e.g., different terminals)
- Hot-swappable at runtime

## Crate Dependency Graph

```
shared/arch                ← Platform traits (no deps)
    │
    ▼
server/lib/kernel          ← Core mechanisms (depends on arch)
    │
    ├──▶ server/lib/drivers/*   ← Service adapters
    │
    └──▶ shared/module-macros   ← declare_module! proc-macro
            │
            ▼
        server/lib/server/registry/  ← Module loader, registry
            │
            ▼
        server/modules/              ← Policy modules (vim, motions, etc.)

shared/protocol            ← gRPC v2 definitions
    │
    ├──▶ server/lib/server  ← Server runtime
    │
    └──▶ clients/*          ← TUI, CLI, Web clients
```

## Source Layout

```
server/
├── lib/
│   ├── kernel/              # Core kernel
│   │   └── src/
│   │       ├── api/         # PUBLIC interface
│   │       │   ├── v1.rs    # Stable API re-exports
│   │       │   ├── module.rs # Module trait, registrations
│   │       │   └── context.rs # KernelContext, ModuleContext
│   │       │
│   │       ├── mm/          # Memory management
│   │       │   ├── buffer.rs # Buffer storage
│   │       │   └── position.rs # Position types
│   │       │
│   │       ├── ipc/         # Inter-process communication
│   │       │   └── event_bus.rs # Pub/sub event system
│   │       │
│   │       ├── core/        # Core primitives
│   │       │   ├── motion.rs # Motion types
│   │       │   └── register.rs # Register types
│   │       │
│   │       └── block/       # Block operations
│   │           └── undo.rs  # UndoTree
│   │
│   ├── server/              # Server runtime
│   │   └── src/
│   │       ├── grpc/        # gRPC service handlers (12 services)
│   │       │   ├── input.rs       # InputService
│   │       │   ├── state.rs       # StateService
│   │       │   ├── buffer.rs      # BufferService
│   │       │   ├── editor.rs      # EditorService
│   │       │   ├── notification.rs # NotificationService
│   │       │   ├── server_service.rs # ServerService
│   │       │   ├── debug.rs       # DebugService
│   │       │   ├── module.rs      # ModuleService
│   │       │   ├── command.rs     # CommandService
│   │       │   ├── syntax.rs      # SyntaxService
│   │       │   ├── presence.rs    # PresenceService
│   │       │   ├── extension.rs   # ExtensionService
│   │       │   ├── auth.rs        # Authentication
│   │       │   └── notification_builder.rs # Notification helpers
│   │       ├── session/     # Session management
│   │       └── registry/    # Module loader, registry
│   │
│   └── drivers/             # Driver implementations (14 crates)
│       ├── input/           # Key event parsing
│       ├── syntax/          # Tree-sitter integration
│       ├── lsp/             # LSP client
│       ├── vfs/             # Virtual filesystem
│       ├── session/         # Session state traits
│       ├── buffer/          # Buffer operations
│       ├── undo/            # Undo/redo
│       └── ...              # (search, clipboard, ffi, etc.)
│
└── modules/                 # Policy modules (21 crates)
    ├── vim/                 # Core Vim behavior
    ├── motions/             # Movement commands
    ├── textobjects/         # Text object definitions
    ├── keymap/              # Keymap definitions
    └── ...                  # (editor, options, etc.)

clients/
├── tui/                     # TUI client
│   └── lib/drivers/         # TUI-specific drivers (display, tui)
├── cli/                     # CLI client
└── web/                     # Web client (gRPC-Web, WASM)

shared/
├── protocol/                # gRPC v2 definitions
│   └── proto/               # .proto files
├── arch/                    # Platform abstraction
│   ├── src/unix/            # Unix implementation
│   └── src/windows/         # Windows implementation
├── net/                     # Network transport
├── log/                     # Logging infrastructure
├── trace/                   # Tracing/diagnostics
├── module-macros/           # declare_module! proc-macro
└── testing/                 # Integration test utilities
```

## Client Layer Model

Clients follow a separate layered architecture defined by the
[Client Layer Model](./client/overview.md):

```
┌──────────────────────────────────────────────────────────────┐
│  PLATFORM ADAPTER           Ground truth (TUI, Web, Mobile)  │
├──────────────────────────────────────────────────────────────┤
│  COMMON CLIENT                               Platform-agnostic │
│  ├── CLIENT CORE            Compositor, event dispatch        │
│  ├── CLIENT DRIVER          Trait contracts (ClientModule,    │
│  │                          ViewportRenderer, RenderSurface)  │
│  └── CLIENT MODULE          Policy (statusline, line-numbers) │
└──────────────────────────────────────────────────────────────┘
```

See: [Client Architecture](./client/overview.md) for the full specification.

## Related Documents

- [Session Model](./session-model.md) - tmux-like multi-client architecture
- [Mechanism vs Policy](./mechanism-vs-policy.md) - Core principle
- [Module-Mode Inheritance](./modules/mode-inheritance.md) - Mode system
- [Kernel Subsystems](./kernel/overview.md) - Kernel internals
- [Driver Layer](./drivers/overview.md) - Server driver implementations
- [Module System](./modules/overview.md) - Server modules
- [Server Runtime](./server/overview.md) - gRPC server, sessions
- [Client Architecture](./client/overview.md) - Client Layer Model
