# Reovim Documentation

Welcome to the Reovim documentation. This guide covers the new **Linux kernel-inspired architecture**.

## Quick Links

- [Architecture Overview](./architecture/overview.md) - System design and layer diagram
- [Mechanism vs Policy](./architecture/mechanism-vs-policy.md) - Core design principle
- [Module System](./architecture/modules.md) - Dynamic module loading
- [Configuration Guide](./guides/configuration.md) - Editor settings

## Documentation Structure

### [Architecture](./architecture/)

New kernel-based architecture (v0.9.0+).

| Document | Description |
|----------|-------------|
| [Overview](./architecture/overview.md) | Layer diagram, Linux mapping, crate deps |
| [Kernel](./architecture/kernel.md) | mm/, ipc/, core/, block/, sched/, api/ |
| [Drivers](./architecture/drivers.md) | syntax/, input/, display/, lsp/, net/, vfs/ |
| [Modules](./architecture/modules.md) | Module trait, FFI, loader, registry |
| [Mechanism vs Policy](./architecture/mechanism-vs-policy.md) | Kernel = WHAT, Modules = HOW |
| [Module-Mode Inheritance](./architecture/module-mode-inheritance.md) | Mode system with inheritance |
| [Clean Architecture Proposal](./architecture/clean-architecture-proposal.md) | Original proposal |

### [Guides](./guides/)

User and developer guides.

| Document | Description |
|----------|-------------|
| [Configuration](./guides/configuration.md) | Editor settings |
| [Development](./guides/development.md) | Development setup |
| [Testing](./guides/testing.md) | Testing guide |
| [Troubleshooting](./guides/troubleshooting.md) | Common issues |

### [Reference](./reference/)

API and command reference.

| Document | Description |
|----------|-------------|
| [Commands](./reference/commands.md) | Command system |
| [Server Mode](./reference/server-mode.md) | RPC server |
| [Text Objects](./reference/text-objects.md) | Vim text objects |

### [Archive](./archive/)

Legacy documentation for `lib/core` (v0.8.x). These will be removed after Phase 6.

## Suggested Reading Order

### For Module Developers

1. [Architecture Overview](./architecture/overview.md) - Understand the layers
2. [Mechanism vs Policy](./architecture/mechanism-vs-policy.md) - Design principle
3. [Module System](./architecture/modules.md) - Create modules
4. [Module-Mode Inheritance](./architecture/module-mode-inheritance.md) - Mode handling

### For Core Contributors

1. [Architecture Overview](./architecture/overview.md) - System design
2. [Kernel Subsystems](./architecture/kernel.md) - Internal structure
3. [Driver Layer](./architecture/drivers.md) - Driver implementations
4. [Development Guide](./guides/development.md) - Build and test

### For Users

1. [Configuration](./guides/configuration.md) - Set up your editor
2. [Troubleshooting](./guides/troubleshooting.md) - Fix common issues

## Version Notes

| Version | Architecture | Documentation |
|---------|--------------|---------------|
| v0.9.0+ | `lib/kernel` + `lib/drivers` + modules | This documentation |
| v0.8.x | `lib/core` (legacy) | See `archive/` |
