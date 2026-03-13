# Reovim Documentation

Welcome to the Reovim documentation. This guide covers the **Linux kernel-inspired architecture** (v0.9.0+).

## Quick Links

| Topic | Link | Description |
|-------|------|-------------|
| Architecture | [architecture/overview.md](./architecture/overview.md) | System design, layer diagram |
| Client Architecture | [architecture/client/overview.md](./architecture/client/overview.md) | Client Layer Model |
| Contributing | [contributing/overview.md](./contributing/overview.md) | Contributor guides |
| User Guide | [user-guide/](./user-guide/) | Configuration, commands |
| Philosophy | [contributing/philosophy/mechanism-vs-policy.md](./contributing/philosophy/mechanism-vs-policy.md) | Core design principle |

## Documentation Structure

```
docs/
├── architecture/    # Component architecture
│   ├── overview.md
│   ├── kernel/      # Core mechanisms (server)
│   ├── drivers/     # Server drivers
│   ├── modules/     # Server modules
│   ├── server/      # Server runtime
│   └── client/      # Client Layer Model
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
| [Drivers](./architecture/drivers/overview.md) | Server drivers: syntax, input, display, LSP |
| [Modules](./architecture/modules/overview.md) | Server modules: keymap, operators |
| [Server](./architecture/server/overview.md) | Server runtime: gRPC, sessions |
| [Client](./architecture/client/overview.md) | Client Layer Model: platform, rendering |

#### Kernel Subsystems

| Document | Description |
|----------|-------------|
| [api/](./architecture/kernel/api/overview.md) | Public API surface |
| [mm/](./architecture/kernel/mm/overview.md) | Memory management (Buffer, Position) |
| [ipc/](./architecture/kernel/ipc/overview.md) | EventBus, channels |
| [core/](./architecture/kernel/core/overview.md) | Motion, TextObject, Mode |
| [block/](./architecture/kernel/block/overview.md) | UndoTree, transactions |
| [sched/](./architecture/kernel/sched/overview.md) | Runtime, WorkQueue |

#### Server Drivers

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

#### Server Modules

| Document | Description |
|----------|-------------|
| [Mode Inheritance](./architecture/modules/mode-inheritance.md) | Mode system |
| [Extension Contracts](./architecture/modules/extension-contracts.md) | Server-client pairing |
| [Built-in Modules](./architecture/modules/builtin/README.md) | Editor, keymap, operators |

#### Server Runtime

| Document | Description |
|----------|-------------|
| [Server Overview](./architecture/server/overview.md) | RPC server components |
| [Sessions & Viewports](./architecture/server/sessions.md) | Multi-client architecture |
| [Transport](./architecture/server/transport.md) | gRPC, TCP, Unix socket |
| [gRPC Protocol](./architecture/server/rpc-protocol.md) | gRPC v2 protocol spec |
| [Notifications](./architecture/server/notifications.md) | Client notifications |

#### Client Architecture

| Document | Description |
|----------|-------------|
| [Client Overview](./architecture/client/overview.md) | Client Layer Model |
| [Principles](./architecture/client/principles.md) | Semantic, Presence, Adoption, Specialization |
| [Layers](./architecture/client/layers.md) | Layer diagram, rules, dependency graph |
| [Platform](./architecture/client/platform.md) | PlatformCapabilities, RenderSurface |
| [Module](./architecture/client/module.md) | ClientModule trait, lifecycle |
| [Rendering](./architecture/client/rendering.md) | Compositor, ViewportRenderer |
| [Data Flow](./architecture/client/data-flow.md) | Event routing, ServerHandle |
| [Types](./architecture/client/types.md) | All type definitions |
| [Extensibility](./architecture/client/extensibility.md) | Cargo crate model |
| [Examples](./architecture/client/examples.md) | Worked examples |
| [Gaps](./architecture/client/gaps.md) | Known gaps, migration strategy |
| [TUI](./architecture/client/tui.md) | Terminal user interface |
| [CLI](./architecture/client/cli.md) | Command-line interface |

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
| [Client Extensions](./heritage/client-extensions.md) | Pre-CLM client extension taxonomy (superseded) |
| [Runner Overview](./heritage/runner-overview.md) | Pre-Phase 8 runner architecture (superseded) |

---

## Suggested Reading Order

### For Module Developers

1. [Mechanism vs Policy](./contributing/philosophy/mechanism-vs-policy.md) - Understand the design
2. [Module System](./architecture/modules/overview.md) - Server module architecture
3. [Module Development](./contributing/guides/module-development.md) - Create modules
4. [Kernel API](./architecture/kernel/api/overview.md) - Available APIs

### For Client Developers

1. [Client Architecture](./architecture/client/overview.md) - Client Layer Model
2. [Principles](./architecture/client/principles.md) - Design philosophy
3. [Layers](./architecture/client/layers.md) - Layer diagram and rules
4. [Module](./architecture/client/module.md) - ClientModule trait
5. [Examples](./architecture/client/examples.md) - Worked examples

### For Core Contributors

1. [Architecture Overview](./architecture/overview.md) - System design
2. [Kernel Overview](./architecture/kernel/overview.md) - Kernel subsystems
3. [Driver Overview](./architecture/drivers/overview.md) - Driver layer
4. [Server Overview](./architecture/server/overview.md) - Server runtime
5. [Client Overview](./architecture/client/overview.md) - Client architecture
6. [Testing](./contributing/guides/testing.md) - Testing guide

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
