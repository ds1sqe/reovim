# Codec Driver Overview

`reovim-driver-codec` (`ext/server/drivers/codec/`)

## Overview

The codec driver mediates between raw on-disk bytes and the decoded text or
structured views that the rest of the editor operates on. Its design mirrors
Linux VFS: an **inode** holds one authoritative copy of a file's bytes, and one
or more **mounts** attach codec views to that inode. The decoded representation
is always derived from canonical bytes; bytes are never reconstructed from
decoded text on save.

Mechanism vs policy split:

- **Driver** (`ext/server/drivers/codec/`) — traits, storage primitives,
  factory registry, edit pipeline.
- **Modules** (`server/modules/codec-*/`) — implement `ContentCodec` and
  `ContentCodecFactory` for specific formats (UTF-8, CJK, hex, CSV, tar.gz,
  PDF, rlib, ...).

## Key Types

| Type | Location | Role |
|------|----------|------|
| `InodeId` | `inode.rs` | `NonZeroU64`-backed inode identifier. |
| `MountId` | `inode.rs` | `NonZeroU64`-backed mount identifier. |
| `MountHandle` | `inode.rs` | Composite `(InodeId, BufferId, MountId)` — the stable token returned by `mount()` and passed to `apply_edit()` / `flush()`. |
| `MountMode` | `inode.rs` | `Summary` (default) or `Structural`. Summary mounts reject `DecodedEdit::Tree` at the mount level. |
| `Mount` | `inode.rs` | One codec view: `name`, `Arc<dyn ContentCodec>`, `content_valid` flag, `MountMode`. |
| `Inode` | `inode.rs` | `Arc<dyn ByteSource>` (canonical bytes) + optional on-disk `path` + `HashMap<MountId, Mount>`. |
| `InodeTable` | `inode.rs` | Session-scoped registry — inodes, `files` (BufferId→InodeId), `mount_idx` (MountId→InodeId). |
| `DecodedEdit` | `decoded_edit.rs` | Three-variant edit type: `Text`, `Bytes`, `Tree`. |
| `ContentCodec` | `codec.rs` | Core trait: `decode()`, `translate_edit()`, `views()`. |
| `ContentCodecFactory` | `factory.rs` | Module trait: `create(content_type)` → `Option<Arc<dyn ContentCodec>>`. |
| `ContentCodecFactoryStore` | `store.rs` | Prioritised factory registry; `find()` returns the first codec matched by descending priority. |
| `CodecSessionState` | `state.rs` | `SessionExtension` that owns `InodeTable` + per-buffer metadata + active view + indices. |

## Inode and Mount Architecture

```
BufferId ──► InodeTable.files ──► InodeId ──► Inode
                                               ├── bytes: Arc<dyn ByteSource>   (canonical)
                                               ├── path: Option<Arc<Path>>
                                               └── mounts: HashMap<MountId, Mount>
                                                            ├── Mount { name, codec, content_valid, mode }
                                                            └── Mount { ... }   (additional views)
```

A `BufferId` maps to an `InodeId` through the `files` table. Each `Inode` owns
canonical bytes via `Arc<dyn ByteSource>`. Mounts are attached to the inode, not
to the buffer directly; multiple buffers could theoretically share an inode,
though the current session model binds one buffer per inode.

`InodeTable.mount_idx` is a secondary index from `MountId` to `InodeId` so
`flush()` and `apply_edit()` can resolve the owning inode from a bare `MountId`
or `MountHandle` in O(1).

## Codec Classification

### Faithful text

UTF-8, CJK (EUC-KR, Shift-JIS, ...), legacy encodings, CSV.

`translate_edit` returns `Ok(Some(ByteEdit))`. Decoded text coordinates are
translated to byte offsets and applied to `inode.bytes` atomically.
`:w` writes canonical bytes directly — no re-encode step.

### Faithful bytes

Hex dump (`codec-xxd`).

Same `Ok(Some(ByteEdit))` contract, but coordinates are byte offsets in the
decoded output space rather than text positions.

### Transforming (read-only summary views)

ELF, `.rlib` summary, zip manifest, and similar one-way projections.

The default `translate_edit` implementation returns
`Err(TranslateEditError::ReadOnly)`. `InodeTable::apply_edit` surfaces this as
`EditError::ReadOnly`. No write path exists by construction; no runtime flag is
needed.

### Structural

tar.gz, PDF, or any format where the user edits a parsed tree rather than raw
bytes.

The mount is created with `MountMode::Structural`. `translate_edit` receives a
`DecodedEdit::Tree { path, op }`, resolves the `TreePath` in the codec's
mount-local parse cache, applies the `TreeOp`, and returns `Ok(Some(ByteEdit))`
immediately. The tree cache is always derived from `inode.bytes`; it is never a
parallel source of truth.

## DecodedEdit

```rust
pub enum DecodedEdit {
    Text   { start: Position, end: Position, replacement: String },
    Bytes  { offset: usize, old_len: usize, new_bytes: Vec<u8> },
    Tree   { path: TreePath, op: TreeOp },
}
```

`TreePath` is a `Vec<String>` of format-agnostic path components, e.g.
`["sections", "text", "bytes"]` for ELF or `["metadata", "title"]` for PDF.

`TreeOp` wraps a `Box<dyn AnyTreeOp>`. Each codec module defines its own
concrete op enum and registers it via `impl_tree_op!(MyFormatOp)`. The driver
layer never depends on format-specific operation types; codec `translate_edit`
implementations recover the concrete type via `TreeOp::downcast_ref::<T>()`.

## Write Path

`:w` → `InodeTable::flush(mount_id, path_override)`

1. Resolve the owning inode via `mount_idx`.
2. Determine the target path (explicit override, or `inode.path`).
3. Call `inode.bytes.write_to(&mut file)`.
4. Populate `inode.path` on first save of a scratch buffer.

No codec encode method is invoked. Bytes are the source of truth; saving writes
what is already there.

## Edit Pipeline

`InodeTable::apply_edit(handle, edit)` return semantics:

| Result | Meaning | Side effects |
|--------|---------|--------------|
| `Ok(Some(byte_edit))` | Edit accepted and applied. | `inode.bytes` mutated; all peer mounts marked `content_valid = false`; source mount stays `true`. |
| `Ok(None)` | Codec accepted edit as a clean no-op. | No mutation, no stale-marking, no undo append. |
| `Err(EditError::ReadOnly)` | Codec returned `ReadOnly`, or `MountMode::Summary` rejected a `Tree` edit. | No mutation. |
| `Err(EditError::Unsupported)` | Codec returned `UnsupportedEdit`. | No mutation. |
| `Err(EditError::InvalidEdit)` | `ConstraintViolation` or `MalformedPath`. | No mutation. |
| `Err(EditError::ApplyFailed)` | Infrastructure failure. | No mutation. |

## Multi-Mount

`InodeTable::mount_additional(inode_id, buffer_id, mount)` attaches a second
(or Nth) codec view alongside any existing mounts on the same inode.
`InodeTable::mount()` is used for the first mount only.

When an edit is applied through one mount, all peer mounts on the same inode
have their `content_valid` flag set to `false`. Consumers observe staleness via
the `StaleCheck` hook (see below) and re-decode from `inode.bytes` before the
next read.

`CodecSessionState::mount_codec()` is the orchestration entry point used by the
`MountCodec` gRPC handler. It selects `mount()` or `mount_additional()` based
on whether the inode already has any mounts.

## StaleCheck

`reovim-driver-session` defines the `StaleCheck` trait and calls it from
`SessionRuntime::buffer_content`. The codec driver provides
`InodeStaleCheck` in `stale_check.rs` as the adapter installed at bootstrap.

Current state: the adapter body is a probe only. It increments an atomic
counter and emits a `tracing::trace!` event per call, enabling test-side
verification and log-tap observability. The full re-decode-on-read path — when a
peer mount is stale, re-decode from `inode.bytes` before the client reads — is a
known limitation (B4) deferred until `CodecSessionState` migrates out of the
per-call `ExtensionMap` borrow (tracked under `#740`).

The seam is installed and the contract is defined; the body is the open edge.

## Factory Priority

```rust
// Default priority (100). Used by codec modules with no priority opinion.
factory_store.add_factory(factory);

// Explicit priority. Higher value wins. Equal-priority ties break by
// first-registered-wins (insertion order preserved internally).
factory_store.add_factory_with_priority(factory, priority);
```

`ContentCodecFactoryStore::find(content_type)` iterates registered entries in
descending-priority, stable-insertion-order and returns the first
`Arc<dyn ContentCodec>` produced. Priority is registration metadata on the store;
it does not appear on `ContentCodec` itself.

## Domain-Generic Codec Traits

`domain_codec.rs` defines a parallel, domain-aware trait layer for future
multi-domain support (text, audio, PCM, ...):

| Trait | Purpose |
|-------|---------|
| `Decode<D>` | Stateless `raw bytes → D::Content`. |
| `Encode<D>` | Stateless `D::Content + metadata → bytes`. Optional for read-only codecs. |
| `ByteNotifiable` | Type-erased byte-edit notification. Stored in `CodecSessionState::indices`. |
| `Index<D>` | Stateful index: `build()`, `notify(ByteEdit)`, `to_bytes()`, `offset_to_position()`, `translate_edit()`. |

`ByteNotifiable` is the object-safe subset of `Index<D>` stored in
`CodecSessionState` so the session runtime can route `ByteEdit` notifications
without knowing the domain type. B4 (see StaleCheck above) applies here: the
notification path is wired at the type level but has no production callers yet.

## gRPC RPCs

Defined in `server/lib/server/src/grpc/buffer.rs`:

| RPC | Direction | Description |
|-----|-----------|-------------|
| `MountCodec` | client → server | Attach a codec mount to a buffer's inode. Returns the assigned `MountId`. |
| `UmountCodec` | client → server | Detach a mount by `MountId`. |
| `ListMounts` | client → server | Enumerate all active mounts on a buffer's inode (`MountInfo` list). |
| `ListAvailableCodecs` | client → server | Enumerate every registered factory and its supported content types. |
| `SwitchCodecView` | client → server | **Deprecated** (`#740` Plan 06 Phase 5). Use `MountCodec` / `UmountCodec` / `ListMounts` instead. Scheduled for removal in v0.11.0. |

`MountCodec` accepts a `mount_mode` field: `0` = `Summary` (default), `1` =
`Structural`.

## Known Limitations

| ID | Severity | Description |
|----|----------|-------------|
| B2 | HIGH | `DecodeResult::readonly` is set by codecs but discarded by every consumer. The field will be removed once `translate_edit → Err(ReadOnly)` is the sole writability seam (Phase 3, `#740`). |
| B4 | LATENT | `ByteNotifiable` index notification path is wired but has no production callers. Index drifts after edits. Becomes load-bearing in Phase 5 once `Mount.index` participates in multi-mount peer notification (`#740`). |

## Related Documents

- `docs/architecture/drivers/overview.md` — driver layer overview
- `docs/architecture/drivers/vfs/` — `ByteSource`, `FileHandle`, `HeapByteSource`
- `docs/architecture/drivers/session/` — `SessionExtension`, `StaleCheck`, `ExtensionMap`
- `docs/architecture/drivers/buffer/` — `BufferId`, `ByteEdit`
- `ext/server/drivers/codec/src/lib.rs` — crate-level doc comment and re-exports
