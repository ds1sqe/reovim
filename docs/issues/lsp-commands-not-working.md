# LSP Commands Not Working

## Status: NOT FIXED

## Problem

LSP keybindings (gd, gr, K) don't work on real Rust files. Despite multiple fixes and enhancements, the core functionality remains broken:
- `gd` (goto definition) - Never navigates to definition
- `gr` (goto references) - Never shows references picker
- `K` (hover) - Never shows hover popup

Commands execute but fail silently or log errors like:
```
LSP: hover - no document or handle buffer_id=0
```

## Root Causes Identified

### Issue 1: Async Event Processing Race Condition

The keybindings ARE registered correctly. The real issue is a race condition between:
- **File loading** (synchronous during startup)
- **Event processing** (asynchronous in EventBus background task)

```
Timeline:
1. create_buffer_from_file() runs synchronously
2. event_bus.emit(FileOpened{...}) queues event (non-blocking)
3. Function returns -> user can press keys
4. EventBus background task processes FileOpened (eventually)
5. LSP handler registers document

Commands run at step 3, but document isn't registered until step 5.
```

**Fix Applied**: Added `ensure_document_registered()` in `LspRenderStage::transform()` to synchronously register documents during the first render cycle. This handles the race condition because render always happens before user sees output.

**File**: `plugins/features/lsp/src/stage.rs`

### Issue 2: mark_opened() Called Unconditionally

In `stage.rs`, `mark_opened()` was called AFTER the `with_mut` closure, even when the closure returned early (e.g., when handle was None).

```rust
// BEFORE (buggy):
self.manager.with_mut(|m| {
    // ... if handle is None, return early
    if doc.opened { ... } else {
        handle.did_open(...);
    }
});
// This was called even if closure returned early!
self.manager.with_mut(|m| {
    m.documents.mark_opened(buffer_id);
});

// AFTER (fixed):
self.manager.with_mut(|m| {
    // ...
    if doc.opened {
        handle.did_change(...);
    } else {
        handle.did_open(...);
        // Only mark opened after actually sending didOpen
        m.documents.mark_opened(buffer_id);
    }
});
```

**File**: `plugins/features/lsp/src/stage.rs`

### Issue 3: LSP Server Startup Timing

Documents were opened before rust-analyzer finished initializing. The `FileOpened` event would try to send `didOpen` but handle was None.

**Fix Applied**: Simplified `FileOpened` handler to just register document and schedule sync. Added code in `boot()` to send `didOpen` for all pending documents after `set_connection()` completes.

**File**: `plugins/features/lsp/src/lib.rs`

### Issue 4: Request Channel Capacity Too Small (Potential)

The LSP saturator uses a channel with capacity=1 for backpressure:
```rust
let (request_tx, request_rx) = mpsc::channel::<LspRequest>(1);
```

If requests come in faster than they can be processed, `try_send()` returns `Full` and the request is dropped. This was logged at `debug` level, making it invisible in normal operation.

**Fix Applied**: Changed logging to `info` level for better visibility.

**File**: `lib/lsp/src/saturator.rs`

## Files Modified

| File | Change |
|------|--------|
| `plugins/features/lsp/src/stage.rs` | Added `ensure_document_registered()`, fixed `mark_opened()` placement |
| `plugins/features/lsp/src/lib.rs` | Simplified FileOpened handler, added boot() logic for pending documents |
| `lib/lsp/src/saturator.rs` | Improved channel error logging |

## Enhancements Made (UI Ready, But Core Broken)

The following improvements were made assuming the LSP responses would arrive:

### Hover Popup Improvements
- **Coordinate calculation fixed** - Properly transforms buffer to screen coordinates
- **Markdown rendering added** - Uses `MarkdownDecorator::parse_decorations()` for syntax highlighting
- **Channel buffer increased** - From 1 to 32 to prevent request drops

### Multiple Definition Picker
- **LspDefinitionsPicker added** - Shows picker when `gd` returns multiple locations
- Matches `gr` (references) picker UX

**However, none of these improvements matter because the LSP responses never arrive.**

## Remaining Issues (CRITICAL)

### Core Problem: LSP Responses Never Received

All three commands (gd, gr, K) share the same failure mode:
1. Request is sent to rust-analyzer (logged)
2. No response is ever received
3. UI improvements are never triggered

Possible root causes to investigate:

1. **Response channel disconnected** - The `oneshot::Receiver` may be dropped before response arrives
2. **Response handler not polling** - The spawned async task may not be running properly
3. **rust-analyzer never responds** - Server may be stuck or not processing requests
4. **Event loop blocked** - Runtime may not be processing async responses

### Investigation Required

```rust
// In lib.rs, the response handling spawns a task:
tokio::spawn(async move {
    match rx.await {  // <-- Does this ever complete?
        Ok(Ok(Some(response))) => { ... }
        ...
    }
});
```

Questions to answer:
1. Does the `rx.await` ever resolve?
2. Is the tokio runtime properly processing the spawned task?
3. Is the oneshot sender dropped prematurely?

### Debug Steps Needed

1. Add logging INSIDE the spawned task (before `rx.await`)
2. Add logging for oneshot channel creation/drop
3. Check if rust-analyzer stdout is being read
4. Verify the response parsing in saturator works

## Verification Steps

1. Build: `cargo build --release`
2. Start server: `./target/release/reovim --server --test --log=/tmp/lsp.log runner/src/main.rs`
3. Wait for rust-analyzer to initialize (check log for "Language server ready")
4. Send keys: `cargo run -p reo-cli -- keys 'gg49jwK'`
5. Wait for response: `sleep 5`
6. Capture screen: `cargo run -p reo-cli -- capture`
7. Check logs: `grep -E "(hover|didOpen|definition|references)" /tmp/lsp.log`

## Related Files

- `plugins/features/lsp/src/document.rs` - DocumentState and DocumentManager
- `plugins/features/lsp/src/lib.rs` - Event handlers and async response tasks
- `lib/lsp/src/client.rs` - LSP client implementation
- `lib/lsp/src/saturator.rs` - Request handling, channel management, response routing
