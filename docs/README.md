# Reovim Documentation

Welcome to the reovim documentation. This guide is organized by topic for easy navigation.

## Quick Links

- [Architecture Overview](./architecture/overview.md) - System design and structure
- [Plugin Tutorial](./plugins/tutorial.md) - Create your first plugin
- [Configuration Guide](./guides/configuration.md) - Editor settings
- [Troubleshooting](./guides/troubleshooting.md) - Common issues and solutions

## Documentation Structure

### [Architecture](./architecture/)

Core system design and components.

| Document | Description |
|----------|-------------|
| [Overview](./architecture/overview.md) | High-level architecture |
| [Runtime](./architecture/runtime.md) | Central event loop |
| [Buffer](./architecture/buffer.md) | Text storage and syntax |
| [Screen](./architecture/screen.md) | Terminal rendering |
| [Plugins](./architecture/plugins.md) | Plugin system architecture |
| [Features](./architecture/features.md) | Feature modules |

### [Events](./events/)

Event system and communication.

| Document | Description |
|----------|-------------|
| [Overview](./events/overview.md) | Event system overview |
| [Event Bus](./events/eventbus.md) | Type-erased event system |
| [Runtime Events](./events/runtime-events.md) | RuntimeEventPayload |
| [Patterns](./events/patterns.md) | Event design patterns |
| [Handlers](./events/handlers.md) | Event handlers |
| [Keybindings](./events/keybindings.md) | Key binding system |

### [Rendering](./rendering/)

Render pipeline and plugin UI systems.

| Document | Description |
|----------|-------------|
| [Overview](./rendering/overview.md) | Rendering system overview |
| [Pipeline](./rendering/pipeline.md) | Data transformation stages |
| [UI Systems](./rendering/ui-systems.md) | Plugin UI rendering |
| [Custom Stages](./rendering/custom-stages.md) | Extending the pipeline |

### [Plugins](./plugins/)

Plugin development guides.

| Document | Description |
|----------|-------------|
| [Overview](./plugins/overview.md) | Plugin development overview |
| [Tutorial](./plugins/tutorial.md) | Beginner guide |
| [Advanced](./plugins/advanced.md) | Advanced patterns |
| [System](./plugins/system.md) | Full API reference |

### [Guides](./guides/)

User and developer guides.

| Document | Description |
|----------|-------------|
| [Configuration](./guides/configuration.md) | Editor settings |
| [Troubleshooting](./guides/troubleshooting.md) | Common issues |
| [Development](./guides/development.md) | Development setup |
| [Testing](./guides/testing.md) | Testing guide |

### [Reference](./reference/)

API and command reference.

| Document | Description |
|----------|-------------|
| [Commands](./reference/commands.md) | Command system |
| [Server Mode](./reference/server-mode.md) | RPC server |
| [Text Objects](./reference/text-objects.md) | Vim text objects |

### [Features](./features/)

Feature-specific documentation.

| Document | Description |
|----------|-------------|
| [Syntax Highlighting](./features/syntax-highlighting.md) | Treesitter integration |
| [Saturator](./features/saturator.md) | Background processing |
| [Color System](./features/color-system.md) | Color handling |
| [Decoration System](./features/decoration-system.md) | Language decorations |
| [Animation System](./features/animation-system.md) | Visual effects |
| [Diagnostics](./features/diagnostics.md) | LSP diagnostics |
| [Window/Buffer](./features/window-buffer.md) | Window management |

## Suggested Reading Order

### For Users

1. [Configuration](./guides/configuration.md) - Set up your editor
2. [Troubleshooting](./guides/troubleshooting.md) - Fix common issues

### For Plugin Developers

1. [Plugin Tutorial](./plugins/tutorial.md) - Create your first plugin
2. [Event Patterns](./events/patterns.md) - Design plugin events
3. [UI Systems](./rendering/ui-systems.md) - Add plugin UI
4. [Plugin Advanced](./plugins/advanced.md) - Advanced techniques

### For Core Contributors

1. [Architecture Overview](./architecture/overview.md) - System design
2. [Runtime](./architecture/runtime.md) - Event loop
3. [Events Overview](./events/overview.md) - Event flow
4. [Development](./guides/development.md) - Build and test
