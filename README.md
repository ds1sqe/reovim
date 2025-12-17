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
- **Leap motion** - Two-character jump (`s`/`S` + 2 chars)
- **Telescope fuzzy finder** - Files, buffers, grep (`Space f`)
- **Explorer file browser** - Tree view (`Space e`)
- Jump list navigation (`Ctrl-O`, `Ctrl-I`)

### UI
- Command-line mode (`:q`, `:w`, `:wq`, `:set`)
- Line numbers (absolute, relative, hybrid)
- Which-key popup for keybindings
- Completion engine (`Ctrl-Space`)
- Status line with mode, pending keys, last command
- Landing page when started without a file

### Performance
- **50-80% faster** than v0.3.0 baseline
- Sub-microsecond window render (~639ns)
- 91% faster page navigation
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
| `s` | Leap forward (type 2 chars to jump) |
| `S` | Leap backward |
| `Space e` | Toggle explorer |
| `Space f f` | Telescope find files |
| `Space f g` | Telescope live grep |
| `Space f b` | Telescope buffers |
| `Ctrl-Space` | Trigger completion |

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

## Architecture

```
MAIN ──▶ CORE ──▶ SYS
```

- `reovim` (MAIN) - Main binary
- `reovim-core` (CORE) - Core editor logic (runtime, buffers, events, screen)
- `reovim-sys` (SYS) - System abstraction layer (crossterm re-exports)

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
- [Development](./docs/DEVELOPMENT.md) - Setup and contributing
- [Testing](./docs/TESTING.md) - Running and writing tests

## License

MIT
