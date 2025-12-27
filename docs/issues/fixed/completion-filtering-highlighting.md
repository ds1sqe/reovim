# Completion: Filtering, Highlighting, and UI Enhancements

**Status:** Fixed
**Date:** 2024-12-27
**Affected Version:** v0.7.5

## Problems Fixed

### 1. Missing Filtering Logic (Critical)

All completion candidates were shown regardless of typed prefix. Typing "com" showed items like "zebra", "apple" alongside "complete".

**Root Cause:** The saturator collected items but never filtered them based on prefix. The `nucleo` crate was listed as dependency but never used.

### 2. No Match Character Highlighting (High)

Completion items didn't highlight matched characters. All characters rendered with same style.

**Root Cause:** `CompletionItem` had no field to store match positions, and `window.rs` applied uniform style.

### 3. No Dismiss on Cursor Movement (Medium)

Completion popup stayed visible when cursor moved away from completion context.

**Root Cause:** `CursorMoved` event is defined but NOT emitted by runtime.

## Fixes

### Fuzzy Filtering with Nucleo

**File:** `plugins/features/completion/src/saturator.rs`

```rust
use nucleo::{Matcher, Utf32Str, pattern::{AtomKind, CaseMatching, Normalization, Pattern}};

// Filter and score using nucleo fuzzy matching
let prefix = &request.prefix;
if !prefix.is_empty() {
    let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
    let pattern = Pattern::new(prefix, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy);
    let min_score = (prefix.len() as u32).saturating_mul(10);

    items = items.into_iter().filter_map(|mut item| {
        let filter_text = item.filter_text();
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(filter_text, &mut buf);
        let mut indices = Vec::new();

        pattern.indices(haystack, &mut matcher, &mut indices)
            .filter(|&score| score >= min_score)
            .map(|score| {
                item.score = score;
                item.match_indices = indices.to_vec();
                item
            })
    }).collect();
}
```

### Match Highlighting

**File:** `lib/core/src/completion/mod.rs`
```rust
pub struct CompletionItem {
    // ... existing fields
    pub match_indices: Vec<u32>,  // NEW: indices of matched characters
}
```

**File:** `lib/core/src/highlight/theme.rs`
```rust
pub struct PopupStyles {
    // ... existing fields
    pub match_fg: Style,  // NEW: foreground for matched characters
}
```

**File:** `plugins/features/completion/src/window.rs`
```rust
let matched_set: HashSet<u32> = item.match_indices.iter().copied().collect();
for (i, &ch) in label_chars.iter().enumerate() {
    let char_style = if matched_set.contains(&(i as u32)) {
        // Apply match highlight
    } else {
        base_style.clone()
    };
    buffer.put_char(col, row, ch, &char_style);
}
```

### UI Columns (Kind + Source)

**File:** `plugins/features/completion/src/window.rs`

Added kind column (left) and source column (right):
- Kind: Colored abbreviation (`fn`=magenta, `mod`=yellow, `st`=green)
- Source: Dimmed source name (`buffer_words`, `lsp`)

### Prefix Replacement

**File:** `lib/core/src/event_bus/core_events.rs`
```rust
pub struct RequestInsertText {
    pub text: String,
    pub move_cursor_left: bool,
    pub delete_prefix_len: usize,  // NEW: delete N chars before inserting
}
```

This fixes `pkg` -> `CARGO_PKG` instead of `pkgCARGO_PKG`.

### Dismiss Behavior (Partial)

Since `CursorMoved` is not emitted, dismiss is handled via:
- `ModeChanged` - dismisses when leaving insert mode
- `BufferModified` - dismisses on whitespace, validates cursor on re-trigger

## Files Changed

1. `lib/core/src/completion/mod.rs` - Added `match_indices` field
2. `lib/core/src/event_bus/core_events.rs` - Added `delete_prefix_len` field
3. `lib/core/src/highlight/theme.rs` - Added `match_fg` style
4. `lib/core/src/runtime/core.rs` - Handle prefix deletion
5. `plugins/features/completion/src/lib.rs` - Live update, dismiss logic
6. `plugins/features/completion/src/saturator.rs` - Nucleo filtering
7. `plugins/features/completion/src/window.rs` - Kind/source columns, highlighting
8. `plugins/features/pair/src/lib.rs` - Updated RequestInsertText usage
