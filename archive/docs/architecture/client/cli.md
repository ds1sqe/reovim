# CLI Client

Command-line interface for scripting and automation.

## Source Location

`clients/cli/src/`

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

### Buffer Queries

```bash
# List open buffers
reovim cli buffers

# Get buffer content (defaults to active buffer)
reovim cli buffer
reovim cli buffer --id 3
```

### Register Queries

```bash
# List all registers
reovim cli registers

# Get a specific register
reovim cli registers a
```

### Screen Capture

```bash
# Capture current screen in various formats
reovim cli capture --format raw_ansi
reovim cli capture --format plain_text
reovim cli capture --format cell_grid
reovim cli capture --format png
reovim cli capture --format html
```

### Extension Queries

```bash
# List registered extensions
reovim cli extensions

# Query extension state by kind
reovim cli extension-state --kind <KIND>
```

### Server Queries

```bash
# List connected clients
reovim cli clients

# Server info
reovim cli ping
reovim cli version
```

### Module Management

Manage the module registry — install, remove, update, and inspect loadable modules.

```bash
reovim module list                    # List installed modules
reovim module install <source>        # Install from source
reovim module remove <id>             # Remove a module
reovim module update [id]             # Update modules (all if no id given)
reovim module info <id>               # Show module info
reovim module check                   # Verify module integrity
reovim module resolve                 # Resolve module dependencies
```

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
