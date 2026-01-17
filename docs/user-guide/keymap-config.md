# Keymap Configuration

Customize keybindings via `~/.config/reovim/keymap.toml`. User bindings have the highest priority and override default Vim keybindings.

## Configuration File Location

| Platform | Path |
|----------|------|
| Linux | `~/.config/reovim/keymap.toml` |
| macOS | `~/Library/Application Support/reovim/keymap.toml` |
| Windows | `%APPDATA%\reovim\keymap.toml` |

## Quick Start

Create the config file and add your bindings:

```toml
# ~/.config/reovim/keymap.toml

[bindings.normal]
"<C-s>" = "buffer:save"
"<C-q>" = "app:quit"

[bindings.insert]
"jk" = "mode:normal"
```

Restart reovim to apply changes.

## Adding Keybindings

Use `[bindings.<mode>]` sections to add or override keybindings:

```toml
[bindings.normal]
"<C-s>" = "buffer:save"           # Ctrl+S to save
"<C-q>" = "app:quit"              # Ctrl+Q to quit
"<Leader>ff" = "picker:files"     # Leader+ff for file picker
"<Leader>fg" = "picker:grep"      # Leader+fg for grep

[bindings.insert]
"jk" = "mode:normal"              # Quick escape to normal mode
"<C-c>" = "mode:normal"           # Ctrl+C also escapes

[bindings.visual]
"<Leader>y" = "clipboard:yank-system"  # Yank to system clipboard
```

### Supported Modes

| Mode | Description |
|------|-------------|
| `normal` | Normal mode (default) |
| `insert` | Insert mode |
| `visual` | Visual character mode |
| `visual_line` | Visual line mode |
| `visual_block` | Visual block mode |
| `operator_pending` | After pressing an operator (d, y, c) |
| `commandline` | Command-line mode (after `:`) |

You can also use the full form `editor:normal` instead of just `normal`.

### Key Notation

| Notation | Description |
|----------|-------------|
| `a`, `b`, `z` | Single characters |
| `<C-s>` | Ctrl + S |
| `<A-x>` or `<M-x>` | Alt/Meta + X |
| `<S-Tab>` | Shift + Tab |
| `<C-S-a>` | Ctrl + Shift + A |
| `<Enter>` or `<CR>` | Enter/Return |
| `<Esc>` | Escape |
| `<Tab>` | Tab |
| `<Space>` | Space |
| `<BS>` | Backspace |
| `<Leader>` | Leader key (default: `\`) |
| `<Up>`, `<Down>`, `<Left>`, `<Right>` | Arrow keys |
| `<Home>`, `<End>` | Home/End keys |
| `<PageUp>`, `<PageDown>` | Page Up/Down |
| `<F1>` - `<F12>` | Function keys |

### Command Format

Commands use the format `module:command`:

```toml
"<C-s>" = "buffer:save"       # buffer module, save command
"j" = "editor:cursor-down"    # editor module, cursor-down command
"w" = "motions:word-forward"  # motions module, word-forward command
"d" = "operators:delete"      # operators module, delete command
```

Common modules:
- `editor` - Basic cursor movement and mode switching
- `buffer` - Buffer operations (save, close)
- `motions` - Word, line, and search motions
- `operators` - Delete, yank, change operators
- `mode` - Mode transitions
- `app` - Application commands (quit)

## Removing Keybindings

Use `[remove.<mode>]` sections to disable default keybindings:

```toml
[remove.normal]
keys = ["Q", "gQ"]        # Disable Ex mode keys

[remove.insert]
keys = ["<C-a>"]          # Disable Ctrl+A in insert mode
```

Removed bindings are shadowed at the User layer - the underlying binding still exists but won't be triggered.

## Layer Priority

Keybindings are resolved in layer order (highest to lowest):

1. **User** - Your `keymap.toml` overrides (highest priority)
2. **Policy** - Module defaults (Vim keybindings)
3. **Base** - Mechanism defaults (lowest priority)

This means your bindings always win over defaults.

## Examples

### Emacs-style Insert Mode

```toml
[bindings.insert]
"<C-a>" = "editor:line-start"
"<C-e>" = "editor:line-end"
"<C-f>" = "editor:cursor-right"
"<C-b>" = "editor:cursor-left"
"<C-n>" = "editor:cursor-down"
"<C-p>" = "editor:cursor-up"
"<C-d>" = "editor:delete-char"
"<C-k>" = "editor:delete-to-eol"
```

### Custom Leader Mappings

```toml
[bindings.normal]
# File operations
"<Leader>w" = "buffer:save"
"<Leader>q" = "buffer:close"
"<Leader>Q" = "app:quit"

# Window navigation
"<Leader>h" = "window:focus-left"
"<Leader>j" = "window:focus-down"
"<Leader>k" = "window:focus-up"
"<Leader>l" = "window:focus-right"

# Splits
"<Leader>v" = "window:split-vertical"
"<Leader>s" = "window:split-horizontal"
```

### Disable Dangerous Keys

```toml
[remove.normal]
keys = [
    "ZZ",       # Disable save and quit
    "ZQ",       # Disable quit without saving
    "Q",        # Disable Ex mode
]
```

## Validation and Errors

Reovim validates your config at startup:

- **Invalid key sequences** are reported with warnings
- **Invalid command IDs** (missing `module:command` format) are reported
- **Valid entries are still applied** even if some entries have errors
- **Missing config file** is fine - defaults are used

Check server logs for validation warnings:

```
WARN keymap.toml validation warning: invalid key sequence '<BadKey>' in mode 'normal'
INFO applied user keymap configuration: bindings_added=5, bindings_removed=2, warnings=1
```

## Troubleshooting

### Binding Not Working

1. Check the key notation is correct (use `<C-s>` not `Ctrl-s`)
2. Verify the command ID format (`module:command`)
3. Check server logs for validation errors
4. Ensure the mode name is correct

### Finding Command Names

Use the command palette or check module documentation:

```bash
# List all registered commands
reovim cli command/list
```

### Config Not Loading

1. Verify the file path is correct for your platform
2. Check TOML syntax (use a TOML validator)
3. Look for parse errors in server logs

## Related Documentation

- [Configuration Reference](./configuration.md) - Editor settings
- [Commands](./commands.md) - Command reference
- [Server Mode](./server-mode.md) - Server options
