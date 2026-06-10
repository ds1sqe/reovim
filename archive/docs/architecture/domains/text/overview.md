# Text Domain (`reovim-domain-text`)

**Crate**: `reovim-domain-text`  
**Path**: `ext/server/domain/text/`  
**Sibling events crate**: `reovim-domain-text-events` at `ext/server/domain/text/events/`  
**Extracted from kernel**: #739 / #740

## Overview

`reovim-domain-text` holds the pure text domain types — values and algorithms
with no kernel dependency and no concrete buffer or rope dependency. Everything
in this crate operates on abstract text geometry.

The crate implements the `Domain` trait from `reovim-domain`:

```rust
pub struct Text;

impl Domain for Text {
    type Position = TextPosition;  // line:column
    type Edit     = TextEdit;      // insert/delete with text
    type Content  = String;        // decoded UTF-8
}
```

This binding flows through all generic codec and provider traits
(`Decode<D>`, `Encode<D>`, `Index<D>`), enabling domain-agnostic
infrastructure.

Zero kernel dependencies. Zero external syntax dependencies.

## Key Types

### `TextPosition` / `Position`

Line/column location in a text buffer. `Position` is a backward-compatibility
alias for `TextPosition`, scheduled for removal in Phase 5 of the #740
migration.

### `TextEdit` / `Edit`

Atomic insert/delete operation. Carries enough information for undo/redo
round-trips. `Edit` is a backward-compatibility alias for `TextEdit`.

### `TextDimensions`

Shape of a text string: line count and last line length.

### `Cursor`

Cursor state: position + anchor + preferred column. Per-client; stored in
`Window`, not `Buffer`.

### `Selection`

Selection state: anchor position + `SelectionMode`.

### `SelectionMode`

Enum: `Character`, `Line`, `Block`.

### `Rope`

Balanced B-tree text storage. O(log n) insert/delete, O(1) clone via
`Arc<Node>` structural sharing. Exposed as `Rope`, `RopeChunks`, `RopeLines`.
Used by `Buffer` in `reovim-provider-text`.

### `Motion` / `MotionEngine`

Pure cursor motion calculations over any `TextGeometry` implementor.
`MotionEngine` methods take `&dyn TextGeometry` — they work identically over
`Buffer`, `VirtualBuffer`, or `SimpleText`.

### `TextObject` / `TextObjectEngine`

Text object range calculations (word, sentence, paragraph, etc.) over
`&dyn TextGeometry`.

### `Direction`, `LinePosition`, `WordBoundary`

Direction and boundary enums used by `MotionEngine`.

### `CharKind`, `WordType`

Character classification for word-boundary detection.

Free functions: `word_start`, `word_end`, `next_word_start`, `next_word_end`,
`word_bounds`, `char_kind`.

### `RegisterBank`

Yank/paste register storage. Multiple named registers. `Register`,
`RegisterContent`, `YankType`.

### `HistoryRing`

Clipboard history ring buffer.

### `UndoTree`

Undo/redo tree. Nodes are `UndoNode`. Results are `UndoResult`.
`EditOrigin` distinguishes user edits from programmatic edits.

### `Transaction`

Atomic group of edits applied together for undo purposes.

### `History` / `HistoryEntry`

Change history log for replay and audit.

### `LineIndex`

`Vec<u64>` of newline byte offsets. Built from raw bytes in a single pass.
Used by `VirtualBuffer` in `reovim-provider-text` for O(log n) line lookup.

See the dedicated section below.

### `Delimiter`

Free functions for matching delimiter pairs (parentheses, brackets, quotes).
`find_delimiter_pair`, `find_matching_delimiter`.

### `SimpleText`

In-memory `TextGeometry` implementor backed by `Vec<String>`. Useful for
testing and doc examples without taking a dependency on `reovim-provider-text`.

## `TextGeometry` Trait

Defined in `src/text_geometry.rs`. The minimal read-only interface over line-
based text. Both `Buffer` (Rope) and `VirtualBuffer` (PieceTree) implement it.

```rust
pub trait TextGeometry {
    fn line_count(&self) -> usize;
    fn line(&self, idx: usize) -> Option<Cow<'_, str>>;
    fn line_len(&self, idx: usize) -> Option<usize>;
    fn is_empty(&self) -> bool { self.line_count() == 0 }  // default
}
```

Three required methods, one default. Object-safe: all `&self`, no generics.

`line()` returns `Cow::Borrowed` for `Rope` (zero-copy, the rope chunk is
already a valid `str` slice) and `Cow::Owned` for `VirtualBuffer` (pieces must
be materialized from the mmap and add-buffer to produce a contiguous `str`).

`MotionEngine` and `TextObjectEngine` accept `&dyn TextGeometry`. Callers pass
the result of `BufferOps::as_text_geometry()` or use `SimpleText` directly in
tests.

## `LineIndex`

Defined in `src/line_index.rs`.

```rust
pub struct LineIndex {
    offsets: Vec<u64>,    // byte offset of each '\n'
    total_bytes: u64,
    has_crlf: bool,
}
```

`offsets[i]` is the byte offset of the (i+1)-th newline. Line 0 starts at
byte 0. Line k (k > 0) starts at `offsets[k-1] + 1`.

Construction:

- `LineIndex::from_bytes(bytes)` — validates UTF-8, returns `Err(InvalidUtf8)` on failure. Single-pass scan.
- `LineIndex::build_from_str(s)` — infallible (already valid UTF-8). Single-pass scan.

Key operations and complexity:

| Operation | Method | Complexity |
|-----------|--------|-----------|
| Line count | `line_count()` | O(1) |
| Byte range for line | `line_byte_range(idx)` | O(1) |
| Line from byte offset | `byte_to_line(byte)` | O(log n) via `partition_point` |
| Byte start of line | `line_to_byte(line)` | O(1) |
| Incremental insert update | `apply_insert(offset, text)` | O(lines) |
| Incremental delete update | `apply_delete(start, end)` | O(lines) |
| Full rebuild | `rebuild(bytes)` | O(n) |

Line semantics:

- Empty content (0 bytes): 0 lines
- `"hello"` (no newline): 1 line
- `"hello\n"` (trailing newline): 2 lines (last line is empty)

`apply_insert` and `apply_delete` maintain the offset vector incrementally.
For large files with small edits this is vastly cheaper than a full `rebuild`.
`has_crlf` is set conservatively: once true, it is never cleared.

`VirtualBuffer` in `reovim-provider-text` uses `LineIndex` for all line lookups
after the initial O(n) open scan.

## Events

The sibling crate `reovim-domain-text-events` (`ext/server/domain/text/events/`)
emits these events via the kernel event bus:

### `TextBufferModified`

Emitted when a buffer's text content is modified by a semantic text edit.

Fields: `buffer_id: BufferId`, `edit: TextEdit`, `start_byte: usize`,
`old_end_byte: usize`, `new_end_byte: usize`.

The byte-range fields (`start_byte`, `old_end_byte`, `new_end_byte`) allow
incremental parsers such as tree-sitter to correlate byte ranges with point
ranges atomically in a single handler callback.

For pure byte-layer notifications without semantic context, subscribe to
`BufferBytesEdited` (kernel substrate event) instead.

### `CursorMoved`

Emitted when the cursor position changes within a text window.

Fields: `window_id: WindowId`, `buffer_id: BufferId`, `from: TextPosition`,
`to: TextPosition`.

Keyed on `WindowId` because text cursors are per-window (vim convention). Two
windows on the same buffer have independent cursors and each emits its own event.

### `ViewportScrolled`

Emitted when a text viewport is scrolled.

Fields: `window_id: WindowId`, `buffer_id: BufferId`, `top_line: u32`,
`bottom_line: u32` (both 0-indexed).

All three events implement `Event` with `priority::NORMAL`.

See `docs/architecture/event-layers.md` for the full event layer taxonomy.

## Related Documents

- `docs/architecture/domains/overview.md` — domain abstraction layer and `Domain` trait
- `docs/architecture/providers/text/overview.md` — `reovim-provider-text`: buffer implementations
- `docs/architecture/event-layers.md` — event taxonomy and priority levels
- `docs/architecture/type-layers.md` — type layers and dependency graph
- `docs/architecture/drivers/vfs/overview.md` — VFS byte layer (`PieceTree`, `FileMapping`)
