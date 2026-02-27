# Troubleshooting Guide

This guide helps diagnose and fix common issues with reovim.

## Quick Diagnostics

### Check System Health

Run the health check command:

```vim
:health
```

This displays:
- Rust version
- Terminal capabilities
- LSP server status
- Plugin status

### Enable Debug Logging

```bash
# Enable debug logging via environment
REOVIM_LOG=debug reovim server --grpc 12540

# View logs from a running server
reovim cli --grpc 127.0.0.1:12540 log-tail --level debug
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

## Common Issues

### 1. Editor Won't Start

**Symptom:** Terminal shows error or hangs on startup.

**Solutions:**

1. **Check terminal compatibility:**
   ```bash
   echo $TERM
   # Should be xterm-256color, screen-256color, etc.
   ```

2. **Reset configuration:**
   ```bash
   mv ~/.config/reovim ~/.config/reovim.bak
   reovim
   ```

3. **Run with minimal plugins:**
   ```bash
   REOVIM_LOG=debug reovim server --grpc 12540
   ```

### 2. No Syntax Highlighting

**Symptom:** Code appears without colors.

**Solutions:**

1. **Check treesitter is loaded:**
   ```vim
   :health
   ```
   Look for "Treesitter: OK"

2. **Verify file type detection:**
   - File must have correct extension (`.rs`, `.py`, etc.)
   - Or set filetype manually: `:set filetype=rust`

3. **Check color mode:**
   ```vim
   :set colormode?
   ```
   Ensure terminal supports the mode (`truecolor`, `256`, `ansi`)

4. **Wait for parsing:**
   Large files may take a moment. Check logs for:
   ```
   [DEBUG] Treesitter parse complete
   ```

### 3. LSP Not Working

**Symptom:** No completions, diagnostics, or go-to-definition.

**Solutions:**

1. **Check LSP server is installed:**
   ```bash
   # Rust
   which rust-analyzer

   # Python
   which pyright

   # TypeScript
   which typescript-language-server
   ```

2. **Enable LSP logging:**

   > **Note:** `--lsp-log` is a planned feature (not yet implemented). LSP logs
   > currently appear in the server's standard error stream alongside other
   > tracing output. To capture them, run the server with debug logging enabled:
   > ```bash
   > REOVIM_LOG=debug reovim server --grpc 12540
   > ```

3. **Check LSP status:**
   ```vim
   :health
   ```
   Look for "LSP: Connected" or error messages

4. **Restart LSP:**

   > **Planned feature (not yet implemented):** `:LspRestart` is not yet
   > available. As a workaround, restart the reovim server to reinitialize all
   > LSP connections.

### 4. Keybindings Not Working

**Symptom:** Key presses have no effect or wrong action.

**Solutions:**

1. **Check current mode:**
   Look at the mode indicator in the status line (NORMAL, INSERT, etc.)

2. **Check keybinding exists:**
   Press `Space ?` to open which-key hints

3. **Check for conflicts:**
   Plugins may override default bindings. Enable trace logging and search the
   output for your key:
   ```bash
   REOVIM_LOG=trace reovim server --grpc 12540
   reovim cli --grpc 127.0.0.1:12540 log-tail --level trace
   ```

4. **Reset keymap:**
   Remove custom keybindings from profile:
   ```bash
   rm ~/.config/reovim/profiles/default.toml
   ```

### 5. Display Issues

**Symptom:** Garbled text, wrong colors, or rendering glitches.

**Solutions:**

1. **Check terminal emulator:**
   Recommended: kitty, alacritty, wezterm, iTerm2
   Known issues: some older terminals

2. **Try different color mode:**
   ```vim
   :set colormode=256
   ```

3. **Force redraw:**
   Press `Ctrl-l` to refresh the screen

4. **Check font:**
   Ensure a monospace font with good Unicode support is configured

5. **Disable problematic features:**
   ```vim
   :set noindentguide
   :set noscrollbar
   ```

### 6. Server Mode Connection Issues

**Symptom:** CLI/TUI client can't connect to server.

**Solutions:**

1. **List running servers:**
   ```bash
   reovim cli --grpc 127.0.0.1:12540 clients
   ```

2. **Check server is listening:**
   ```bash
   # Server prints this on startup
   # "Listening on 127.0.0.1:12540"
   ```

3. **Check port availability:**
   ```bash
   netstat -an | grep 12540
   ```

4. **Connect to specific port:**
   ```bash
   reovim cli --grpc 127.0.0.1:12540 keys 'j' --client 1
   ```

5. **Clean up stale port files:**
   ```bash
   rm ~/.local/share/reovim/servers/*.port
   ```

### 7. High CPU Usage

**Symptom:** reovim uses excessive CPU.

**Solutions:**

1. **Check for parsing loops:**
   Enable trace logging and look for repeated parse messages

2. **Reduce treesitter timeout:**
   ```vim
   :set treesitter.timeout=50
   ```

3. **Disable animations:**
   Some themes have animations that may be CPU-intensive

4. **Profile with system tools:**
   ```bash
   # Linux
   perf top -p $(pgrep reovim)

   # macOS
   sample $(pgrep reovim) 5
   ```

### 8. Memory Issues

**Symptom:** High memory usage or OOM errors.

**Solutions:**

1. **Large files:**
   reovim loads entire files into memory. For very large files:
   - Use `less` or `vim` for viewing
   - Split large files

2. **Clear caches:**
   Restart reovim to clear syntax and decoration caches

3. **Check buffer count:**
   Close unused buffers: `:bd` (buffer delete)

### 9. Completion Not Triggering

**Symptom:** `Alt-Space` doesn't show completions.

**Solutions:**

1. **Check completion plugin:**
   ```vim
   :health
   ```
   Look for "Completion: OK"

2. **Verify mode:**
   Completion only works in Insert mode

3. **Check keybinding:**
   Alt may be intercepted by terminal. Try:
   ```vim
   :CompletionTrigger
   ```

4. **Use Ctrl-y to confirm:**
   Tab does NOT work for completion confirm (see Known Issues)

### 10. File Not Saving

**Symptom:** `:w` doesn't persist changes.

**Solutions:**

1. **Check file permissions:**
   ```bash
   ls -la myfile.txt
   ```

2. **Check disk space:**
   ```bash
   df -h
   ```

3. **Try write with path:**
   ```vim
   :w /tmp/backup.txt
   ```

4. **Check for errors:**
   Look at command line for error messages

## Debug Techniques

### Trace Event Flow

```bash
REOVIM_LOG=trace reovim server --grpc 12540
reovim cli --grpc 127.0.0.1:12540 log-tail --level trace
```

### Profile Rendering

```bash
REOVIM_LOG=debug reovim server --grpc 12540
reovim cli --grpc 127.0.0.1:12540 log-tail --level debug
```

### Inspect RPC Communication

```bash
# Server
REOVIM_LOG=debug reovim server --grpc 12540

# Client
reovim cli --grpc 127.0.0.1:12540 keys 'itest<Esc>'
reovim cli --grpc 127.0.0.1:12540 log-tail --level debug
```

### Check Plugin Loading

```bash
REOVIM_LOG=info reovim server --grpc 12540
reovim cli --grpc 127.0.0.1:12540 log-tail --level info
```

## Getting Help

1. **Check documentation:**
   - [Architecture](../architecture/overview.md)
   - [Event System](../events/overview.md)
   - [Module System](../modules/overview.md)

2. **Search existing issues:**
   https://github.com/ds1sqe/reovim/issues

3. **File a bug report:**
   Include:
   - reovim version (`reovim --version`)
   - OS and terminal
   - Steps to reproduce
   - Debug log output

## Known Limitations

### Tab Key in Completion

Tab cannot confirm completion due to keybinding architecture limitations. Use `Ctrl-y` instead.

### Clipboard on Wayland

Clipboard operations require `wl-copy`/`wl-paste` on Wayland:

```bash
# Install on Ubuntu/Debian
sudo apt install wl-clipboard

# Install on Arch
sudo pacman -S wl-clipboard
```

### Terminal-Specific Issues

| Terminal | Issue | Workaround |
|----------|-------|------------|
| macOS Terminal.app | Limited color support | Use iTerm2 |
| tmux | Key escape delays | Set `escape-time 0` |
| SSH | Slow rendering | Use mosh or increase bandwidth |

## Performance Tuning

### Large Files

```vim
:set norelativenumber   " Disable relative line numbers
:set noindentguide      " Disable indent guides
:set noscrollbar        " Disable scrollbar
```

### Slow Syntax Highlighting

```vim
:set treesitter.timeout=50   " Reduce parse timeout
```

### Reduce Logging Overhead

To minimize logging overhead, simply omit the `REOVIM_LOG` environment variable
when starting the server. By default, only `error`-level messages are emitted:

```bash
reovim server --grpc 12540
```
