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
│  │BufferId│ │EventBus│ │  Mode  │ │ByteEdit│ │Runtime │         │
│  │WindId  │ │ Scope  │ │ Config │ │Storage │ │WorkQue │         │
│  │ TabId  │ │        │ │        │ │  Ops   │ │        │         │
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
│              PROVIDERS (server/lib/providers/)                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  text/   (reovim-provider-text)                         │    │
│  │  High-level text services built on kernel + drivers      │    │
│  └─────────────────────────────────────────────────────────┘    │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                        SHARED (shared/)                         │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  protocol/ │ arch/ │ net/ │ log/ │ trace/ │ module-macros │   │
│  │  domain/ │ domains/text/                                │    │
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
| `fs/` | `server/lib/subsys/vfs/` | Virtual filesystem |
| Subsystem libraries | `server/lib/providers/*` | High-level domain services |
| Loadable Modules | `server/modules/` | Dynamic policy modules |

## Design Principles

### 1. Mechanism vs Policy

- **Kernel** provides WHAT can be done (service objects via `sys.*`, traits)
- **Modules** decide HOW to do it (keybindings, behavior)

See: [Mechanism vs Policy](./mechanism-policy/README.md)

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

### 5. Provider Layer

`server/lib/providers/` sits between drivers and modules. Providers compose kernel
primitives and driver services into higher-level, domain-oriented APIs (e.g.,
`reovim-provider-text` for text operations). Modules consume providers rather than
accessing drivers directly when a provider already wraps the needed functionality.

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
│   │       │   ├── module/  # Module trait, registrations
│   │       │   ├── context.rs # KernelContext, ModuleContext
│   │       │   └── service.rs # ServiceRegistry
│   │       │
│   │       ├── mm/          # Memory management (ID types)
│   │       │   ├── buffer_id.rs # BufferId newtype
│   │       │   ├── window_id.rs # WindowId newtype
│   │       │   ├── tab_id.rs    # TabId newtype
│   │       │   └── saturator.rs # SaturatorHandle (work-queue)
│   │       │
│   │       ├── ipc/         # Inter-process communication
│   │       │   ├── event_bus/ # Pub/sub event system
│   │       │   └── events/    # Kernel event definitions
│   │       │
│   │       ├── core/        # Core primitives
│   │       │   ├── mode.rs  # Mode, ModeId, ModeStack, CommandId
│   │       │   ├── config.rs # Config, ConfigValue
│   │       │   └── option/  # Editor option types
│   │       │
│   │       ├── block/       # Block-level storage
│   │       │   ├── byte_edit.rs     # ByteEdit operations
│   │       │   ├── byte_undo_log.rs # Byte-level undo log
│   │       │   └── storage_ops.rs   # StorageOps trait
│   │       │
│   │       ├── sched/       # Scheduler
│   │       ├── debug/       # Metrics, profiler, tracing
│   │       ├── printk/      # Kernel logging (pr_err!, etc.)
│   │       └── panic/       # Panic handling and recovery
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
│   │       └── registry/    # Mode, keymap, command registries
│   │
│   ├── drivers/             # Driver implementations (27 crates)
│   │   ├── input/           # Key event parsing
│   │   ├── syntax/          # Tree-sitter integration
│   │   ├── lsp/             # LSP client
│   │   ├── vfs/             # Virtual filesystem
│   │   ├── session/         # Session state traits
│   │   ├── buffer/          # Buffer operations
│   │   ├── codec/           # Codec framework
│   │   ├── module-loader/   # Dynamic module loading
│   │   └── ...              # (undo, search, clipboard, ffi, git, etc.)
│   │
│   └── providers/           # Provider layer (high-level services)
│       └── text/            # reovim-provider-text: text services on kernel+drivers
│
└── modules/                 # Policy modules (73 crates)
    ├── vim/                 # Core Vim behavior
    ├── motions/             # Movement commands
    ├── textobjects/         # Text object definitions
    ├── keymap/              # Keymap definitions
    ├── codec-*/             # Codec modules (9)
    └── ...                  # (editor, options, git, lsp, etc.)

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
├── testing/                 # Integration test utilities
├── capabilities/            # Capability definitions
├── depgraph/                # Dependency graph utilities
├── domain/                  # reovim-domain: core domain abstractions
├── domains/                 # Domain type families
│   └── text/                # reovim-domain-text: Text domain (Buffer, Position, Motion, TextObject)
└── clients/                 # Client shared libraries
    ├── model/               # reovim-client-model: client model types
    └── driver/              # reovim-client-driver: client driver traits
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
- [Mechanism vs Policy](./mechanism-policy/README.md) - Core design principle
- [Server/Client Split](./server-client-split.md) - Data/presentation separation
- [Event Layers](./event-layers.md) - Kernel, domain, and streaming events
- [Type Layers](./type-layers.md) - Context type hierarchy
- [Module-Mode Inheritance](./modules/mode-inheritance.md) - Mode system
- [Kernel Subsystems](./kernel/overview.md) - Kernel internals
- [Driver Layer](./drivers/overview.md) - Server driver implementations
- [Module System](./modules/overview.md) - Server modules
- [Server Runtime](./server/overview.md) - gRPC server, sessions
- [Client Architecture](./client/overview.md) - Client Layer Model
