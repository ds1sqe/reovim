---
name: e2e
description: "Ad-hoc E2E verification using real server, headless TUI, and CLI. Spawn infrastructure, send keys, capture frames, verify behavior. Usable by agents during reviews."
---

# E2E - Ad-hoc End-to-End Verification

Spawn a real reovim server + headless TUI + CLI to verify editor behavior interactively.

## Usage

```
/e2e [scenario-description]
```

- `scenario-description` - What to verify (e.g., "test :q works", "verify dd deletes line", "check insert mode")
- If no scenario given, run a smoke test (connect, capture frame, send basic keys, verify mode transitions)

## What This Does

1. **Build** the binary if needed
2. **Spawn** a real server with OS-assigned port
3. **Connect** a headless TUI client (creates a real client session)
4. **Execute** the scenario using CLI commands (keys, capture, mode, cursor, buffer)
5. **Report** pass/fail with captured evidence
6. **Cleanup** all spawned processes (ONLY processes we started)

## Infrastructure Setup Protocol

Follow these steps EXACTLY. Process safety is critical.

### Step 1: Build

```bash
cargo build -p reovim-app 2>&1 | tail -5
```

If build fails, STOP and report the error. Do not proceed with stale binaries.

### Step 2: Check for conflicts

```bash
ps aux | grep "[r]eovim server" | grep -v grep
```

Note existing PIDs so you NEVER kill them. Record your own PIDs for cleanup.

### Step 3: Start server

```bash
./target/debug/reovim server --grpc 0 2>server_stderr.tmp &
SERVER_PID=$!
echo "SERVER_PID=$SERVER_PID"
sleep 2
```

Extract the port from stderr output:

```bash
PORT=$(grep -oP 'Listening on 127\.0\.0\.1:\K\d+' server_stderr.tmp)
echo "PORT=$PORT"
```

If PORT is empty, check `server_stderr.tmp` for errors and STOP.

### Step 4: Connect headless TUI

```bash
./target/debug/reovim tui --grpc "127.0.0.1:$PORT" --headless --width 80 --height 24 &
TUI_PID=$!
echo "TUI_PID=$TUI_PID"
sleep 2
```

### Step 5: Find client ID

```bash
./target/debug/reovim cli --grpc "127.0.0.1:$PORT" clients
```

The headless TUI registers as a client. Use the client ID from the output.
Typically client ID is `1` (first connected client).

### Step 6: Verify connection

```bash
./target/debug/reovim cli --grpc "127.0.0.1:$PORT" ping
./target/debug/reovim cli --grpc "127.0.0.1:$PORT" mode -c 1
```

Expected: ping succeeds, mode shows `NORMAL`.

## CLI Command Reference

All commands use `./target/debug/reovim cli --grpc "127.0.0.1:$PORT"` as prefix.

| Command | Example | What it does |
|---------|---------|--------------|
| `keys -c ID "KEYS"` | `keys -c 1 "ihello<Esc>"` | Send vim-notation keys to client |
| `mode -c ID` | `mode -c 1` | Get current mode (NORMAL, INSERT, etc.) |
| `cursor -c ID` | `cursor -c 1` | Get cursor position (line, col) |
| `buffer` | `buffer` | Get active buffer content |
| `buffer --id N` | `buffer --id 0` | Get specific buffer content |
| `buffers` | `buffers` | List all open buffers |
| `registers` | `registers` | List non-empty registers |
| `registers NAME` | `registers "\""` | Get specific register |
| `capture -c ID -f FMT` | `capture -c 1 -f plain_text` | Capture rendered frame |
| `clients` | `clients` | List connected clients |
| `extensions` | `extensions` | List registered extensions |
| `extension-state KIND -c ID` | `extension-state whichkey -c 1` | Query extension state |

Key notation: `<Esc>`, `<CR>`, `<C-w>`, `<BS>`, etc. Literal characters are sent as-is.

## Timing

- After `keys`, wait 100-200ms before querying state (async dispatch)
- After `:e filename<CR>`, wait 200ms for file load
- After mode changes, wait 50ms
- Use `sleep 0.1` or `sleep 0.2` between send and query

## Cleanup Protocol (MANDATORY)

After ALL testing, clean up ONLY your processes:

```bash
kill $TUI_PID 2>/dev/null; wait $TUI_PID 2>/dev/null
kill $SERVER_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null
rm -f server_stderr.tmp
```

NEVER use `pkill reovim`, `killall reovim`, or any broad kill command.

## Scenario Execution

When executing a scenario, follow this pattern:

1. **Setup** - Any pre-conditions (open file, set buffer content)
2. **Action** - Send the key sequence being tested
3. **Wait** - Brief delay for async processing
4. **Assert** - Query state and verify expectations
5. **Record** - Note pass/fail with evidence

### Example: Verify `dd` deletes a line

```bash
CLI="./target/debug/reovim cli --grpc 127.0.0.1:$PORT"

# Setup: insert two lines
$CLI keys -c 1 "ihello<CR>world<Esc>"
sleep 0.2

# Verify setup
$CLI buffer
# Expected: "hello\nworld"

# Action: go to first line and delete it
$CLI keys -c 1 "ggdd"
sleep 0.2

# Assert
BUFFER=$($CLI buffer)
MODE=$($CLI mode -c 1)
echo "Buffer: $BUFFER"
echo "Mode: $MODE"
# Expected: buffer contains only "world", mode is NORMAL
```

### Example: Verify `:q` quits

```bash
CLI="./target/debug/reovim cli --grpc 127.0.0.1:$PORT"

# Action: send quit command
$CLI keys -c 1 ":q<CR>"
sleep 0.5

# Assert: server should have shut down (or client disconnected)
$CLI ping 2>&1 && echo "STILL RUNNING" || echo "SERVER STOPPED"
```

### Example: Verify insert mode

```bash
CLI="./target/debug/reovim cli --grpc 127.0.0.1:$PORT"

# Action: enter insert mode and type
$CLI keys -c 1 "itest text<Esc>"
sleep 0.2

# Assert
MODE=$($CLI mode -c 1)
BUFFER=$($CLI buffer)
echo "Mode: $MODE"     # Expected: NORMAL (after Esc)
echo "Buffer: $BUFFER"  # Expected: contains "test text"
```

### Example: Capture and inspect frame

```bash
CLI="./target/debug/reovim cli --grpc 127.0.0.1:$PORT"

# Capture the full TUI frame
FRAME=$($CLI capture -c 1 -f plain_text)
echo "$FRAME"

# Check for statusline, tildes, mode indicator, etc.
echo "$FRAME" | grep -q "NORMAL" && echo "PASS: mode shown" || echo "FAIL: no mode"
echo "$FRAME" | grep -q "~" && echo "PASS: tildes shown" || echo "FAIL: no tildes"
```

## Output Format

When reporting results, use this format:

```
## E2E Verification Report

**Scenario**: [description]
**Date**: [date]
**Binary**: [debug/release]
**Server PID**: [pid] (port [port])
**TUI PID**: [pid] (client ID [id])

### Results

| Step | Action | Expected | Actual | Status |
|------|--------|----------|--------|--------|
| 1 | Send `ihello<Esc>` | Buffer: "hello", Mode: NORMAL | Buffer: "hello", Mode: NORMAL | PASS |
| 2 | Send `dd` | Buffer: empty | Buffer: "" | PASS |

### Evidence

[Include relevant buffer content, mode output, or frame captures]

### Verdict: PASS / FAIL
```

## Smoke Test (default scenario)

If no scenario is specified, run this smoke test:

1. Verify server responds to ping
2. Verify initial mode is NORMAL
3. Capture initial frame (should have tildes `~`)
4. Enter insert mode (`i`), verify mode is INSERT
5. Type text (`hello`), verify buffer contains "hello"
6. Exit insert mode (`<Esc>`), verify mode is NORMAL
7. Delete line (`dd`), verify buffer is empty
8. Capture final frame
9. Cleanup

## Instructions

When invoked, you MUST:

1. **Parse the scenario** from user input (or use smoke test if none given)
2. **Build the binary** with `cargo build -p reovim-app`
3. **Follow the Infrastructure Setup Protocol** exactly (Steps 1-6)
4. **Execute the scenario** using CLI commands with proper timing delays
5. **Record all results** in the output format above
6. **Run the Cleanup Protocol** - kill ONLY your PIDs
7. **Report** the verdict (PASS/FAIL) with evidence

CRITICAL SAFETY RULES:
- Track ALL PIDs you spawn
- NEVER use `pkill`, `killall`, or broad process killing
- NEVER kill processes you didn't start
- Always clean up, even if tests fail (use trap or ensure cleanup runs)
- If the server fails to start, do NOT proceed — report the error
- Wait for async operations (100-200ms delays between send and query)

For agent invocation (e.g., from telemetry during review):
- Agents should describe the specific scenarios they want to verify
- Keep scenarios focused — verify the specific behavior under review
- Include the E2E report in the agent's review output
