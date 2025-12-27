# reo-cli: Backspace Not Working in Microscope

**Status:** Fixed
**Date:** 2024-12-27
**Affected Version:** v0.7.5

## Problem

When sending `<BS>` or `<Backspace>` via `reo-cli keys` in microscope interactor mode, the backspace key was not deleting characters from the query input.

```bash
# This was NOT working - backspace had no effect
cargo run -p reo-cli -- keys '<Space>ff'  # Open files picker
cargo run -p reo-cli -- keys 'itest'       # Type "test"
cargo run -p reo-cli -- keys '<BS>'        # Expected: "tes", Actual: "test" (unchanged)
```

## Root Cause

The microscope plugin subscribed to `PluginTextInput` events but **not** `PluginBackspace` events.

**Event flow:**
1. `CommandHandler` detects Backspace in a mode where `accepts_char_input()` returns true
2. Sends `TextInputEvent::DeleteCharBackward` (BEFORE keymap lookup)
3. Runtime converts this to `PluginBackspace { target: microscope }`
4. **BUG:** Microscope had no subscription for `PluginBackspace` - event was dropped

**Comparison:** Both Explorer and WhichKey plugins correctly subscribe to both `PluginTextInput` AND `PluginBackspace`.

## Fix

Added `PluginBackspace` subscription to microscope plugin:

**File:** `plugins/features/microscope/src/lib.rs`

```rust
// Added to imports
core_events::{PluginBackspace, PluginTextInput, RequestFocusChange, RequestModeChange},

// Added subscription handler
bus.subscribe::<PluginBackspace, _>(100, move |event, ctx| {
    if event.target != COMPONENT_ID {
        return EventResult::NotHandled;
    }

    let is_active = state_clone
        .with::<MicroscopeState, _, _>(|s| s.active)
        .unwrap_or(false);

    if is_active {
        state_clone.with_mut::<MicroscopeState, _, _>(|s| {
            s.delete_char();
        });
        ctx.request_render();
        EventResult::Handled
    } else {
        EventResult::NotHandled
    }
});
```

## Additional Enhancement

Enhanced `reo-cli keys` response to include parsed keys list for better debugging:

```json
{
  "injected": 7,
  "keys": ["i", "H", "e", "l", "l", "o", "<Esc>"]
}
```

## Verification

```bash
# Start server
cargo run -- --server --test &

# Test backspace now works
cargo run -p reo-cli -- keys '<Space>ff'  # Open files picker
cargo run -p reo-cli -- keys 'itest'       # Type "test"
cargo run -p reo-cli -- keys '<BS>'        # Query: "tes" ✓
cargo run -p reo-cli -- keys '<BS><BS>'    # Query: "t" ✓
```

## Files Changed

1. `plugins/features/microscope/src/lib.rs` - Added `PluginBackspace` subscription
2. `lib/core/src/rpc/server.rs` - Enhanced keys response with keys list
