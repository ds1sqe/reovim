# reovim

A Rust-powered neovim-like text editor.

## Features

- Modal editing (Normal, Insert, Visual, Command modes)
- Vim-style keybindings (h/j/k/l navigation, w/b word motions)
- Visual selection with yank/delete
- Command-line mode (`:q`, `:w`, `:wq`)
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

**Normal Mode**
| Key | Action |
|-----|--------|
| `h/j/k/l` | Move cursor left/down/up/right |
| `w/b` | Move word forward/backward |
| `0/$` | Move to line start/end |
| `i` | Enter insert mode |
| `a` | Enter insert mode after cursor |
| `v` | Enter visual mode |
| `:` | Enter command mode |
| `x` | Delete character |
| `p/P` | Paste after/before cursor |

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

## Architecture

- `reovim` - Main binary
- `reovim-core` - Core editor logic (runtime, buffers, events, screen)
- `reovim-sys` - System abstraction layer (crossterm re-exports)

## License

MIT
