# vfs/ - Virtual Filesystem

File operations abstraction.

## Source Location

`lib/drivers/vfs/src/`

## Key Traits

```rust
pub trait VfsDriver: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write(&mut self, path: &Path, content: &[u8]) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    fn create_dir(&mut self, path: &Path) -> Result<()>;
    fn remove(&mut self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Metadata>;
    fn mmap_read(&self, path: &Path) -> Result<MappedFile>;
}
```

> Note: The trait shown above is a simplified view. The actual `VfsDriver` trait has evolved significantly beyond these 9 methods. The `mmap_read` method was added as part of the large-file subsystem (#739); see the Large File Support section below.

## Types

```rust
pub struct DirEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Metadata {
    pub size: u64,
    pub modified: SystemTime,
    pub is_readonly: bool,
}
```

## Large File Support (#739)

The large-file subsystem adds zero-copy, mmap-backed infrastructure for working with
files that are too large to load into a heap buffer. The components live in
`server/lib/subsys/vfs/src/`.

### PieceTree

Located in `server/lib/subsys/vfs/src/piece_table.rs`.

A B-tree data structure with `Arc` structural sharing that enables O(1) clone. It is
byte-only — it carries no text semantics (no line indexing, no encoding). Higher layers
supply encoding and line-splitting on top of `PieceTree`.

### FileMapping trait

Zero-copy access to the original file bytes. Implemented by types that wrap a memory-mapped
or in-memory view of a file.

```rust
pub trait FileMapping: Send + Sync {
    fn as_bytes(&self) -> &[u8];
    fn len(&self) -> u64;
    fn is_stale(&self) -> bool;
}
```

- `as_bytes()` — returns a byte slice over the mapped region.
- `len()` — total byte length of the mapped file.
- `is_stale()` — returns `true` when the underlying file has changed since the mapping
  was created (e.g., external write detected via inode change).

### MappedFile

A real memory-mapped file wrapper backed by `memmap2::Mmap` behind an `Arc`. Cloning a
`MappedFile` is cheap — it increments the `Arc` reference count without copying any bytes.
Implements `FileMapping`.

### ByteSource trait

The inode-layer byte source abstraction. Drivers implement `ByteSource` to expose a
uniform read interface regardless of whether data is heap-resident or mmap-backed.

```rust
pub trait ByteSource: Send + Sync {
    fn len(&self) -> u64;
    fn read(&self, range: Range<u64>) -> Cow<'_, [u8]>;
    fn as_slice(&self) -> Option<&[u8]>;
    fn write_to(&self, dest: &mut dyn Write) -> Result<u64>;
    fn capabilities(&self) -> ByteSourceCapabilities;
    fn as_any(&self) -> &dyn Any;
}
```

- `len()` — total byte length.
- `read(range)` — byte-range read; returns `Cow<'_, [u8]>`, either a zero-copy borrowed
  slice or an owned copy depending on the backing store.
- `as_slice()` — returns `Some(&[u8])` when the full content is directly addressable
  (e.g., mmap or heap slice); `None` for streaming sources.
- `write_to()` — stream the full content to a `Write` sink without an intermediate
  allocation.
- `capabilities()` — returns the bitflag set describing what this source supports.

### ByteSourceCapabilities

Bitflags describing the capabilities of a `ByteSource` implementation.

| Flag | Meaning |
|------|---------|
| `RANDOM_ACCESS` | Supports arbitrary byte-range reads without seeking overhead |
| `WRITABLE` | Underlying storage can be written back |
| `STREAMING` | Data arrives incrementally; random access is not guaranteed |
| `MMAP_BACKED` | Backed by a kernel memory mapping; `as_slice()` will return `Some` |

### HeapByteSource

An `Arc<Vec<u8>>`-backed `ByteSource`. Reports `RANDOM_ACCESS`. Used for small files
and in-memory buffers where mmap overhead is not warranted.

### MappedByteSource

An mmap-backed `ByteSource` wrapping `MappedFile`. Reports `RANDOM_ACCESS | MMAP_BACKED`.
`as_slice()` always returns `Some`. Used for large files where avoiding a heap copy is
important.

### mmap_read on VfsDriver

```rust
fn mmap_read(&self, path: &Path) -> Result<MappedFile>;
```

Opens a file and returns a `MappedFile` (cheap-clone mmap wrapper). The caller owns the
resulting mapping independently of the `VfsDriver` — closing or reloading the driver does
not invalidate it. Use `is_stale()` to detect external modifications.

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
