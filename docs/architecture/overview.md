# Architecture Overview

Reovim follows a **Linux kernel-inspired architecture** with clear separation between kernel mechanisms, drivers, and loadable modules.

## Layer Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         MODULES                                 │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │ keymap  │ │ motions │ │operators│ │ layout  │ │ options │   │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘   │
│       │           │           │           │           │         │
│       └───────────┴───────────┼───────────┴───────────┘         │
│                               │                                 │
│                    use reovim_kernel::api::*                    │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                         KERNEL API                              │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  pub mod api { KernelContext, traits, types, module }   │    │
│  └─────────────────────────────────────────────────────────┘    │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                      KERNEL (lib/kernel)                        │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐        │
│  │  mm/   │ │  ipc/  │ │ core/  │ │ block/ │ │ sched/ │        │
│  │ Buffer │ │EventBus│ │ Motion │ │UndoTree│ │Runtime │        │
│  │Position│ │ Scope  │ │TextObj │ │  Txn   │ │WorkQue │        │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘        │
│                                                                 │
│  ┌────────┐ ┌────────┐                                          │
│  │printk/ │ │ debug/ │                                          │
│  │ Logger │ │ Panic  │                                          │
│  └────────┘ └────────┘                                          │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                      DRIVERS (lib/drivers)                      │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐        │
│  │syntax/ │ │ input/ │ │display/│ │  lsp/  │ │  net/  │        │
│  │Syntax  │ │Keyboard│ │ Frame  │ │  LSP   │ │  RPC   │        │
│  │Driver  │ │ Mouse  │ │Composit│ │ Client │ │ Server │        │
│  └────────┘ └────────┘ └────────┘ └────────┘ └────────┘        │
│                                                                 │
│  ┌────────┐ ┌────────┐                                          │
│  │  vfs/  │ │  log/  │                                          │
│  │  VFS   │ │Tracing │                                          │
│  └────────┘ └────────┘                                          │
└───────────────────────────────┼─────────────────────────────────┘
                                │
┌───────────────────────────────┼─────────────────────────────────┐
│                        ARCH (lib/arch)                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Platform Traits: Terminal, FileSystem, Process, Time   │    │
│  ├─────────────────────────────────────────────────────────┤    │
│  │  unix/    │  windows/   │  (future: wasm/, embedded/)   │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Linux Kernel Mapping

| Linux | Reovim | Purpose |
|-------|--------|---------|
| `arch/` | `lib/arch/` | Platform abstraction (Unix, Windows) |
| `kernel/` | `lib/kernel/` | Core mechanisms (no policy) |
| `drivers/` | `lib/drivers/*` | Hardware/service adapters |
| `fs/` | `lib/drivers/vfs/` | Virtual filesystem |
| Loadable Modules | `modules/`, `plugins/` | Dynamic policy modules |

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
lib/arch          ← Platform traits (no deps)
    │
    ▼
lib/kernel        ← Core mechanisms (depends on arch)
    │
    ├──▶ lib/drivers/*   ← Service adapters
    │
    └──▶ lib/module-macros  ← declare_module! proc-macro
            │
            ▼
        runner/src/module/  ← Module loader, registry
            │
            ▼
        modules/            ← Policy modules (keymap, motions, etc.)
        plugins/            ← Feature plugins (treesitter, lsp, etc.)
```

## Source Layout

```
lib/
├── arch/                    # Platform abstraction
│   ├── src/traits.rs        # Terminal, FileSystem, Process traits
│   ├── src/unix/            # Unix implementation
│   └── src/windows/         # Windows implementation
│
├── kernel/                  # Core kernel
│   └── src/
│       ├── api/             # PUBLIC interface
│       │   ├── v1.rs        # Stable API re-exports
│       │   ├── module.rs    # Module trait, registrations
│       │   ├── context.rs   # KernelContext, ModuleContext
│       │   └── version.rs   # Version types
│       │
│       ├── mm/              # Memory management
│       │   ├── buffer.rs    # Buffer storage
│       │   ├── position.rs  # Position types
│       │   └── edit.rs      # Edit operations
│       │
│       ├── ipc/             # Inter-process communication
│       │   ├── event_bus.rs # Pub/sub event system
│       │   └── scope.rs     # EventScope for sync
│       │
│       ├── core/            # Core primitives
│       │   ├── motion.rs    # Motion types
│       │   └── textobject.rs# TextObject types
│       │
│       ├── block/           # Block operations
│       │   ├── undo.rs      # UndoTree
│       │   └── transaction.rs
│       │
│       ├── sched/           # Scheduler
│       │   ├── runtime.rs   # Event loop
│       │   └── workqueue.rs # Async tasks
│       │
│       └── printk/          # Kernel logging
│           └── logger.rs    # Logger trait
│
├── drivers/                 # Driver implementations
│   ├── syntax/              # SyntaxDriver trait
│   ├── input/               # InputDriver trait
│   ├── display/             # DisplayDriver trait
│   ├── lsp/                 # LSP client types
│   ├── net/                 # RPC server
│   ├── vfs/                 # Virtual filesystem
│   └── log/                 # Tracing logger
│
└── module-macros/           # Proc-macro crate
    └── src/lib.rs           # declare_module!

runner/
└── src/
    └── module/              # Module system (runner layer)
        ├── loader.rs        # Static + dynamic loading
        ├── registry.rs      # Dependency resolution
        ├── handle.rs        # FFI trampolines
        └── hot_reload.rs    # File watching
```

## Related Documents

- [Mechanism vs Policy](./mechanism-vs-policy.md) - Core principle
- [Module-Mode Inheritance](./module-mode-inheritance.md) - Mode system
- [Kernel Subsystems](./kernel.md) - Kernel internals
- [Driver Layer](./drivers.md) - Driver implementations
- [Module System](./modules.md) - Dynamic modules
