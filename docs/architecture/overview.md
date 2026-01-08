# Architecture Overview

This document provides a high-level overview of the reovim editor architecture.

## Design Goals

- **Fastest-reaction editor**: Minimal latency and instant response to user input
- **Scalability**: Architecture designed to handle large files and complex operations
- **Async-first**: Non-blocking I/O using tokio runtime

## Workspace Structure

```
reovim/
├── runner/                 # Binary crate - editor entry point
├── lib/core/               # reovim-core - core editor logic
├── lib/sys/                # reovim-sys - terminal abstraction (crossterm)
├── plugins/features/       # External feature plugins
│   ├── completion/         # reovim-plugin-completion
│   ├── explorer/           # reovim-plugin-explorer
│   ├── health-check/       # reovim-plugin-health-check
│   ├── lsp/                # reovim-plugin-lsp
│   ├── microscope/         # reovim-plugin-microscope (fuzzy finder)
│   ├── notification/       # reovim-plugin-notification
│   ├── pair/               # reovim-plugin-pair
│   ├── pickers/            # reovim-plugin-pickers
│   ├── profiles/           # reovim-plugin-profiles
│   ├── range-finder/       # reovim-plugin-range-finder (jump/fold)
│   ├── settings-menu/      # reovim-plugin-settings-menu
│   ├── statusline/         # reovim-plugin-statusline
│   ├── treesitter/         # reovim-plugin-treesitter
│   └── which-key/          # reovim-plugin-which-key
├── plugins/languages/      # Language plugins
│   ├── rust/               # reovim-lang-rust
│   ├── c/                  # reovim-lang-c
│   ├── javascript/         # reovim-lang-javascript
│   ├── python/             # reovim-lang-python
│   ├── json/               # reovim-lang-json
│   ├── toml/               # reovim-lang-toml
│   └── markdown/           # reovim-lang-markdown
└── tools/
    ├── perf-report/        # Performance report generator
    ├── reo-cli/            # CLI client for server mode
    └── bench/              # Performance benchmarks (criterion)
```

## Dependency Graph

```
┌──────────────────────────────────────────────────────────────┐
│                          RUNNER                               │
│                         (reovim)                              │
└──────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│    CORE      │◄──│ Feature Plugins  │   │ Language Plugins │
│(reovim-core) │   │ (range-finder,   │   │ (rust, c, js,    │
└──────────────┘   │ microscope, lsp) │   │  python, etc.)   │
        │          └──────────────────┘   └──────────────────┘
        ▼
┌──────────────┐
│     SYS      │
│(reovim-sys)  │
└──────────────┘
        │
        ▼
   crossterm
```

## Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `runner` | Bootstrap editor, parse CLI args, configure plugins (AllPlugins), invoke runtime |
| `reovim-core` | Runtime, buffers, events, screen, commands, plugin system, DefaultPlugins |
| `reovim-sys` | Re-exports crossterm for terminal I/O |
| `reovim-plugin-*` | External feature plugins (range-finder, microscope, lsp, completion, explorer, etc.) |
| `reovim-lang-*` | Language support plugins (syntax highlighting, queries) |

## Core Architecture Pattern

The editor follows a **central runtime event loop** pattern with async tokio tasks:

```
main.rs
  │
  ▼
Runtime::init() ──────────────────────────────────┐
  │                                               │
  ├── Screen (terminal output)                    │
  ├── Buffers (text storage)                      │
  ├── CommandRegistry (trait-based commands)      │
  ├── dual mpsc channels (hi/lo priority)         │
  ├── watch channel (ModeState broadcast)         │
  │                                               │
  └── spawned async tasks:                        │
      ├── InputEventBroker (reads terminal)       │
      ├── KeyEventBroker (broadcasts keys)        │
      ├── CommandHandler (keys → commands)        │
      ├── CompletionHandler (async completion)    │
      └── TerminateHandler (Ctrl+C)               │
                                                  │
      ◄─────────── event loop ────────────────────┘
```

## Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime |
| `crossterm` | Terminal I/O |
| `futures` | Async utilities |
| `nucleo` | Fuzzy matching (for microscope) |
| `parking_lot` | Synchronization primitives |
| `tree-sitter` | Incremental parsing |

## Related Documentation

- [Runtime](./runtime.md) - Central event loop and state
- [Buffer](./buffer.md) - Text storage and cursor
- [Screen](./screen.md) - Terminal rendering
- [Plugins](./plugins.md) - Plugin system
- [Features](./features.md) - Feature modules
