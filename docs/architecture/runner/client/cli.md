# CLI Client

Command-line interface for scripting and automation.

## Source Location

`runner/src/client/cli/`

## Commands

### Key Injection

```bash
reovim cli keys 'iHello<Esc>'
```

Supports vim-style key notation:
- `<Esc>`, `<CR>`, `<Tab>`
- `<C-w>`, `<A-x>` (modifiers)
- Regular characters

### State Queries

```bash
# Current mode
reovim cli mode
Mode: NORMAL

# Cursor position
reovim cli cursor
Cursor: line 0, column 5

# JSON output
reovim cli --format json mode
{"mode":"Normal"}
```

### Server Management

```bash
# List servers
reovim cli list
127.0.0.1:12521 (pid: 123456)

# Kill server
reovim cli kill
```

### Interactive REPL

```bash
$ reovim cli -i
reovim> keys iHello<Esc>
ok: true
reovim> mode
Mode: NORMAL
reovim> cursor
Cursor: line 0, column 5
reovim> quit
```

## Connection Options

```bash
# Auto-discover (default)
reovim cli keys 'j'

# Explicit TCP
reovim cli --tcp localhost:12521 keys 'j'

# Unix socket
reovim cli --socket /tmp/reovim.sock keys 'j'
```

## Output Formats

| Format | Flag | Description |
|--------|------|-------------|
| Human | (default) | Readable output |
| JSON | `--format json` | Machine-parseable |

## Use Cases

### Automated Testing

```bash
#!/bin/bash
reovim server &
sleep 1
reovim cli keys 'iTest<Esc>'
if reovim cli mode | grep -q "NORMAL"; then
  echo "PASS"
fi
reovim cli kill
```

### IDE Integration

```python
import subprocess
result = subprocess.run(
    ['reovim', 'cli', '--format', 'json', 'cursor'],
    capture_output=True, text=True
)
cursor = json.loads(result.stdout)
```

## Related Documents

- [Client Overview](./overview.md) - Client architecture
- [TUI Client](./tui.md) - Full terminal interface
