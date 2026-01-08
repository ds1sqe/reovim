# reovim

[![CI](https://github.com/ds1sqe/reovim/actions/workflows/ci.yml/badge.svg)](https://github.com/ds1sqe/reovim/actions/workflows/ci.yml)
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
reovim --server

# With a file
reovim --server myfile.txt

# Custom TCP port
reovim --server --listen-tcp 9000

# Unix socket
reovim --listen-socket /tmp/reovim.sock

# Stdio transport (for process piping)
reovim --stdio

# Dual output: render to terminal while serving RPC
reovim --server --terminal
```

**Server Options:**
| Flag | Description |
|------|-------------|
| `--server` | Start in server mode (TCP on 127.0.0.1:12521) |
| `--test` | Exit when all clients disconnect (for testing/CI) |
| `--stdio` | Use stdio instead of TCP (for process piping, always one-shot) |
| `--listen-socket <PATH>` | Listen on Unix socket |
| `--listen-tcp <PORT>` | Listen on custom TCP port |
| `--listen-host <HOST>` | Bind to specific host (default: 127.0.0.1) |
| `--terminal` | Also render to terminal in server mode |

**Default port:** `12521` (derived from ASCII: 'r'×100 + 'e'×10 + 'o')

#### reo-cli Client

A CLI client tool is available for interacting with the server:

```bash
# List running servers
reo-cli list

# Inject keys and show status (screen + mode + cursor)
reo-cli keys 'iHello<Esc>'

# Connect to custom address
reo-cli --tcp localhost:9000 keys 'j'
reo-cli --socket /tmp/reovim.sock keys 'j'

# Interactive REPL
reo-cli --interactive
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

```
┌─I/O─────────────────────────────────────────────────────────┐
│  ┌──────────────┐                        ┌──────────────┐   │
│  │    CLIENT    │                        │   TERMINAL   │   │
│  │  (reo-cli)   │                        │  (crossterm) │   │
│  └──────────────┘                        └──────────────┘   │
└───────┬────────────────────────────────────────────┬────────┘
        │ input                               output │
        ▼                                            ▲
┌──────────────────┐                       ┌─────────┴──────────┐
│ InputEventBroker │                       │    FrameBuffer     │
│  (async reader)  │                       │  (diff rendering)  │
└────────┬─────────┘                       └──────────▲─────────┘
         │                                            │
         ▼                                            │
┌──────────────────┐                       ┌──────────┴─────────┐
│  KeyEventBroker  │                       │  LayerCompositor   │
│   (broadcast)    │                       │ (editor/overlays)  │
└────────┬─────────┘                       └──────────▲─────────┘
         │                                            ¦
       ┌─┴──────────────┐                             ¦
       ▼                ▼                             ¦
┌──────────────┐ ┌──────────────┐                     ¦
│CommandHandler│ │PluginHandlers│                     ¦
│(keys→command)│ │  (EventBus)  │ ◀───────────────┐  ¦
└──────┬───────┘ └──────┬───────┘                  │  ¦
       │                │                          │  ¦
       └───────┬────────┘                          │  ¦
               ▼                                   │  ¦
┌─────────────────────────────┐                    │  ¦
│      PRIORITY CHANNELS      │                    │  ¦
│  hi: user input (64)        │                    │  ¦
│  lo: background (255)       │                    │  ¦
└─────────────┬───────────────┘                    │  ¦
              ▼                                    ▼  ¦
┌─────────────────────────────┐  ┌────────────────────────────┐
│          RUNTIME            │  │           PLUGINS          │
│ ┌───────┐ ┌──────┐ ┌──────┐ │  │ Feature: range-finder,     │
│ │Buffers│ │Screen│ │ Cmds │ │  │   microscope, lsp,         │
│ └───────┘ └──────┘ └──────┘ │  │   completion, explorer     │
│ ┌──────┐                    │  │ Languages: rust, c, js,    │
│ │ Mode │                    │  │   python, json, toml, md   │
│ └──────┘                    │  └────────────────────────────┘
└──────────────┬──────────────┘                       ¦
               └-----─────────────────────────────────┘
```

- `reovim` (RUNNER) - Main binary, plugin loading
- `reovim-core` (CORE) - Runtime, buffers, events, screen, commands
- `reovim-sys` (SYS) - System abstraction layer (crossterm re-exports)

**Key Design:**
- **Priority channels** separate user input (hi) from background tasks (lo) for ~1000x latency improvement
- **Diff-based rendering** sends only changed cells to terminal (zero flickering)
- **EventBus** enables plugin-to-plugin communication without core modifications

## Performance

Run benchmarks:
```bash
cargo bench -p reovim-core
```

Generate performance report:
```bash
cargo run -p perf-report -- update --version X.Y.Z
```

See [perf/](./perf/) for versioned benchmark results.

## Documentation

- [Architecture](./docs/architecture/overview.md) - System design and component overview
- [Event System](./docs/events/overview.md) - Input handling and event flow
- [Commands](./docs/reference/commands.md) - Command system and keybindings
- [Plugin System](./docs/plugins/system.md) - Plugin architecture and development
- [Rendering](./docs/rendering/overview.md) - Rendering systems guide
- [Render Pipeline](./docs/rendering/pipeline.md) - Render stages and data flow
- [Diagnostics](./docs/features/diagnostics.md) - Sign column and virtual text configuration
- [Text Objects](./docs/reference/text-objects.md) - Delimiter and semantic text objects
- [Server Mode](./docs/reference/server-mode.md) - RPC server and multi-instance support
- [Window & Buffer](./docs/features/window-buffer.md) - Window architecture
- [Animation System](./docs/features/animation-system.md) - Visual effects and animations
- [Decoration System](./docs/features/decoration-system.md) - Language-aware decorations
- [Saturator](./docs/features/saturator.md) - Background task architecture
- [Color System](./docs/features/color-system.md) - Color palette design
- [Syntax Highlighting](./docs/features/syntax-highlighting.md) - Rust AST taxonomy
- [Development](./docs/guides/development.md) - Setup and contributing
- [Testing](./docs/guides/testing.md) - Running and writing tests

## License

AGPL-3.0 - See [LICENSE](./LICENSE) for details.

For commercial licensing options, contact: ds1sqe@mensakorea.org
