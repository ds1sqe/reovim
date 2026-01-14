# Configuration Reference

This document describes reovim's configuration options and how to set them.

## Setting Options

Options can be set using the `:set` ex-command:

```vim
:set number          " Enable boolean option
:set nonumber        " Disable boolean option
:set tabwidth=4      " Set numeric/string option
:set tabwidth?       " Query current value
```

Short forms are available for common options:

```vim
:set nu              " Same as :set number
:set rnu             " Same as :set relativenumber
:set tw=4            " Same as :set tabwidth=4
```

## Profiles

Configuration is saved to profiles. Use `:profile` to manage them:

```vim
:profile save myprofile    " Save current settings
:profile load myprofile    " Load a profile
:profile list              " List available profiles
:profile delete myprofile  " Delete a profile
```

Profile files are stored in `~/.config/reovim/profiles/`.

## Editor Options

### Line Numbers

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `number` | `nu` | bool | `true` | Show line numbers |
| `relativenumber` | `rnu` | bool | `false` | Show relative line numbers |

**Examples:**
```vim
:set number relativenumber   " Hybrid line numbers
:set nonumber               " Hide line numbers
```

### Tabs and Indentation

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `tabwidth` | `tw` | int | `4` | Spaces per tab (1-8) |
| `expandtab` | `et` | bool | `true` | Use spaces instead of tabs |
| `indentguide` | - | bool | `true` | Show indentation guides |

**Examples:**
```vim
:set tabwidth=2 expandtab    " 2-space indentation
:set tabwidth=4 noexpandtab  " Real tabs
```

### Scrolling

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `scrolloff` | `so` | int | `5` | Lines to keep visible above/below cursor |
| `scrollbar` | - | bool | `true` | Show scrollbar |

### Sign Column

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `signcolumn` | string | `"yes"` | Sign column display mode |

**Values:**
- `"auto"` - Show only when signs present
- `"yes"` - Always show (2 columns)
- `"no"` - Never show
- `"number"` - Show signs in number column

## Display Options

### Theme

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `theme` | choice | `"dark"` | Color theme |
| `colormode` | choice | `"truecolor"` | Terminal color support |

**Theme values:** `"dark"`, `"light"`, `"tokyonight"`

**Color mode values:**
- `"ansi"` - 16 ANSI colors
- `"256"` - 256 color palette
- `"truecolor"` - 24-bit RGB colors

**Examples:**
```vim
:set theme=tokyonight
:set colormode=256
```

## Window Options

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `splitbelow` | `sb` | bool | `true` | New horizontal splits below |
| `splitright` | `spr` | bool | `true` | New vertical splits right |

**Examples:**
```vim
:set splitbelow splitright   " Split to bottom-right
:set nosplitbelow            " Split above
```

## Diagnostics Options

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `virtual_text` | `vt` | bool | `true` | Show inline diagnostics |
| `virtual_text_prefix` | - | string | `""` | Prefix for virtual text |
| `virtual_text_max_length` | - | int | `80` | Max virtual text length (10-200) |
| `virtual_text_show` | - | choice | `"first"` | Which diagnostics to show |

**virtual_text_show values:**
- `"first"` - Show first diagnostic only
- `"highest"` - Show highest severity only
- `"all"` - Show all diagnostics

**Examples:**
```vim
:set virtual_text               " Enable inline diagnostics
:set virtual_text_show=highest  " Show only errors
```

## Plugin Options

Plugins register their own options under namespaced sections. Common plugin options:

### Treesitter

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `treesitter.highlight` | bool | `true` | Enable syntax highlighting |
| `treesitter.timeout` | int | `100` | Parse timeout (ms) |

### Fold

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `fold.enabled` | bool | `true` | Enable code folding |

## Configuration File

Settings are stored in TOML format in profile files:

```toml
# ~/.config/reovim/profiles/default.toml

[editor]
number = true
relativenumber = false
tabwidth = 4
expandtab = true
scrolloff = 5

[display]
theme = "dark"
colormode = "truecolor"

[window]
splitbelow = true
splitright = true

[diagnostics]
virtual_text = true
virtual_text_show = "first"

[plugin.treesitter]
highlight = true
timeout = 100
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `REOVIM_LOG` | Log level (`error`, `warn`, `info`, `debug`, `trace`) |
| `XDG_CONFIG_HOME` | Config directory (default: `~/.config`) |
| `XDG_DATA_HOME` | Data directory (default: `~/.local/share`) |

## Command Line Options

```bash
reovim <COMMAND>

Commands:
  server    Start headless server
  tui       Connect with terminal UI
  cli       Execute commands

Server options:
  --tcp <PORT>      Listen on custom TCP port (default: 12521)
  --socket <PATH>   Listen on Unix socket
  --stdio           Use stdio transport

Client options (tui/cli):
  --tcp <ADDR>      Connect via TCP (e.g., 127.0.0.1:12521)
  --socket <PATH>   Connect via Unix socket
```

**Examples:**
```bash
# Start server
reovim server

# Server on custom port
reovim server --tcp 9000

# Connect TUI client
reovim tui

# CLI commands
reovim cli list
reovim cli keys 'iHello<Esc>'

# Debug logging
REOVIM_LOG=debug reovim server --log=/tmp/reovim.log
```

## Settings Menu

Press `Space s` to open the interactive settings menu. Navigate with `j`/`k`, toggle booleans with `Enter`, and edit values inline.

## Related Documentation

- [Plugin System](../plugins/system.md) - Plugin configuration
- [Server Mode](../reference/server-mode.md) - Server options
- [Architecture](../architecture/overview.md) - System overview
