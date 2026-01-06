# reovim

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

### Syntax & Code Intelligence
- **Treesitter syntax highlighting** - Accurate parsing for Rust, C, JavaScript, Python, JSON, TOML, Markdown
- **Semantic text objects** - `af`/`if` (function), `ac`/`ic` (class/struct)

### Performance

v0.7.10 continues the diff-based rendering with significant optimizations:

| Metric | v0.6.0 | v0.7.10 | Change |
|--------|--------|---------|--------|
| Window render (10 lines) | 10 µs | 5.3 µs | **-47%** |
| Window render (10k lines) | 56 µs | 26 µs | **-54%** |
| Full scroll cycle | 85 µs | 55 µs | **-35%** |
| Large file (5k lines) | 174 µs | 87 µs | **-50%** |
| Throughput | 18k/sec | 38k/sec | **+111%** |

**Key benefits:**
- **Zero flickering** - Only changed cells sent to terminal
- **2x faster rendering** - Optimized render pipeline
- **Composable layers** - Clean UI component separation
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
MAIN ──▶ CORE ──▶ SYS
           │
           └── LayerCompositor ──▶ FrameBuffer ──▶ Terminal
```

- `reovim` (MAIN) - Main binary
- `reovim-core` (CORE) - Core editor logic (runtime, buffers, events, screen)
- `reovim-sys` (SYS) - System abstraction layer (crossterm re-exports)

**Rendering Pipeline**: LayerCompositor renders UI layers (editor, explorer, overlays) to a FrameBuffer, which diffs against the previous frame to emit minimal terminal updates.

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

- [Architecture](./docs/architecture.md) - System design and component overview
- [Event System](./docs/event-system.md) - Input handling and event flow
- [Commands](./docs/commands.md) - Command system and keybindings
- [Plugin System](./docs/plugin-system.md) - Plugin architecture and development
- [Plugin Rendering](./docs/plugin-rendering.md) - Rendering systems guide
- [Render Pipeline](./docs/render-pipeline.md) - Render stages and data flow
- [Diagnostics](./docs/diagnostics.md) - Sign column and virtual text configuration
- [Text Objects](./docs/text-objects.md) - Delimiter and semantic text objects
- [Server Mode](./docs/server-mode.md) - RPC server and multi-instance support
- [Window & Buffer](./docs/window-buffer.md) - Window architecture
- [Animation System](./docs/animation-system.md) - Visual effects and animations
- [Decoration System](./docs/decoration-system.md) - Language-aware decorations
- [Saturator](./docs/saturator.md) - Background task architecture
- [Color System](./docs/color-system.md) - Color palette design
- [Syntax Highlighting](./docs/syntax-highlighting.md) - Rust AST taxonomy
- [Development](./docs/DEVELOPMENT.md) - Setup and contributing
- [Testing](./docs/TESTING.md) - Running and writing tests

## License

AGPL-3.0 - See [LICENSE](./LICENSE) for details.

For commercial licensing options, contact: ds1sqe@mensakorea.org
