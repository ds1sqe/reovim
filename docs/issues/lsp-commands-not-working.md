# LSP Commands Not Working

## Status: In Progress

## Problem

LSP keybindings (gd, gr, K) don't work on real Rust files. Commands execute but fail silently or log errors like:
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

## Remaining Issues

### Hover Response Not Displayed

The hover request is sent successfully but the popup doesn't appear. Investigation shows:
- Request is sent: `LSP: hover request sent buffer_id=0 line=49 column=4`
- No follow-up log about hover content received

Possible causes:
1. Response arrives after client disconnects (in test mode)
2. Popup rendering not triggered properly
3. `send_render_signal()` not working as expected

### Investigation Notes

- The `handle.hover()` returns `None` when the channel is full or closed
- rust-analyzer may take time to provide hover information for complex types
- The test disconnects client very quickly after sending keys

## Verification Steps

1. Build: `cargo build --release`
2. Start server: `./target/release/reovim --server --test --log=/tmp/lsp.log runner/src/main.rs`
3. Wait for rust-analyzer to initialize (check log for "Language server ready")
4. Send keys: `cargo run -p reo-cli -- keys 'gg49jwK'`
5. Wait for response: `sleep 5`
6. Capture screen: `cargo run -p reo-cli -- capture`
7. Check logs: `grep -E "(hover|didOpen)" /tmp/lsp.log`

## Related Files

- `plugins/features/lsp/src/document.rs` - DocumentState and DocumentManager
- `lib/lsp/src/client.rs` - LSP client implementation
- `lib/lsp/src/saturator.rs` - Request handling and channel management
