# Completion Plugin Issues

Known issues in the completion plugin that need to be addressed.

---

## Issue 1: Missing Filtering Logic (Critical)

### Description

All completion candidates are shown regardless of what the user types. When typing "com", items like "zebra", "apple", etc. appear alongside "complete" and "component".

### Expected Behavior

Only items matching the typed prefix should appear. Fuzzy matching should rank items by relevance.

### Root Cause

The filtering step is completely missing from the saturator:

1. Prefix is correctly extracted in `commands.rs` (lines 44-65)
2. Prefix is stored in `CompletionRequest` and passed to sources via `CompletionContext`
3. **The saturator never filters items based on prefix** (`saturator.rs` lines 136-148)

**Evidence:**
```rust
// saturator.rs:136-148 - NO FILTERING!
let mut items: Vec<CompletionItem> = results.into_iter().flatten().collect();

// Sort by priority then score (but NO filtering!)
items.sort_by(|a, b| {
    a.sort_priority
        .cmp(&b.sort_priority)
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| a.label.cmp(&b.label))
});

items.truncate(max_items);
```

The `nucleo` crate is listed as a dependency in `Cargo.toml` but is never used.

### Fix Approach

1. Use `nucleo` for fuzzy matching in `saturator.rs`
2. Filter items based on prefix match
3. Set `CompletionItem::score` based on fuzzy match quality
4. Sort by score (already implemented, just needs actual scores)

### Related Files

- `plugins/features/completion/src/saturator.rs` (lines 136-148)
- `plugins/features/completion/src/commands.rs` (lines 44-65)
- `plugins/features/completion/Cargo.toml` (nucleo dependency)

---

## Issue 2: No Dismiss on Cursor Movement

### Description

The completion popup stays visible when the user moves the cursor away from the completion context. For example, pressing `h` or `l` in normal mode or using arrow keys doesn't dismiss the popup.

### Expected Behavior

Completion should automatically dismiss when:
- Cursor moves horizontally away from the word start position
- Cursor moves to a different line
- User enters normal mode and moves

### Root Cause

The completion plugin does not subscribe to the `CursorMoved` event.

**Evidence:**

The `CursorMoved` event exists in `lib/core/src/event_bus/core_events.rs` (line 157):
```rust
pub struct CursorMoved {
    pub buffer_id: usize,
    pub from: (usize, usize),
    pub to: (usize, usize),
}
```

Other plugins correctly subscribe to it:
- **Pair plugin** (`plugins/features/pair/src/lib.rs` line 105)
- **LSP plugin** (priority 200)

The completion plugin's subscriptions (`lib.rs` lines 167-322) include:
- `CompletionTriggered`, `RegisterSource`, `CompletionSelectNext/Prev`
- `CompletionDismiss`, `CompletionConfirm`
- `ModeChanged`, `BufferModified`

**Missing:** `CursorMoved` subscription

### Fix Approach

1. Subscribe to `CursorMoved` event in completion plugin
2. Compare `event.to` position with stored `word_start_col` from snapshot
3. Dismiss if cursor moved outside valid completion range

### Related Files

- `plugins/features/completion/src/lib.rs` (subscribe section, lines 167-322)
- `plugins/features/completion/src/cache.rs` (stores `word_start_col`)
- `lib/core/src/event_bus/core_events.rs` (line 157, `CursorMoved` definition)
- `plugins/features/pair/src/lib.rs` (line 105, example subscription)

---

## Issue 3: No Highlight on Matching Characters

### Description

Completion items don't highlight the characters that match the typed prefix. All characters are rendered with the same style.

### Expected Behavior

When user types "com", completion items should display like:
- "**com**plete" (with "com" highlighted)
- "**com**ponent"
- "**com**pressor"

This is standard behavior in VS Code, vim-cmp, and other modern editors.

### Root Cause

1. `CompletionItem` has no field to store match positions
2. Fuzzy matching (nucleo) would provide match indices, but isn't used
3. `window.rs` render loop applies uniform style to all characters

**Evidence:**

```rust
// window.rs:96-122 - all characters get same style
for (idx, item) in snapshot.items.iter().take(max_items).enumerate() {
    let style = if is_selected { &theme.popup.selected } else { &theme.popup.normal };

    for (i, &ch) in label_chars.iter().enumerate() {
        buffer.put_char(x, y, ch, style);  // Same style for ALL chars
    }
}
```

### Fix Approach

1. Add `match_indices: Vec<u32>` field to `CompletionItem` or snapshot
2. During fuzzy matching (in saturator), capture match positions from nucleo
3. In `window.rs` render loop, apply highlight style to matched character positions

### Related Files

- `lib/core/src/completion/mod.rs` (`CompletionItem` struct)
- `plugins/features/completion/src/saturator.rs` (fuzzy matching location)
- `plugins/features/completion/src/window.rs` (render loop, lines 96-122)
- `lib/core/src/highlight/mod.rs` (style definitions)

---

## Priority

1. **Issue 1** (Critical) - Makes completion unusable
2. **Issue 3** (High) - Poor UX without match highlighting
3. **Issue 2** (Medium) - Annoying but users can press Escape
