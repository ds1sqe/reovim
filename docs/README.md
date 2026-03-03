# Reovim Documentation

Welcome to the Reovim documentation. This guide covers the **Linux kernel-inspired architecture** (v0.9.0+).

## Quick Links

| Topic | Link | Description |
|-------|------|-------------|
| Architecture | [architecture/overview.md](./architecture/overview.md) | System design, layer diagram |
| Contributing | [contributing/overview.md](./contributing/overview.md) | Contributor guides |
| User Guide | [user-guide/](./user-guide/) | Configuration, commands |
| Philosophy | [contributing/philosophy/mechanism-vs-policy.md](./contributing/philosophy/mechanism-vs-policy.md) | Core design principle |

## Documentation Structure

```
docs/
├── architecture/    # Component architecture
│   ├── overview.md
│   ├── kernel/      # Core mechanisms
│   ├── drivers/     # Service layer
│   ├── modules/     # Policy layer
│   └── runner/      # Application layer
├── contributing/    # Contributor documentation
│   ├── philosophy/  # Design principles
│   ├── guides/      # How-to guides
│   └── internals/   # Deep technical docs
├── user-guide/      # End-user documentation
└── heritage/        # Foundational documents
```

---

### [Architecture](./architecture/)

Component architecture documentation.

| Document | Description |
|----------|-------------|
| [Overview](./architecture/overview.md) | Layer diagram, crate deps |
| [Kernel](./architecture/kernel/overview.md) | Core mechanisms: mm/, ipc/, core/, block/ |
| [Drivers](./architecture/drivers/overview.md) | Services: syntax, input, display, LSP |
| [Modules](./architecture/modules/overview.md) | Policy modules: keymap, operators |
| [Runner](./architecture/runner/overview.md) | Application: server, client |

#### Kernel Subsystems

| Document | Description |
|----------|-------------|
| [api/](./architecture/kernel/api/overview.md) | Public API surface |
| [mm/](./architecture/kernel/mm/overview.md) | Memory management (Buffer, Position) |
| [ipc/](./architecture/kernel/ipc/overview.md) | EventBus, channels |
| [core/](./architecture/kernel/core/overview.md) | Motion, TextObject, Mode |
| [block/](./architecture/kernel/block/overview.md) | UndoTree, transactions |
| [sched/](./architecture/kernel/sched/overview.md) | Runtime, WorkQueue |

#### Driver Layer

| Document | Description |
|----------|-------------|
| [command/](./architecture/drivers/command/overview.md) | Command traits |
| [display/](./architecture/drivers/display/overview.md) | Frame buffer, compositor |
| [input/](./architecture/drivers/input/overview.md) | Keyboard, mouse |
| [syntax/](./architecture/drivers/syntax/overview.md) | Syntax highlighting |
| [lsp/](./architecture/drivers/lsp/overview.md) | LSP client |
| [net/](./architecture/drivers/net/overview.md) | Network, RPC |
| [vfs/](./architecture/drivers/vfs/overview.md) | Virtual filesystem |
| [log/](./architecture/drivers/log/overview.md) | Logging (tracing) |

#### Module System

| Document | Description |
|----------|-------------|
| [Mode Inheritance](./architecture/modules/mode-inheritance.md) | Mode system |
| [Built-in Modules](./architecture/modules/builtin/README.md) | Editor, keymap, operators |

#### Runner (Server/Client)

| Document | Description |
|----------|-------------|
| [Server Overview](./architecture/runner/server/overview.md) | RPC server components |
| [Sessions & Viewports](./architecture/runner/server/sessions.md) | Multi-client architecture |
| [Transport](./architecture/runner/server/transport.md) | gRPC, TCP, Unix socket |
| [gRPC Protocol](./architecture/runner/server/rpc-protocol.md) | gRPC v2 protocol spec |
| [Notifications](./architecture/runner/server/notifications.md) | Client notifications |
| [Client Overview](./architecture/runner/client/overview.md) | CLI/TUI clients |
| [CLI](./architecture/runner/client/cli.md) | Command-line interface |
| [TUI](./architecture/runner/client/tui.md) | Terminal user interface |

---

### [Contributing](./contributing/)

Contributor documentation.

| Document | Description |
|----------|-------------|
| [Overview](./contributing/overview.md) | Contributor entry point |
| [Getting Started](./contributing/getting-started.md) | Development setup |

#### Philosophy

| Document | Description |
|----------|-------------|
| [Mechanism vs Policy](./contributing/philosophy/mechanism-vs-policy.md) | Kernel = WHAT, Modules = HOW |
| [Linux Architecture](./contributing/philosophy/linux-architecture.md) | Linux kernel design mapping |

#### Guides

| Document | Description |
|----------|-------------|
| [Module Development](./contributing/guides/module-development.md) | Creating modules |
| [Testing](./contributing/guides/testing.md) | Testing guide and patterns |

#### Internals

| Document | Description |
|----------|-------------|
| [Concurrency](./contributing/internals/concurrency.md) | Lock patterns, async |

---

### [User Guide](./user-guide/)

End-user documentation.

| Document | Description |
|----------|-------------|
| [Configuration](./user-guide/configuration.md) | Editor settings |
| [Commands](./user-guide/commands.md) | Command reference |
| [Text Objects](./user-guide/text-objects.md) | Vim text objects |
| [Server Mode](./user-guide/server-mode.md) | RPC server usage |
| [Troubleshooting](./user-guide/troubleshooting.md) | Common issues |

---

### [Heritage](./heritage/)

Foundational documents that shaped Reovim's architecture.

| Document | Description |
|----------|-------------|
| [Project Kernel Phases](./heritage/project-kernel-phases.md) | Epic #150 phase-by-phase journey |
| [Legacy Memorial](./heritage/legacy-memorial.md) | What we studied and learned from v0.8.x |
| [Clean Architecture Proposal](./heritage/clean-architecture-proposal.md) | The blueprint for kernel/drivers/modules |
| [Phase 5 Extraction](./heritage/phase5-extraction.md) | How we transitioned v0.8.x → v0.9.0 |
| [Buffer Provider Proposal](./heritage/buffer-provider-proposal.md) | Deferred for future consideration |

---

## Suggested Reading Order

### For Module Developers

1. [Mechanism vs Policy](./contributing/philosophy/mechanism-vs-policy.md) - Understand the design
2. [Module System](./architecture/modules/overview.md) - Module architecture
3. [Module Development](./contributing/guides/module-development.md) - Create modules
4. [Kernel API](./architecture/kernel/api/overview.md) - Available APIs

### For Core Contributors

1. [Architecture Overview](./architecture/overview.md) - System design
2. [Kernel Overview](./architecture/kernel/overview.md) - Kernel subsystems
3. [Driver Overview](./architecture/drivers/overview.md) - Driver layer
4. [Runner Overview](./architecture/runner/overview.md) - Application layer
5. [Testing](./contributing/guides/testing.md) - Testing guide

### For Users

1. [Configuration](./user-guide/configuration.md) - Editor settings
2. [Server Mode](./user-guide/server-mode.md) - RPC usage
3. [Troubleshooting](./user-guide/troubleshooting.md) - Common issues

---

## Version Notes

| Version | Architecture | Documentation |
|---------|--------------|---------------|
| v0.9.0+ | `lib/kernel` + `lib/drivers` + modules | This documentation |
| v0.8.x | `lib/core` (legacy) | See `archive/docs/` |

Legacy documentation has been moved to `archive/docs/` for historical reference.
