# reovim

[![CI](https://github.com/ds1sqe/reovim/actions/workflows/ci.yml/badge.svg)](https://github.com/ds1sqe/reovim/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/ds1sqe/reovim/branch/develop/graph/badge.svg)](https://codecov.io/gh/ds1sqe/reovim)
[![Crates.io](https://img.shields.io/crates/v/reovim.svg)](https://crates.io/crates/reovim)

A Rust-powered neovim-like text editor.

## Project Goals

- **Fastest-reaction editor**: Minimal latency, instant response
- **Scalability**: Handles large files and complex operations
- **Zero-warning policy**: All code must compile warning-free

## Features

### Core Editing
- Modal editing (Normal, Insert, Visual, Command modes)
- Vim-style keybindings (h/j/k/l navigation, w/b word motions)
- Operators with motions (`d`, `y`, `c` + motion)
- Visual selection with yank/delete
- Undo/Redo (`u`, `Ctrl-r`)
- Registers for copy/paste

### Navigation
- **Range-Finder plugin** - Unified jump navigation and code visibility
  - **Multi-char jump** (`s` + 2 chars) - Leap-style navigation with smart auto-jump
  - **Enhanced f/t motions** (`f`/`F`/`t`/`T`) - Single-char search with label selection
  - **Code folding** (`za`/`zo`/`zc`/`zR`/`zM`) - Collapse/expand code blocks
- **Microscope fuzzy finder** - Files, buffers, grep (`Space f`)
- **Explorer file browser** - Tree view (`Space e`)
- Jump list navigation (`Ctrl-O`, `Ctrl-I`)

### UI
- Command-line mode (`:q`, `:w`, `:wq`, `:set`)
- Line numbers (absolute, relative, hybrid)
- **Sign column** - Configurable modes: `auto`, `yes`, `no`, `number`
- **Virtual text** - Inline diagnostic messages after line content
- Which-key popup for keybindings
- Completion engine (`Ctrl-Space`)
- Status line with mode, pending keys, last command
- **Health check** (`:health`) - Diagnostic system for core and plugin status
- Landing page when started without a file
- **Mouse support** - Click to position cursor, scroll wheel navigation

### Customization
- **Theme overrides** - TOML-based color customization
  ```toml
  [editor.theme_overrides]
  "statusline.background" = { bg = "#1a1b26" }
  "gutter.line_number" = { fg = "#565f89" }
  "statusline.mode.normal" = { fg = "#1a1b26", bg = "#7aa2f7", bold = true }
  ```
- Supports hex colors, `rgb()`, `ansi:N`, and named colors

### Syntax & Code Intelligence
- **Treesitter syntax highlighting** - Accurate parsing for Rust, C, JavaScript, Python, JSON, TOML, Markdown
- **Semantic text objects** - `af`/`if` (function), `ac`/`ic` (class/struct)

### Performance

Kernel-level benchmarks (v0.14.0):

| Operation | 10 lines | 1k lines | 10k lines |
|-----------|----------|----------|-----------|
| Insert char | 353 ns | 415 ns | 531 ns |
| Delete char | 441 ns | 510 ns | 690 ns |
| Buffer from string | 658 ns | 99 µs | 1.06 ms |
| Event dispatch (5 handlers) | 247 ns | — | — |

- **Zero flickering** - Diff-based rendering sends only changed cells
- **Async architecture** with tokio runtime
- **Cross-platform** terminal support via crossterm

See [perf/](./perf/) for full versioned benchmark reports.

## Installation

```bash
git clone https://github.com/ds1sqe/reovim.git
cd reovim
cargo build --release
```

The binary is at `target/release/reovim`.

## Usage

```bash
reovim
```

This launches in **integrated mode**: an embedded gRPC server + interactive TUI in one process. Open files with `:e <file>` from within the editor.

**Global Options:**
| Flag | Description |
|------|-------------|
| `-v`, `--verbose` | Enable debug-level logging |
| `--log <PATH>` | Custom log file path |

### Server Mode

Reovim can run as a gRPC server for programmatic control, enabling integration with external tools, IDEs, and automation scripts. The server accepts multiple client connections with independent viewports and cursors.

```bash
# Start server (TCP with auto port fallback)
reovim server

# gRPC transport (recommended for CLI/TUI clients)
reovim server --grpc 12540

# Specific TCP port
reovim server --tcp 12521

# Unix socket
reovim server --socket /tmp/reovim.sock
```

**Server Options:**
| Flag | Description |
|------|-------------|
| `--grpc <PORT>` | gRPC transport (recommended for multi-client use) |
| `--tcp <PORT>` | TCP transport |
| `--socket <PATH>` | Unix domain socket |
| `--session <NAME>` | Default session name (default: "main") |
| `--instance <NAME>` | Instance name for discovery (default: "default") |

When no transport flag is given, the server binds TCP with automatic port fallback.

#### CLI Client

The built-in CLI client provides command-line access to running servers:

```bash
# Inject keys (--client targets a connected TUI)
reovim cli keys 'iHello<Esc>' --client 1

# Query state
reovim cli mode --client 1
reovim cli cursor --client 1

# List connected clients
reovim cli clients

# List open buffers
reovim cli buffers

# JSON output
reovim cli --format json mode --client 1

# Connect to custom server (--grpc takes host:port, default: 127.0.0.1:12540)
reovim cli --grpc 127.0.0.1:9000 keys 'j' --client 1

# Buffer content
reovim cli buffer --id 1

# Register contents
reovim cli registers

# Capture screen (raw ANSI, plain text, cell grid, png, html)
reovim cli capture --client 1 --capture-format plain_text

# Server version
reovim cli version

# Tail server logs (with filters)
reovim cli log-tail --count 100 --level warn --target reovim_server --grep "connection"

# Ping server
reovim cli ping

# List extensions / query extension state
reovim cli extensions
reovim cli extension-state whichkey --client 1
```

#### Module Management

Manage third-party modules offline (no running server required):

```bash
# Install from git URL or local path
reovim module install https://github.com/user/my-module.git
reovim module install ./local-module --rev v1.0

# List / update / remove
reovim module list
reovim module update                      # Update all
reovim module update my-module            # Update one
reovim module remove my-module

# Diagnostics
reovim module check                       # Verify .so integrity
reovim module info my-module              # Detailed info
reovim module resolve                     # Resolve dependencies
```

#### TUI Client

Connect to a running server with a full terminal UI:

```bash
# Connect to default address (127.0.0.1:12540)
reovim tui

# Connect to specific server (--grpc takes host:port, default: 127.0.0.1:12540)
reovim tui --grpc 127.0.0.1:9000

# Headless mode (for scripting/testing)
reovim tui --grpc 127.0.0.1:12540 --headless

# Custom viewport size (headless only, default: 120x40)
reovim tui --grpc 127.0.0.1:12540 --headless --width 200 --height 50
```

### Key Bindings

Most movement commands support a numeric prefix (e.g., `5j` moves down 5 lines).

**Normal Mode**
| Key | Action |
|-----|--------|
| `h/j/k/l` | Move cursor left/down/up/right |
| `w/b` | Move word forward/backward |
| `0/$` | Move to line start/end |
| `gg` | Go to first line (or `{n}gg` to go to line n) |
| `G` | Go to last line (or `{n}G` to go to line n) |
| `i` | Enter insert mode |
| `a` | Enter insert mode after cursor |
| `A` | Enter insert mode at end of line |
| `o` | Open new line below and enter insert mode |
| `O` | Open new line above and enter insert mode |
| `v` | Enter visual mode |
| `:` | Enter command mode |
| `x` | Delete character |
| `p/P` | Paste after/before cursor |
| `s` | Multi-char jump (type 2 chars + label to jump) |
| `f/F` | Find char forward/backward with labels |
| `t/T` | Till char forward/backward with labels |
| `Space e` | Toggle explorer |
| `Space f f` | Microscope find files |
| `Space f g` | Microscope live grep |
| `Space f b` | Microscope buffers |
| `Ctrl-Space` | Trigger completion |
| `za` | Toggle fold at cursor |
| `zo` | Open fold at cursor |
| `zc` | Close fold at cursor |
| `zR` | Open all folds |
| `zM` | Close all folds |

**Insert Mode**
| Key | Action |
|-----|--------|
| `Escape` | Return to normal mode |
| `Backspace` | Delete character before cursor |

**Visual Mode**
| Key | Action |
|-----|--------|
| `h/j/k/l` | Extend selection |
| `d` | Delete selection |
| `y` | Yank selection |
| `Escape` | Return to normal mode |

**Command Mode**
| Command | Action |
|---------|--------|
| `:q` | Quit |
| `:w [file]` | Write (save) |
| `:wq` | Write and quit |
| `:set nu` / `:set number` | Show line numbers |
| `:set nonu` / `:set nonumber` | Hide line numbers |
| `:set rnu` / `:set relativenumber` | Show relative line numbers |
| `:set nornu` / `:set norelativenumber` | Hide relative line numbers |
| `:set signcolumn=<mode>` | Sign column: `auto`, `yes`, `no`, `number` |
| `:set virtual_text` | Enable inline diagnostic messages |
| `:health` / `:checkhealth` | Open health check diagnostic modal |
| `:LspLog` | Open LSP log file in editor |
| `:profile list` | Open profile picker |
| `:profile load <name>` | Load a configuration profile |
| `:profile save <name>` | Save current settings as profile |

## Architecture

Reovim follows a **Linux kernel-inspired architecture** with clear separation between mechanisms and policies.

```
┌─────────────────────────────────────────────────────────────┐
│  APPLICATION (apps/bin/, clients/)                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Server    │  │   Clients   │  │   Event Loop        │  │
│  │  (gRPC/TCP) │  │(TUI/CLI/Web)│  │   Module Loader     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  MODULES (server/modules/)                        POLICY    │
│  Keymap, Operators, Layout, Options, Mode-Manager           │
│  → Decide HOW things behave (keybindings, defaults)         │
├─────────────────────────────────────────────────────────────┤
│  DRIVERS (server/lib/drivers/)                    MECHANISM │
│  syntax/, input/, session/, lsp/, vfs/, command/            │
│  → Provide services, define trait contracts                 │
├─────────────────────────────────────────────────────────────┤
│  KERNEL (server/lib/kernel/)                      MECHANISM │
│  mm/ (Buffer, Position), ipc/ (EventBus), core/ (Mode)      │
│  block/ (UndoTree), sched/ (Runtime), api/ (public API)     │
│  → Core primitives, WHAT can be done                        │
└─────────────────────────────────────────────────────────────┘
```

**Crate Structure:**
- `reovim-app` (apps/bin/) - Main binary, server/client modes, module loading
- `reovim-kernel` (server/lib/kernel/) - Core mechanisms: buffers, events, modes, undo
- `reovim-driver-*` (server/lib/drivers/) - Services: syntax, input, session, LSP, network
- `reovim-module-*` (server/modules/) - Policy modules: keymap, operators, layout
- `reovim-client-*` (clients/) - Client applications: TUI, CLI, Web

**Key Design Principles:**
- **Mechanism vs Policy** - Kernel provides WHAT (traits), modules decide HOW (behavior)
- **API Boundary** - Modules use ONLY `reovim_kernel::api::*` (compile-time enforced)
- **Zero external deps in kernel** - Tree-sitter lives in drivers, not kernel
- **Multi-client support** - Server mode with per-client viewports and independent cursors

## Performance

Run benchmarks:
```bash
cargo bench -p reovim-kernel
```

Generate performance report:
```bash
cargo run -p perf-report -- update --version X.Y.Z
```

See [perf/](./perf/) for versioned benchmark results.

## Documentation

**Architecture (v0.9.0+):**
- [Architecture Overview](./docs/architecture/overview.md) - Layer diagram, Linux mapping
- [Kernel Overview](./docs/architecture/kernel/overview.md) - Core subsystems: mm/, ipc/, core/, block/
- [Driver Overview](./docs/architecture/drivers/overview.md) - Services: syntax, input, display, LSP
- [Module System](./docs/architecture/modules/overview.md) - Module trait, registration
- [Server Runtime](./docs/architecture/server/overview.md) - gRPC server, sessions
- [Client Architecture](./docs/architecture/client/overview.md) - Client Layer Model

**Contributing:**
- [Getting Started](./docs/contributing/getting-started.md) - Development setup
- [Mechanism vs Policy](./docs/contributing/philosophy/mechanism-vs-policy.md) - Core design principle
- [Linux Architecture](./docs/contributing/philosophy/linux-architecture.md) - Kernel design mapping
- [Module Development](./docs/contributing/guides/module-development.md) - Creating modules
- [Testing](./docs/contributing/guides/testing.md) - Testing guide and patterns

**User Guide:**
- [Configuration](./docs/user-guide/configuration.md) - Editor settings
- [Commands](./docs/user-guide/commands.md) - Command system and keybindings
- [Text Objects](./docs/user-guide/text-objects.md) - Delimiter and semantic text objects
- [Server Mode](./docs/user-guide/server-mode.md) - RPC server usage
- [Troubleshooting](./docs/user-guide/troubleshooting.md) - Common issues

**Archive (v0.8.x legacy):**
- [Legacy Documentation](https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel/docs) - Pre-v0.9.0 documentation (removed from tree, preserved in git history)

## License

AGPL-3.0 - See [LICENSE](./LICENSE) for details.

For commercial licensing options, contact: ds1sqe@mensakorea.org
