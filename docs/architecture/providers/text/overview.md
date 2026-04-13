# Text Content Provider (`reovim-provider-text`)

**Crate**: `reovim-provider-text`  
**Path**: `server/lib/providers/text/`  
**Introduced**: #739 (provider layer), populated incrementally in #740

## Overview

The text content provider houses text-specific buffer implementations and
algorithms that operate on decoded text from any codec (UTF-8, Shift-JIS, xxd,
etc.). It sits above the VFS byte layer and below the session runtime:

```
Session runtime
    |
TextBufferRegistry  (this crate)
    |
BufferOps (dyn trait)
    |
Buffer (Rope, small files)   |   VirtualBuffer (mmap + PieceTree, large files)
    |                                |
reovim-domain-text                  reovim-driver-vfs
    |
reovim-kernel (StorageOps, BufferMeta, BufferId)
```

Buffer selection is caller-controlled. Small files use `Buffer`; large files
use `VirtualBuffer`. Callers are always polymorphic — they hold
`Arc<RwLock<dyn BufferOps>>` and never downcast.

## Key Types

### `BufferOps` trait

Defined in `src/buffer_ops.rs`. The unified polymorphic interface over all
text buffers. Extends two kernel supertraits:

```
StorageOps (byte I/O)  ─┐
                          ├─ BufferOps (text-specific extension)
BufferMeta (identity)  ─┘
```

Required methods beyond the supertraits:

| Method | Purpose |
|--------|---------|
| `buffer_capabilities() -> BufferCapabilities` | Text-level capability flags |
| `line_count() -> usize` | Number of lines |
| `line(idx) -> Option<Cow<str>>` | Line by 0-based index |
| `line_len(idx) -> Option<usize>` | Line length in characters |
| `content_bytes() -> Vec<u8>` | Full content as bytes |
| `position_to_byte(pos) -> usize` | Line/column to byte offset |
| `byte_to_position(offset) -> Position` | Byte offset to line/column |
| `insert_at(pos, text)` | Insert at position |
| `delete_range(start, end) -> String` | Delete range, returns deleted text |
| `set_content(content)` | Replace all content |
| `content() -> String` | Full content as string |
| `write_to(writer)` | Streaming write, no full-string allocation |
| `as_text_geometry() -> &dyn TextGeometry` | Upcast bridge for motion engine |

The trait is object-safe: all methods use `&self`/`&mut self`, return owned
types, and have no generic parameters.

`write_to()` has a default implementation that materializes via `content()`.
`VirtualBuffer` overrides this to iterate PieceTree pieces directly from mmap,
avoiding a full `String` allocation for large files.

`as_text_geometry()` bridges the `dyn BufferOps` vtable to `&dyn TextGeometry`.
Rust cannot coerce between two different `dyn` types automatically, so each
implementor returns `self`.

### `Buffer`

Defined in `src/buffer.rs`. Rope-backed buffer for small files. Migrated from
the kernel in #740.

- Storage: `Rope` (balanced B-tree of text chunks from `reovim-domain-text`)
- Insert/delete: O(log n)
- Clone: O(1) via `Arc<Node>` structural sharing in the rope
- `BufferCapabilities`: `CONTENT_MATERIALIZABLE | SNAPSHOTTABLE | LINE_READABLE | EDITABLE`

Cursor and selection are not stored in `Buffer`. Both are per-client state in
`Window` (removed in #471 and Phase 8/#465 respectively). All edit operations
take explicit `Position` arguments.

`Buffer::snapshot(cursor)` produces a `BufferSnapshot` in O(1) by cloning the
rope arc. The cursor must be passed in from the window — the buffer does not
know which client is requesting the snapshot.

### `VirtualBuffer`

Defined in `src/virtual_buffer.rs`. mmap + PieceTree for large files.

- `original: Arc<dyn FileMapping>` — zero-copy mmap access to the original file
- `pieces: PieceTree` — B-tree with `Arc` structural sharing; O(1) clone
- `add_buffer: String` — append-only buffer for edits
- `line_index: LineIndex` — `Vec<u64>` of newline byte offsets for O(log n) line lookup
- `crlf: bool` — whether the original file used CRLF line endings

Open cost is O(n) for the single-pass `LineIndex` scan of the file bytes.
Subsequent line lookups are O(log n) via binary search over `line_index.offsets`.

`write_to()` iterates PieceTree pieces. Each piece reads from either `original`
(zero-copy from mmap) or `add_buffer` (appended edits), writing directly to the
`Write` sink. No intermediate `String` is allocated.

`BufferCapabilities`: `SNAPSHOTTABLE | STREAMABLE | FILE_BACKED | LINE_READABLE | EDITABLE`

(`CONTENT_MATERIALIZABLE` is absent — materializing a multi-GB mmap file would
be O(n) memory. Callers that need full content use `write_to()` instead.)

### `VirtualSnapshot`

Defined in `src/virtual_snapshot.rs`. Opaque snapshot of `VirtualBuffer` state.

Clone is O(1): `PieceTree` clones via Arc structural sharing; `original` is an
Arc bump; `line_index` and `add_buffer_len` are small value copies.

Fields: `pieces: PieceTree`, `add_buffer_len: usize`, `original: Arc<dyn FileMapping>`,
`line_index: LineIndex`, `crlf: bool`.

Produced by `VirtualBuffer::capture_snapshot()`, consumed by
`VirtualBuffer::restore_snapshot()`.

### `TextBufferRegistry`

Defined in `src/text_buffer_registry.rs`. Session-layer registry for text
buffer access. Lives in the provider layer, independent of the kernel
`BufferManager`.

```rust
pub struct TextBufferRegistry {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn BufferOps>>>>,
}
```

Key methods:

| Method | Purpose |
|--------|---------|
| `get(id) -> Option<Arc<RwLock<dyn BufferOps>>>` | Look up by ID |
| `register(buffer) -> BufferId` | Insert, returns ID from `BufferMeta::id()` |
| `unregister(id)` | Remove and return the Arc |
| `count() -> usize` | Number of registered buffers |
| `list() -> Vec<BufferId>` | All registered IDs |

Implements `Service` from `reovim-kernel::api::v1`, allowing it to be stored
in the `ServiceRegistry` and shared across threads via `Arc<TextBufferRegistry>`.

During the #740 migration, session code switches from `kernel.buffers.get(id)`
to `text_buffers.get(id)` for text operations. The kernel `BufferManager` will
eventually store only `dyn KernelBuffer` (byte-only).

### `BufferCapabilities`

Defined in `src/buffer_caps.rs`. Bitflags describing what operations a buffer
supports. Replaces boolean `is_virtual_buffer()` checks with fine-grained
capability queries.

| Flag | Meaning |
|------|---------|
| `CONTENT_MATERIALIZABLE` | Full content fits in a `String` in memory |
| `SNAPSHOTTABLE` | Supports snapshot/restore for undo |
| `STREAMABLE` | Content is stream-backed (mmap, network) |
| `FILE_BACKED` | Backed by a file on disk |
| `LINE_READABLE` | Supports line-based read access |
| `EDITABLE` | Supports `insert_at` / `delete_range` |

Pre-composed sets: `BufferCapabilities::ROPE` and `BufferCapabilities::VIRTUAL`.

The gRPC layer sends capabilities as a `u32` field in the `BufferInfo` proto.

### `HeapMapping`

Defined in `src/virtual_buffer.rs`. Test helper implementing `FileMapping` over
a `Vec<u8>`. Simulates file content without an actual mmap. Used in unit tests
for `VirtualBuffer` and `VirtualSnapshot`.

## Dispatch Model

The `TextBufferRegistry` is the sole session-level entry point for text buffer
access:

```
session code
    |
text_buffers.get(id)  ->  Arc<RwLock<dyn BufferOps>>
                                  |
                          Buffer  |  VirtualBuffer
                         (small)  |  (large)
```

Callers are always polymorphic. The registry returns a trait object; callers
never see or depend on the concrete type. Dispatch to the correct implementation
happens via the vtable.

For motion and text object calculations, callers call `buf.as_text_geometry()`
to get a `&dyn TextGeometry` compatible with `MotionEngine` and
`TextObjectEngine` from `reovim-domain-text`.

## Related Documents

- `docs/architecture/domains/text/overview.md` — `reovim-domain-text`: pure text types and `TextGeometry` trait
- `docs/architecture/domains/overview.md` — domain abstraction layer
- `docs/architecture/drivers/vfs/overview.md` — VFS byte layer (`FileMapping`, `PieceTree`)
- `docs/architecture/drivers/buffer/overview.md` — kernel buffer driver
- `docs/architecture/event-layers.md` — `TextBufferModified`, `CursorMoved`, `ViewportScrolled`
- `docs/architecture/session-model.md` — `SessionId`, `ClientId`, per-client cursor isolation
