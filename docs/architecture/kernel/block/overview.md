# block/ - Block I/O

Byte-level storage abstraction and universal undo log.

## Source Location

`server/lib/kernel/src/block/`

## Overview

The `block/` subsystem defines the kernel's byte-level I/O contract for all
buffer types. The kernel treats buffers as opaque byte containers — content
interpretation (text lines, character counts, audio samples) happens in codecs
and providers above the kernel layer.

This replaces the former `UndoTree`/`Transaction` design: domain-specific undo
semantics now live in the provider layer (`reovim-driver-undo`), while the
kernel provides a universal append-only byte undo log shared across all
codec/provider combinations.

## Layer Position

```
Kernel:   StorageOps + BufferMeta (byte I/O + identity)
VFS:      ByteBuffer, StreamBuffer (concrete storage)
Codec:    ContentCodec (bytes <-> domain)
Provider: TextProvider, AudioProvider (domain navigation)
```

## Key Types

### `ByteEdit`

Atomic byte-level mutation. Invertible: swap `old_bytes` and `new_bytes` for
the undo operation.

```rust
pub struct ByteEdit {
    pub offset: usize,
    pub old_bytes: Vec<u8>,
    pub new_bytes: Vec<u8>,
}

// Constructors
ByteEdit::insert(offset, data)   // pure insert (no bytes removed)
ByteEdit::delete(offset, old)    // pure delete (no bytes added)
ByteEdit::replace(offset, old, new) // replace old with new
```

### `ByteUndoLog`

Universal append-only undo log. One log per buffer, shared across all
codec/provider switches. Supports linear undo/redo.

```rust
let mut log = ByteUndoLog::new();

log.push(vec![ByteEdit::insert(0, b"hello")]);
log.push(vec![ByteEdit::insert(5, b" world")]);

assert!(log.can_undo());

// Step backward — caller applies inverse_edits() to storage
let entry = log.undo().unwrap();
entry.inverse_edits();  // Vec<ByteEdit> to re-apply

// Step forward — caller applies edits() to storage
let entry = log.redo().unwrap();
entry.edits();
```

Pushing a new entry clears the redo stack (standard linear undo behaviour).
The log is position-agnostic: cursor state lives in the provider's semantic
undo layer (`UndoTree` in `reovim-driver-undo`).

### `ByteUndoEntry`

One atomic undo step: a group of `ByteEdit`s applied together.

```rust
pub struct ByteUndoEntry { /* edits: Vec<ByteEdit> */ }

entry.edits()           // &[ByteEdit] — forward direction
entry.inverse_edits()   // Vec<ByteEdit> — undo direction (reversed, inverted)
```

### `StorageOps`

Object-safe byte I/O trait. All buffer types implement this trait.

```rust
pub trait StorageOps: Send + Sync + 'static {
    fn byte_len(&self) -> usize;
    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize;
    fn capabilities(&self) -> StorageCapabilities;

    // Static buffers (SEEKABLE + EDITABLE)
    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError>;
    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError>;

    // Stream buffers (APPENDABLE)
    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError>;

    // Bulk access
    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8>;
}
```

### `StorageCapabilities`

Bitflags advertising what a storage backend supports:

| Flag | Meaning |
|------|---------|
| `SEEKABLE` | Random-access reads at any offset |
| `EDITABLE` | Insert/delete at arbitrary offsets |
| `APPENDABLE` | Append to end (streams) |
| `FINITE` | Known, fixed total size |
| `PERSISTENT` | Backed by a persistent file |

Preset combinations: `StorageCapabilities::HEAP`, `::MMAP`, `::STREAM`.

### `BufferMeta`

Buffer identity (id, file path) and modification state. Separate from
`StorageOps` to allow independent evolution.

```rust
pub trait BufferMeta: Send + Sync + 'static {
    fn id(&self) -> BufferId;
    fn file_path(&self) -> Option<&str>;
    fn set_file_path(&mut self, path: Option<String>);
    fn is_modified(&self) -> bool;
    fn set_modified(&mut self, modified: bool);
}
```

### `KernelBuffer`

Combined trait for the kernel fd table. Any type implementing both `StorageOps`
and `BufferMeta` automatically implements `KernelBuffer` via a blanket impl.
The kernel `BufferManager` stores buffers as `dyn KernelBuffer`.

```rust
pub trait KernelBuffer: StorageOps + BufferMeta {}
impl<T: StorageOps + BufferMeta> KernelBuffer for T {}
```

### `StorageError`

```rust
pub enum StorageError {
    OffsetOutOfRange { offset: usize, len: usize },
    NotSupported(&'static str),
}
```

## API Exports

```rust
use reovim_kernel::api::v1::{
    ByteEdit, ByteUndoLog, ByteUndoEntry,
    StorageOps, StorageCapabilities, StorageError,
    BufferMeta, KernelBuffer,
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [mm Subsystem](../mm/overview.md) - BufferId and window identifiers
- [IPC Subsystem](../ipc/overview.md) - EventBus used by storage drivers
