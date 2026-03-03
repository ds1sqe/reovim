# CLI Client

Command-line interface for scripting and automation.

## Source Location

`clients/cli/src/` (moved from `runner/src/client/cli/` in Phase 8)

## Commands

### Key Injection

```bash
reovim cli keys 'iHello<Esc>' --client 1
```

Supports vim-style key notation:
- `<Esc>`, `<CR>`, `<Tab>`
- `<C-w>`, `<A-x>` (modifiers)
- Regular characters

### State Queries

```bash
# Current mode
reovim cli mode --client 1
Mode: NORMAL

# Cursor position
reovim cli cursor --client 1
Cursor: line 0, column 5

# JSON output
reovim cli --format json mode
{"mode":"Normal"}
```

### Server Queries

```bash
# List connected clients
reovim cli clients

# Server info
reovim cli ping
reovim cli version
```

### Debug Commands

```bash
# Log viewing with filters
reovim cli log-tail --count 100   # Last 100 entries
reovim cli log-tail --level warn  # Filter by level
reovim cli log-tail --target mod  # Filter by module
reovim cli log-tail --grep "err"  # Search messages
```

Output is color-coded: ERROR (red), WARN (yellow), INFO (green), DEBUG (cyan), TRACE (gray)

## Connection Options

```bash
# Auto-discover (default)
reovim cli keys 'j'

# Explicit gRPC endpoint
reovim cli --grpc 127.0.0.1:12540 keys 'j' --client 1
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
reovim server --grpc 12540 &
SERVER_PID=$!
sleep 1
reovim cli --grpc 127.0.0.1:12540 keys 'iTest<Esc>' --client 1
if reovim cli --grpc 127.0.0.1:12540 mode --client 1 | grep -q "NORMAL"; then
  echo "PASS"
fi
kill $SERVER_PID
```

### IDE Integration

```python
import subprocess
result = subprocess.run(
    ['reovim', 'cli', '--format', 'json', 'cursor', '--client', '1'],
    capture_output=True, text=True
)
cursor = json.loads(result.stdout)
```

## Related Documents

- [Client Overview](./overview.md) - Client architecture
- [TUI Client](./tui.md) - Full terminal interface
