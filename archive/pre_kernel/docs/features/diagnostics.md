# Diagnostics: Sign Column and Virtual Text

This document covers the configuration of diagnostics display in reovim, including sign column modes and virtual text for inline diagnostic messages.

## Sign Column

The sign column displays diagnostic indicators (errors, warnings, hints) in a dedicated column to the left of line numbers.

### Configuration

```vim
:set signcolumn=<mode>
```

### Modes

| Mode | Description |
|------|-------------|
| `auto` | Show sign column only when signs are present (width 2) |
| `yes` | Always show sign column (default, width 2) |
| `no` | Never show sign column |
| `number` | Display signs as background color on line numbers |

### Examples

```vim
" Always show sign column (default)
:set signcolumn=yes

" Only show when there are diagnostics
:set signcolumn=auto

" Hide sign column completely
:set signcolumn=no

" Merge signs into line numbers (saves horizontal space)
:set signcolumn=number
```

### Sign Priority

Signs are displayed based on priority. Suggested priority ranges:

| Range | Purpose |
|-------|---------|
| 0-99 | Low priority (hints, suggestions) |
| 100-199 | Medium priority (info, warnings) |
| 200-299 | High priority (errors) |
| 300-399 | Critical (breakpoints, current line) |

When multiple signs exist on a line, the highest priority sign is displayed.

## Virtual Text

Virtual text displays diagnostic messages inline after the line content, providing immediate context without needing to hover or move the cursor.

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `virtual_text` | bool | `true` | Enable/disable virtual text display |
| `virtual_text_prefix` | string | `""` | Custom prefix (empty = use severity icons) |
| `virtual_text_max_length` | number | `80` | Maximum length before truncation |
| `virtual_text_show` | string | `"first"` | Which diagnostics to show: `first`, `highest`, `all` |

### Enabling/Disabling

```vim
" Enable virtual text (default)
:set virtual_text

" Disable virtual text
:set novirtual_text
```

### Display Modes

**`first`** - Show only the first diagnostic on each line:
```
let x = foo();  // ● Cannot find value `foo` in this scope
```

**`highest`** - Show the highest severity diagnostic on each line:
```
let x = foo();  // ● error: Cannot find value `foo`
```

**`all`** - Show all diagnostics (separated by ` | `):
```
let x = foo();  // ● error | ◐ warning: unused variable
```

### Severity Icons

When `virtual_text_prefix` is empty (default), severity-specific icons are used:

| Severity | Icon | Color |
|----------|------|-------|
| Error | `●` | Red |
| Warning | `◐` | Yellow |
| Info | `ⓘ` | Blue |
| Hint | `·` | Gray |

### Custom Prefix

```vim
" Use a custom prefix instead of severity icons
:set virtual_text_prefix=>>
```

This will display:
```
let x = foo();  // >> Cannot find value `foo` in this scope
```

### Truncation

Long diagnostic messages are automatically truncated based on `virtual_text_max_length`:

```vim
" Set maximum virtual text length
:set virtual_text_max_length=60
```

Messages exceeding this length will end with `...`:
```
let x = foo();  // ● Cannot find value `foo` in this scope, did you mean `bar`...
```

## Settings Menu Integration

All diagnostic options are available in the settings menu under the "Diagnostics" section:

```vim
:settings
```

Navigate to the Diagnostics section to toggle options interactively.

## LSP Integration

Diagnostics from LSP servers (like rust-analyzer) are automatically displayed using the sign column and virtual text systems. The integration handles:

- **Severity mapping**: LSP severities map to reovim's Error/Warning/Info/Hint levels
- **Range highlighting**: Diagnostic spans are highlighted in the buffer
- **Real-time updates**: Diagnostics update as you type (with debouncing)

### Diagnostic Priorities

LSP diagnostics use the following priorities for virtual text resolution:

| Severity | Priority |
|----------|----------|
| Error | 404 |
| Warning | 403 |
| Info | 402 |
| Hint | 401 |

Higher priority diagnostics take precedence when using `virtual_text_show=highest`.

## Troubleshooting

### Diagnostics not appearing

1. Check if LSP server is running: `:health`
2. Verify sign column is enabled: `:set signcolumn?`
3. Check virtual text is enabled: `:set virtual_text?`

### LSP diagnostics delayed

rust-analyzer uses LSP 3.17 "pull diagnostics". Reovim requests diagnostics immediately after `textDocument/didOpen` and handles `workspace/diagnostic/refresh` requests.

### Debugging LSP communication

Use the LSP log feature to inspect JSON-RPC messages:

```bash
# Start with LSP logging
reovim --lsp-log=default myfile.rs

# Or with custom path
reovim --lsp-log=/tmp/lsp.log myfile.rs
```

View logs in editor:
```vim
:LspLog
```
