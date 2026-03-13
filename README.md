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
- **Telescope fuzzy finder** - Files, buffers, grep (`Space f`)
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

v0.8.0 introduces priority channels for dramatically improved input responsiveness:

| Metric | v0.6.0 | v0.7.10 | v0.8.0 |
|--------|--------|---------|--------|
| Window render (10 lines) | 10 µs | 5.3 µs | 5.1 µs |
| Window render (10k lines) | 56 µs | 26 µs | 23 µs |
| Full scroll cycle | 85 µs | 55 µs | 48 µs |
| Large file (5k lines) | 174 µs | 87 µs | 87 µs |
| Throughput | 18k/sec | 38k/sec | 40k/sec |
| **Auto-pair latency** | - | ~100ms | **~92µs** |

**v0.8.0 highlights:**
- **~1000x input latency improvement** - Priority channels separate user input from background tasks
- **Zero flickering** - Diff-based rendering sends only changed cells
- **2x faster rendering** - Optimized render pipeline since v0.6.0
- Async architecture with tokio runtime
- Cross-platform terminal support via crossterm

## Installation

```bash
cargo install reovim
```

## Building from Source

```bash
git clone https://github.com/ds1sqe/reovim.git
cd reovim
cargo build --release
```

## Usage

```bash
reovim [file]
```

### Server Mode

Reovim can run as a JSON-RPC 2.0 server for programmatic control, enabling integration with external tools, IDEs, and automation scripts. The server accepts multiple sequential connections - clients can connect, disconnect, and reconnect without restarting the server.

**Multi-instance support**: Multiple reovim servers can run concurrently. When the default port (12521) is in use, the server automatically tries 12522, 12523, etc. Each server writes a port file to `~/.local/share/reovim/servers/<pid>.port` for discovery.

```bash
# Start server (TCP on 127.0.0.1:12521 by default)
reovim server

# Custom TCP port
reovim server --tcp 9000

# Unix socket
reovim server --socket /tmp/reovim.sock

# Stdio transport (for process piping)
reovim server --stdio
```

**Server Options:**
| Flag | Description |
|------|-------------|
| `--tcp <PORT>` | Listen on custom TCP port (default: 12521) |
| `--socket <PATH>` | Listen on Unix socket |
| `--stdio` | Use stdio instead of TCP (for process piping) |

**Default port:** `12521` (derived from ASCII: 'r'×100 + 'e'×10 + 'o')

#### CLI Client

The built-in CLI client provides command-line access to running servers:

```bash
# List running servers
reovim cli list

# Inject keys
reovim cli keys 'iHello<Esc>'

# Query state
reovim cli mode                           # Get current mode
reovim cli cursor                         # Get cursor position

# JSON output format
reovim cli --format json mode

# Connect to custom address
reovim cli --tcp localhost:9000 keys 'j'
reovim cli --socket /tmp/reovim.sock keys 'j'

# Interactive REPL
reovim cli -i
```

#### TUI Client

Connect to a running server with a full terminal UI:

```bash
# Auto-discover and connect
reovim tui

# Connect to specific server
reovim tui --tcp 127.0.0.1:12521
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
| `Space f f` | Telescope find files |
| `Space f g` | Telescope live grep |
| `Space f b` | Telescope buffers |
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
│  RUNNER (runner/)                              APPLICATION  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Server    │  │   Client    │  │   Event Loop        │  │
│  │  (RPC/TCP)  │  │  (CLI/TUI)  │  │   Module Loader     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  MODULES (modules/)                             POLICY      │
│  Keymap, Operators, Layout, Options, Mode-Manager           │
│  → Decide HOW things behave (keybindings, defaults)         │
├─────────────────────────────────────────────────────────────┤
│  DRIVERS (lib/drivers/)                         MECHANISM   │
│  syntax/, input/, display/, lsp/, net/, vfs/, command/      │
│  → Provide services, define trait contracts                 │
├─────────────────────────────────────────────────────────────┤
│  KERNEL (lib/kernel/)                           MECHANISM   │
│  mm/ (Buffer, Position), ipc/ (EventBus), core/ (Mode)      │
│  block/ (UndoTree), sched/ (Runtime), api/ (public API)     │
│  → Core primitives, WHAT can be done                        │
└─────────────────────────────────────────────────────────────┘
```

**Crate Structure:**
- `reovim` (runner/) - Main binary, server/client modes, module loading
- `reovim-kernel` (lib/kernel/) - Core mechanisms: buffers, events, modes, undo
- `reovim-driver-*` (lib/drivers/) - Services: syntax, input, display, LSP, network
- `reovim-module-*` (modules/) - Policy modules: keymap, operators, layout

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
- [Legacy Documentation](./archive/docs/) - Pre-v0.9.0 documentation

## License

AGPL-3.0 - See [LICENSE](./LICENSE) for details.

For commercial licensing options, contact: ds1sqe@mensakorea.org
