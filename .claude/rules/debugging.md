---
paths:
  - "server/**/*.rs"
  - "clients/**/*.rs"
---

# Logs and Debugging

## Current State (v0.9.0)

The logging system is partially implemented. The driver infrastructure exists in `shared/log/` but CLI argument wiring is pending.

**What works now:**
- Server outputs logs to stderr via tracing
- RPC debug commands to query/control running server

**Planned (not yet implemented):**
- `--log=<path>` CLI argument
- `--lsp-log=<path>` CLI argument
- `REOVIM_LOG` environment variable integration
- Timestamped log files in `~/.local/share/reovim/`

## RPC Debug Commands

Query and control logging on a running server via CLI:

```bash
# Get recent log entries (default: 50)
reovim cli log-tail
reovim cli log-tail --count 100

# Filter by level (shows entries >= specified level)
reovim cli log-tail --level warn

# Filter by target module
reovim cli log-tail --target runner::server

# Search for text in messages (case-insensitive)
reovim cli log-tail --grep "connection"

# Combine filters
reovim cli log-tail --level info --target runner --grep error
```

Output is color-coded when stdout is a TTY:
- ERROR (red), WARN (yellow), INFO (green), DEBUG (cyan), TRACE (gray)

## Frame Capture

Capture TUI screen for debugging (requires connected TUI):

```bash
# Capture with ANSI colors (--client specifies target TUI)
reovim cli --grpc 127.0.0.1:PORT capture --client 1

# Plain text (good for logs/LLMs)
reovim cli --grpc 127.0.0.1:PORT capture --client 1 --format plain_text
```

See [Frame Capture Guide](docs/user-guide/frame-capture.md) for details.

## Debugging Flaky Tests

**Think Twice Before Assuming Race Conditions**: When tests pass/fail intermittently:

1. **Don't immediately add delays** - This masks the real problem
2. **Add debug logging** first to trace execution flow
3. **Look for mode/state synchronization issues** - Often what appears to be a timing race is actually a missing state update

**Example**: `<C-w>h` tests failed randomly because `mode_for_command()` didn't recognize `enter_window_mode`, so the local mode wasn't updated before the next key was looked up. This looked like a race condition but was actually a **missing match arm**.

## Debugging Multi-Window and Layout Issues

**Common symptoms and causes:**

1. **TUI not syncing with server state**
   - **Symptom**: Server has correct state (verified via CLI), but TUI shows stale data
   - **Cause**: Push notifications not being sent for state changes
   - **Check**: Verify `emit_from_state_changes()` is called with correct `StateChanges`
   - **Key file**: `server/lib/server/src/grpc/input.rs` - accumulates and emits changes

2. **Focus indicator at wrong position**
   - **Symptom**: `▪` appears at x=0 instead of focused window's position
   - **Cause**: Window ID comparison finding wrong placement
   - **Debug**: Add logging in `render_multi_window()` to trace `focused_id` and found placement

3. **Window separators incomplete**
   - **Symptom**: Missing `│`, `─`, or `┼` characters
   - **Cause**: Border drawing not checking for existing characters
   - **Key file**: `server/lib/server/src/grpc/state.rs` - `draw_window_separators()`

**Notification model (Push vs Pull):**

| Type | Method | Use Case |
|------|--------|----------|
| **Pull** | RPC call (`state/layout`, `content`) | CLI queries, debugging |
| **Push** | `layout_changed` notification | TUI real-time updates |

- TUI caches layout from push notifications
- CLI always queries fresh data from server
- If push is broken, TUI shows stale data but CLI works

**Manual testing workflow:**

```bash
# Start server
./target/release/reovim server --grpc 13000 &

# Test via CLI (Pull - always fresh, --client 1 targets the TUI)
./target/release/reovim cli --grpc 127.0.0.1:13000 capture --client 1 --format plain_text

# Test via TUI (Push - depends on notifications)
./target/release/reovim tui --grpc 127.0.0.1:13000
```

**Key files for layout/notification debugging:**

| File | Purpose |
|------|---------|
| `server/lib/server/src/grpc/input.rs` | Key processing, change accumulation |
| `server/lib/server/src/grpc/state.rs` | Screen content rendering, state queries |
| `server/lib/server/src/session/` | Session and notification logic |
| `clients/tui/src/` | TUI notification handling, layout caching |
| `server/modules/vim/` | Vim-like behavior, mode management |
