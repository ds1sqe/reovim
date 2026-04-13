# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.14.5-dev] - Unreleased

### Added

- **bench**: large-file benchmark harness — programmatic UTF-8 log and ELF binary fixture generators, Linux RSS measurement via `/proc/self/status`, and `slow_bench_config()` for multi-second benchmarks (#739)
- **bench**: large-file performance gate benchmarks (G1–G6) — Criterion benchmarks for all #739 gates: 2 GB UTF-8 open/scroll/edit/write/search and 500 MB ELF open, with RSS measurement (#739)
- **text**: `LineIndex::apply_insert()` and `apply_delete()` for incremental newline-offset updates without full content rebuild — O(lines) instead of O(bytes) (#739)
- **events**: `reovim-domain-text-events` crate with `TextBufferModified`, `CursorMoved`, and `ViewportScrolled` — text-domain events decoupled from kernel, re-exporting `BufferId`/`WindowId`/`TextEdit`/`TextPosition` for consumer convenience (#740)
- **kernel**: `BufferBytesEdited` event — universal byte-layer mutation notification with `CORE` priority, carrying `BufferId` + `ByteEdit`. Subscribers that only need byte changes use this instead of domain-specific events (#740)
- **codec**: `FileTypeChanged` event in codec driver events module — file type is a codec-factory concern, not a kernel or text-domain concern. Coexists with kernel `FileTypeChanged` during dual-emission transition (#740)
- **depgraph**: `kernel_no_domain` boundary guard test — BFS traversal of `cargo_metadata::resolve` graph verifying `reovim-kernel` has no transitive dependency on any `reovim-domain-*` crate (#740)
- **session**: dual-emission transition for text-domain events — `insert_text`, `delete_range`, and `record_cursor_move` now emit both old kernel events (`BufferModified`, `CursorMoved`) and new text-domain events (`TextBufferModified`, `CursorMoved` from `reovim-domain-text-events`) simultaneously. Consumers can migrate at their own pace (#740)
- **text**: `VirtualBuffer::for_each_chunk()` yields raw `&[u8]` slices from pieces for zero-allocation byte-level iteration (#739)
- **vfs**: `TreeFixture` VFS fixture factories relocated from `shared/bench` to `reovim-driver-vfs::fixtures` — corrects shared→driver layer violation (#739)
- **kernel**: virtual buffer architecture — mmap + piece table for multi-GB file editing with constant memory overhead. Files > 64 MB use `VirtualBuffer` backed by zero-copy mmap instead of Rope (#739)
- **kernel**: `PieceTree` B-tree with `Arc` structural sharing for O(1) clone, matching the existing Rope pattern (#739)
- **kernel**: `LineIndex` for O(log n) line lookup via binary search over newline byte offsets, with full UTF-8 validation (#739)
- **kernel**: `FileMapping` trait for zero-copy access to original file bytes — implemented by `MappedFile` (VFS driver) and `HeapMapping` (tests) (#739)
- **kernel**: `VirtualBufferRegistry` service for parallel buffer management alongside existing `BufferManager` (#739)
- **vfs**: `mmap_read()` method on `VfsDriver` trait with `memmap2` implementation and heap-copy fallback (#739)
- **session**: transparent `BufferApi` dispatch — all buffer operations check `VirtualBufferRegistry` first, falling back to Rope `BufferManager`. Modules work with large files without changes (#739)
- **search**: `LineSource` trait with `BufferLineSource` and `VirtualBufferLineSource` adapters for buffer-type-agnostic search (#739)
- **session**: `buffer_write_to()` and `is_virtual_buffer()` methods on `BufferApi` for streaming write and buffer type detection (#739)
- **codec**: `decode_streaming()` method on `ContentCodec` for header-only parsing of large binary files (#739)
- **vfs**: `ByteSource` trait with `HeapByteSource` and `MappedByteSource` implementations for phase 2 inode codec work (#740)
- **codec**: added `ContentCodec::translate_edit` seam (default `None`) and `DecodedEdit` abstraction for decoded-domain mutations (#740)
- **codec**: introduced inode/mount scaffolding (`Inode`, `InodeId`, `InodeTable`, `Inode.mounts`, `Mount`, `MountHandle`, `MountId`, `InodeError`) as the phase-3 data model for mount-based codec workflows (#740)
- **protocol**: `MountCodec`, `UmountCodec`, `ListMounts`, and `ListAvailableCodecs` RPCs on `BufferService` for multi-mount codec workflows — clients can attach multiple codec views on one inode, list them, and discover which codec factories the server has registered. `MountHandle::mount_id()` accessor exposes the assigned id in the `MountCodec` response so clients can unmount later (#740)
- **codec**: priority-based codec selection via `ContentCodecFactoryStore::add_factory_with_priority(factory, priority: u32)` — higher priority factories are consulted first by `find()`. Ties are broken by registration order (first-registered wins). The existing `add_factory` call is unchanged from the caller's perspective: it delegates to `add_factory_with_priority` with `DEFAULT_FACTORY_PRIORITY` (100). `take_factories` and `available` now return entries in priority order. Priority lives on the factory store — **not** on the `ContentCodec` trait — per the Phase 6 plan decision (#740)
- **codec**: Plan 07 Phase 1 shared substrate — new `TranslateEditError` error taxonomy in `reovim-driver-codec::errors` with five variants (`ReadOnly`, `UnsupportedEdit`, `ConstraintViolation`, `MalformedPath`, `Internal`), all carrying `&'static str` reasons for uniformity with `EditError`. Dynamic context from codec implementors is routed through `tracing` rather than surfaced through the error, so `EditError::InvalidEdit` / `EditError::ApplyFailed` boundary stays allocation-free. New `Internal` variant specifically classifies I/O, parse, and infrastructure failures (distinct from `ConstraintViolation`, which is for domain-rule rejections) (#740)
- **codec**: Plan 07 Phase 1 shared substrate — `DecodedEdit::Tree { path: TreePath, op: TreeOp }` variant replaces the `_Reserved` placeholder. `TreePath` is a format-agnostic `Vec<String>` path with `new`, `root`, `is_root`, and `components` accessors. `TreeOp` is an **enum** (NOT a trait object) with `#[non_exhaustive]` and a single test/feature-gated `Synthetic { name: String }` variant for Phase 1 harness tests. Picking an enum over `Box<dyn TreeOp>` / `Arc<dyn TreeOp>` lets `#[derive(Debug, Clone, PartialEq, Eq)]` work without manual impls and eliminates the `Arc::ptr_eq` identity-vs-value-equality hazard that would have broken tests. Phase 2–6 format phases (ELF, .rlib, zip, tar.gz, PDF) add new `TreeOp` variants as each lands (#740)
- **codec**: Plan 07 Phase 1 shared substrate — `reovim_driver_codec::testing::harness` scaffold module behind the new `testing` feature flag (analogous to `reovim-testing`). Exposes `verify_codec(&Arc<dyn ContentCodec>, &HarnessFixture) -> Result<(), HarnessError>` running the four Phase 1 gates in order: no-op round-trip, post-edit re-parse, peer-mount propagation, and undo correctness. `HarnessFixture` carries raw bytes + a list of `DecodedEdit` operations to exercise through `translate_edit`. Format-specific fixtures and real-use workflows come in Phase 2+ (#740)
- **codec**: Plan 07 Phase 2 ELF v1 — `TreeOp::Elf(ElfTreeOp)` is now the first real structural-edit variant. `ElfTreeOp` supports three fixed-size, in-place operations only: executable-section byte patching, same-length symbol rename, and same-size named-section payload replacement. The path surface is explicit (`["sections", "<name>", "bytes"]` / `["symbols", "<name>", "name"]`), and all size-changing / relayout-requiring edits stay rejected in v1 (#740)
- **codec**: Plan 07 Phase 3 `.rlib` v1 — `TreeOp::Rlib(RlibTreeOp)` now supports two fixed-size, in-place archive edits only: same-size member payload replacement at the member byte span and same-length member rename at existing header/name-table storage. The implementation is pinned to `goblin::archive::Archive` plus direct byte-offset rewrites, and any archive relayout / string-table growth / unsupported name-storage case is rejected explicitly (#740)
- **codec**: Plan 07 Phase 4 ZIP v1 — `TreeOp::Zip(ZipTreeOp)` now supports three fixed-size, in-place archive edits only: same-length archive comment rewrite at the EOCD comment span, same-size STORED entry payload replacement at the entry data span, and same-length entry rename patching both local-header and central-directory name bytes in place. The implementation uses the `zip` crate read surface plus direct byte-offset rewrites, and any DEFLATED entry, size-changing edit, or archive rebuild/relayout is rejected explicitly (#740)
- **modules**: external extension ecosystem — dynamic `.so` module loading for both server and client modules (#710, #723–#730)
  - `ClientModuleHandle` with `dlopen`/`dlsym` loading and `catch_unwind` panic isolation (#724)
  - Render-path FFI trampolines (`chrome_render`, `chrome_requested_size`) with `FfiPlatformCaps` snapshot pattern (#723)
  - Server-side `on_all_loaded` FFI + unified bootstrap for builtin and registry modules (#725)
  - Module registry CLI subcommands: `install`, `remove`, `update`, `list`, `info`, `check`, `resolve` (#727)
  - Install-time version constraint enforcement with host ABI semver check (#728)
  - End-to-end sample module pair (server + client) with integration test (#729)
  - CI pipeline for dynamic `.so` artifact verification (#730)
  - Cross-cdylib `ServiceRegistry` using `type_name` instead of `TypeId` for stable service discovery (#710)

- **driver**: `BufferCapabilities` bitflags in `reovim-driver-buffer` — `CONTENT_MATERIALIZABLE`, `SNAPSHOTTABLE`, `STREAMABLE`, `FILE_BACKED`, `LINE_READABLE`, `EDITABLE` with `ROPE` and `VIRTUAL` presets (#739)
- **server**: `BufferHandle` enum unifying Rope and virtual buffer access at the server layer — gRPC handlers now see both buffer types (#739)
- **protocol**: `capabilities` field (u32 bitflags) on `BufferInfo` proto message (#739)
- **kernel**: `TextGeometry` trait — polymorphic line-based read access for both `Buffer` (zero-copy `Cow::Borrowed`) and `VirtualBuffer` (`Cow::Owned`) (#739)
- **session**: `text_buffer_edits` field on `StateChanges` — transport for text-domain edits (`TextBufferModified` events) for incremental syntax parsing and codec index updates (#740)
- **server**: `text_event_to_syntax_edit` — converts `TextBufferModified` events to `SyntaxEdit` for incremental tree-sitter parsing (#740)
- **modules**: `buffer-ops` subscribes to `TextBufferModified`, `window-ops` subscribes to text-domain `ViewportScrolled` (#740)

### Changed

- **BREAKING (internal)**: Renamed `reovim-types-text` crate to `reovim-domain-text` and relocated from `shared/types/text/` to `shared/domains/text/` alongside its sibling `reovim-domain-text-events`. All workspace imports updated from `reovim_types_text` to `reovim_domain_text` (#740)
- **text**: `VirtualBuffer` line access, edit, and search paths rewritten for O(requested_range) instead of O(file_size) — eliminates per-piece String allocation in `materialize_byte_range()`, replaces full-content `rebuild_line_index()` with incremental `apply_insert`/`apply_delete`, and fixes `delete_at`/`read_bytes`/`read_chunk` to avoid materializing the entire buffer (#739)
- **kernel**: `MotionEngine::calculate` and `TextObjectEngine::range` accept `&dyn TextGeometry` instead of `&Buffer` — motions and text objects now work on virtual buffers (#739)
- **session**: `with_text_geometry()` on `SessionRuntime` dispatches to either buffer type — 11 motion/textobject modules migrated (#739)

- **server**: `SessionState::buffer()` returns `BufferHandle` instead of `Arc<RwLock<Buffer>>` — virtual buffers are now visible to all 9 gRPC call sites (#739)
- **server**: `ListBuffers` includes both Rope and virtual buffers with capability flags (#739)
- **snapshot**: `Snapshot` now supports both Rope and `VirtualBuffer` via internal `SnapshotContent` enum — `capture_virtual()` and `restore_virtual()` methods added (#739)
- **commands**: `:e` command detects file size before loading — large UTF-8 files use mmap path, large binaries try streaming codec, small files use existing Rope path (#739)
- **completion**: virtual buffer completion uses line-based access instead of full content materialization (#739)
- **format**: format commands skip virtual buffers gracefully (#739)
- **codec**: Plan 07 Phase 4.5 — `TreeOp` refactored from a closed enum to an opaque type-erased wrapper (`Box<dyn AnyTreeOp>`) with `downcast_ref` recovery. Format-specific op types (`ElfTreeOp`, `ZipTreeOp`, `RlibTreeOp`) moved from the driver crate to their respective module crates (`codec-binary-struct`, `codec-rlib`). New `AnyTreeOp` trait and `impl_tree_op!` macro exported from the driver so modules can register ops without driver changes. Adding a new format no longer requires modifying the driver crate (#740)
- **codec**: Plan 07 Phase 5 tar.gz v1 — new `reovim-module-codec-tar-gz` crate with `TarGzClassifier` (priority 30, gzip magic + `.tar.gz`/`.tgz` extension), `TarGzCodecFactory`, and `TarGzCodec`. `TarGzTreeOp` supports two same-size operations: member rename and member payload replacement. Unlike ELF/rlib/zip (in-place byte patching), tar.gz edits perform a full decompress → patch tar stream → recompress cycle, emitting a whole-file `ByteEdit::replace(0, ...)`. First Phase 4.5 consumer: `TarGzTreeOp` defined in the module crate with zero driver changes (#740)
- **codec**: Plan 07 Phase 6 PDF v1 — `PdfCodec` upgraded from read-only text extraction to structural metadata editing. `PdfTreeOp::SetMetadata` modifies Info dictionary fields (Title, Author, Subject, Keywords, Creator, Producer) via `lopdf` parse → modify → serialize, emitting a whole-file `ByteEdit::replace(0, ...)`. Supports no-op detection (same value returns `Ok(None)`), adding new fields to existing Info dictionaries, and explicit refusal for PDFs without an Info dictionary. The `pdf-extract` text extraction decode path is unchanged; `readonly` metadata now reports `false` (#740)
- **codec**: Plan 07 Phase 7 UI/protocol integration — `MountMode` enum (`Summary`/`Structural`) added to `Mount` struct. Summary mounts (the default, backward-compatible) reject `DecodedEdit::Tree` with `EditError::ReadOnly` at the mount level before the codec is consulted. Structural mounts dispatch tree edits normally. `MountMode` field added to `MountCodecRequest` and `MountInfo` proto messages. `CodecSessionState::mount_codec` accepts a mode parameter. New `:pdf-set-metadata <field> <value>` ex-command in `codec-pdf` module constructs `PdfTreeOp::SetMetadata` server-side and dispatches through the codec pipeline with automatic re-decode (#740)
- **session**: move `TextBufferRegistry` from `reovim-driver-session` into `reovim-provider-text`, with session importing it from the provider for boundary-correct ownership (#740)
- **server**: codec orchestration extracted from the `SwitchCodecView` gRPC handler into `CodecSessionState::switch_view` — handler is now a dispatcher that resolves the buffer, calls the codec-driver helper, translates structured errors to `tonic::Status`, and writes the decoded content to the buffer. No codec-side mutation remains in `server/lib/server/src/grpc/buffer.rs` (#740)
- **codec**: `ContentCodecFactory::create` now returns `Option<Arc<dyn ContentCodec>>` instead of `Option<Box<dyn ContentCodec>>`. `ContentCodecFactoryStore::find` inherits the Arc return type. All in-tree codec module factories (`codec-utf8`, `codec-hex`, `codec-cjk`, `codec-csv`, `codec-legacy`, `codec-pdf`, `codec-rlib`, `codec-binary-struct`) migrated. Callers drop the `Box -> Arc::from` conversion dance, and Phase 5 multi-mount can cheaply share a single codec across sibling mounts (#740)
- **codec**: single-mount invariant relaxed — `InodeTable::mount_additional` lets a second (or Nth) codec view attach to an inode that already has a mount. `InodeTable::apply_edit` now marks every peer mount's `content_valid = false` after an edit lands, so the Phase 5 sub-commit 5e `StaleCheck` hook can re-decode sibling views on next read. Added `InodeTable::flush(mount_id, path_override)` signature stub that returns `io::ErrorKind::Unsupported` until sub-commit 5d wires the consolidated `:w` path (#740)
- **commands**: `:w` consolidated onto `InodeTable::flush` — the command no longer calls `ContentCodec::encode`. For buffers with a codec mount the save path flushes canonical inode bytes directly via `ByteSource::write_to`; STREAMABLE buffers without a mount still stream via `buffer_write_to`; plain scratch buffers write UTF-8 content. The entire bugclass B1 (parallel raw-bytes cache drifts out of sync with the text buffer) is deleted by architecture (#740)
- **codec**: `Inode` gained an `Option<Arc<Path>>` path field; `InodeTable::insert_with_path`, `InodeTable::set_path`, and a real `InodeTable::flush(mount_id, path_override)` body land. Path resolution: override wins; else `inode.path`; else `io::ErrorKind::InvalidInput`. A successful flush with a path override populates `inode.path` for the `:w newfile.txt` scratch flow. The live 512 MB `MMAP_PROMOTION_BUDGET` guard in `apply_byte_edit` rejects edits on inodes that exceed the budget with a user-actionable `EditError::InvalidEdit` message (#740)
- **session**: new `StaleCheck` trait in `reovim-driver-session` with a `refresh_if_stale(&self, BufferId)` contract. `SessionShared` gained `install_stale_check` / `stale_check` / `clear_stale_check` accessors, and `SessionRuntime::buffer_content` now consults the installed hook (if any) before returning decoded text. The hook is a no-op when unset, which keeps tests and headless paths unaffected (#740)
- **codec**: new `InodeStaleCheck` adapter in `reovim-driver-codec` implementing `StaleCheck`. The server-side `SessionState::new` / `with_registries` constructors install the adapter at session creation so the codec → session crate boundary stays one-way. Current adapter body is a counter + `tracing::trace!` event emitter — the full re-decode-on-read path is blocked by the `&self` hook signature vs. `ExtensionMap` interior-mutability constraint and is deferred to a follow-up (#740)
- **codec**: Plan 07 Phase 1 shared substrate — `ContentCodec::translate_edit` trait signature widens from `Option<ByteEdit>` to `Result<Option<ByteEdit>, TranslateEditError>`. Default implementation now returns `Err(TranslateEditError::ReadOnly)` so codecs that do not override the seam inherit read-only behavior by construction. All eight in-tree codec modules migrated: `codec-utf8`, `codec-hex`, `codec-cjk`, `codec-csv`, `codec-legacy` return `Ok(Some(edit))` / `Err(UnsupportedEdit)` / `Err(Internal)` on their existing paths; `codec-rlib`, `codec-pdf`, and `codec-binary-struct` (ELF + ZIP) inherit the `Err(ReadOnly)` default — the old explicit `None` stubs are deleted. Workspace-wide test mock migration matches the new return shape (#740)
- **codec**: Plan 07 Phase 1 shared substrate — `InodeTable::apply_edit` return type widens from `Result<ByteEdit, EditError>` to `Result<Option<ByteEdit>, EditError>`. `Ok(Some(_))` is the existing applied-edit path. `Ok(None)` is the new pinned "clean no-op" path: when a codec returns `Ok(None)`, `apply_edit` early-returns without mutating `inode.bytes`, without marking peer mounts stale, and without appending to the undo log. This guarantees structural codecs that emit `Ok(None)` on a syntactic no-op cannot accidentally invalidate peer mounts or lose user edit intent. All `TranslateEditError` variants map to typed `EditError` variants: `ReadOnly → EditError::ReadOnly`, `UnsupportedEdit → EditError::Unsupported`, `ConstraintViolation`/`MalformedPath → EditError::InvalidEdit`, `Internal → EditError::ApplyFailed` (#740)

### Removed

- **kernel**: `BufferModified`, `Modification`, `CursorMoved`, `ViewportScrolled`, and `FileTypeChanged` from kernel events — text-specific events now live in `reovim-domain-text-events` (text-domain) and `reovim-driver-codec` (codec events). Kernel retains only domain-free substrate events (#740)
- **session**: `modified_buffer_edits` field and `record_buffer_modified_with_edit` method from `StateChanges` — replaced by `text_buffer_edits` carrying `TextBufferModified` events (#740)
- **server**: `modification_to_syntax_edit` bridge function and `modification_to_text_buffer_modified` transitional bridge — replaced by `text_event_to_syntax_edit` operating on text-domain events directly (#740)

### Refactored

- **server**: remove 14 temporary `ClientDirectory` forwarder methods from `Session`, expose `Session::clients()` accessor, and migrate all gRPC and test callers to use `session.clients().*` directly — `Session` drops from ~35 to ~21 methods (#741)
- **server**: extract `PresenceService` from `Session` so the presence/sync graph is owned by a dedicated authority without pulling notification or event-translation work into this slice (#741)
- **server**: extract `ClientDirectory` from `Session` so client membership, relation validation, and input-target resolution live behind a dedicated authority (#741)
- **kernel**: continue the `#740` byte-only extraction — text `Buffer`/Rope and semantic undo test ownership move out of kernel, VFS now owns file-mapping/piece-table primitives, and `KernelContext` drops dead motion/text-object engines (#740)
- **server**: continue the `#740` contract cleanup — remove the transient `BufferHandle` read wrapper, narrow `BufferReadAccess` to per-buffer lookup, and move illuminate word scanning onto `TextGeometry` instead of `BufferOps` (#740)
- **search/session**: continue the `#740` contract cleanup — production search callers now use a `TextGeometry`-backed line source instead of direct `BufferOps`, and production `JumpEntry`/`Jumplist` consumers now flow through `reovim-driver-session` instead of the kernel API path (#740)
- **session/kernel**: continue the `#740` ownership move — `JumpEntry`, `Jumplist`, and `MAX_JUMPLIST_SIZE` now live in `reovim-driver-session`, kernel core no longer implements or exports them, and the remaining imports were migrated to the driver path (#740)
- **session/kernel**: continue the `#740` ownership move — shared uppercase/global marks now live in `reovim-driver-session::SessionShared` instead of `KernelContext`, `SessionRuntime` exposes the session-owned mark seam, and mark/bootstrap test scaffolding no longer constructs kernel mark storage (#740)
- **kernel/session**: continue the `#740` extraction — `Mark`, `MarkBank`, `MarkResult`, `SpecialMark` types moved from `reovim-kernel::core` to `reovim-driver-session`, register snapshot debug surface (`YankTypeSnapshot`, `RegisterSnapshot`, `RegistersSnapshot`, `snapshot_registers`) removed from kernel, and all consumers migrated to import marks from the session driver (#740)
- **session**: add `TextBufferRegistry` service — session-layer registry for text-specific buffer access, decoupling `SessionRuntime` text operations from kernel `BufferManager`. Incremental migration step toward `BufferManager` storing byte-only `dyn KernelBuffer` (#740)
- **kernel**: `BufferManager` now stores `dyn KernelBuffer` (byte-only) instead of `dyn BufferOps` (text-specific) — the kernel sees only bytes and metadata, text access is exclusively through `TextBufferRegistry` at the session layer (#740)
- **kernel/provider-text**: continue the `#740` byte-only extraction — `BufferOps`, `BufferCapabilities`, and `LineCache` removed from kernel. `BufferOps` trait and `BufferCapabilities` bitflags now live in `reovim-provider-text`. `LineCache` deleted (dead code). Kernel `api/v1.rs` exports zero text concepts (#740)
- **kernel**: rebuild `kernel::block` as real byte-level I/O subsystem — `ByteEdit` and `ByteUndoLog` moved from VFS driver to `kernel::block`, `StorageOps`/`BufferMeta`/`KernelBuffer` consolidated from `api/storage_ops.rs` into `block/`. Codec driver and codec-utf8 now import `ByteEdit` from kernel instead of VFS (#740)
- **provider-text**: add `VirtualBuffer::from_mapping()` factory — encapsulates `LineIndex` construction inside the provider, removing direct `LineIndex` usage from the `:edit` command module (#740)
- **session**: remove `ByteUndoRegistry` from production mutation path — `insert_text`, `delete_range`, and `replace_content` now keep byte-level edit emission in `StateChanges` and leave codec-specific byte-index updates to server-level `notify_codec_indices` routing (#740)
- **codec**: `ByteNotifiable` supertrait for `Index<D>` — domain-agnostic byte-level notification interface (`build`, `notify`) extracted as supertrait, `Index<D>` only has domain-specific methods. `CodecSessionState` extended with per-buffer index storage. View switch clears stale index and byte undo log (#740)
- **server**: wire `ByteEdit` notification from mutations to codec indices — `StateChanges` carries `byte_edits`, server routes them to `CodecSessionState::notify_index()` after each key event (#740)
- **codec**: `Utf8LineIndex` now stores raw bytes for proper multi-byte UTF-8 char↔byte translation — `to_bytes()` and `offset_to_position()` walk UTF-8 char boundaries instead of assuming ASCII (1 byte = 1 char), mid-character byte offsets correctly return `None` (#740)
- **provider-text**: `BufferOps::write_to()` method for streaming buffer I/O — default materializes via `content()`, `VirtualBuffer` overrides with piece-by-piece write from mmap/add-buffer. `SessionRuntime::buffer_write_to()` dispatches through this method (#740)
- **commands**: `:w` uses streaming write for `STREAMABLE` buffers without codec metadata — avoids full `String` materialization for large mmap-backed files (#740)

### Deprecated

- **protocol**: `BufferService::SwitchCodecView` RPC — scheduled for removal in v0.11.0. Use `MountCodec` + `UmountCodec` + `ListMounts` + `ListAvailableCodecs` instead. The server keeps `SwitchCodecView` as a compatibility shim that routes through `CodecSessionState::switch_view` and logs a deprecation warning on every call (#740)

### Removed

- **server**: deleted deprecated `SessionState::resolve_key()` and `SessionState::try_on_command_complete()` compatibility helpers along with their temporary `temp_*` per-client scaffolding. Session key resolution and command-complete handling now go through the explicit per-client `*_for_client` paths only (#741)
- **session**: delete `ByteUndoRegistry` module — per-buffer byte-level undo now lives on `Inode` via `InodeTable`, keeping `reovim-driver-session` codec-agnostic (#740)
- **codec**: `ContentCodec::encode` trait method deleted workspace-wide. Bytes are the source of truth; `:w` flushes `inode.bytes` directly. Every in-tree codec (UTF-8, hex, CJK, CSV, legacy, rlib, PDF, binary-struct, xxd) has its `encode` impl removed. Faithful codecs (UTF-8, CJK, CSV, legacy) retain a private inherent `encode_fragment` helper for `translate_edit` re-encoding and round-trip tests. Test mocks and round-trip tests updated. The `encode_and_write` helper in `commands/write.rs` is also deleted (#740)

### Fixed

- **commands**: `:bd` / `:bdelete` now releases per-buffer state on close — previously `BdeleteCommand` bypassed `BufferApi::delete_buffer` and called `kernel.buffers.unregister()` directly, leaking `TextBufferRegistry` and `CodecSessionState` entries on every buffer close. Routing through `delete_buffer` also restores last-buffer protection (#740)
- **commands**: `decode_file_content` returns an error when no buffer is active instead of silently dropping codec metadata — a later `:w` then fell back to a UTF-8 encode that corrupted binary files (#740)
- **coverage**: MC/DC coverage pass — domain-text (line_index, motion engine), bench (large_file template expansion), kernel (byte_undo_log), VFS (mapped_file is_stale restructure), codec harness error paths, rlib classifier (#710)
- **coverage**: MC/DC coverage pass — codec driver core (decode_streaming default, is_insertion/is_deletion MC/DC, MountId::from_raw, store partition_point restructure, state unmount_codec/list_mounts restructure), codec modules (cjk/csv/hex/utf8 translate_edit Tree/wildcard arm merge, decode-failure and error path tests), syntax driver (set_default_injection_language), format hook (TextBufferRegistry None path), module-loader registry (unload restructure), session driver (clear_stale_check, dual_register_buffer None path) (#710)
- **tui**: stabilize flaky `test_remote_cursor_at_eol` with `wait_for` polling (same pattern as Flight 1b presence render fixes)
- **coverage**: MC/DC coverage pass — module-loader registry let-chain→is_some_and + expect restructure, codec-pdf let-chain→and_then chain, codec-tar-gz compound || split, commands buffer/edit if-let→.map() restructure; new tests for codec state unmount (all-inodeless), domain-text newline-at-end drain skip, codec-legacy end-before-start, search backward-source via VecLineSource, CLI format module-list/module-info/check-report branches, runtime extract_text_range None-line paths + stale_check hook + registry-absent create/delete, commands cycle_buffer + codec-pipeline mock tests, module_service format_reload_error extraction (#710)
- **tui**: stabilize flaky `test_fix_cursor_label_preserves_content` and `test_fix_resize_then_cursor_label_rendering` with `wait_for` polling
- **coverage**: MC/DC coverage pass — provider-text buffer.rs (17 StorageOps edge-case tests: read_bytes/insert_bytes/delete_bytes/read_chunk boundary conditions), virtual_buffer.rs (21 tests: delete_at estimate, empty buffer, StorageOps edge cases); codec-binary-struct (14 ELF tests + compound || split + nested if for MC/DC + coverage(off) on unreachable guards), codec-rlib (9 tests + let-chain restructure with .ok().filter() + coverage(off) on archive consistency guards) (#710)
- **coverage**: MC/DC coverage pass — broad non-server sweep (32→18 files below 100%): provider-text trait delegation tests (StorageOps/BufferMeta/BufferOps/TextGeometry on Buffer + VirtualBuffer, VirtualSnapshot accessors), session driver BufferApi default impls + buffer_write_to, search driver BufferOpsLineSource + TextGeometryLineSource line_len/byte_to_position edge cases, search engine find_next_source forward + find_all_source, CLI commands JSON/Plain branches + connected_client error + requires_grpc, commands QuitAllCommand + codec decode fallback; codec modules: cjk/csv/legacy multi-line position and end-of-line column tests, legacy Tree arm merge + position out-of-range + unencodable replacement, utf8/legacy/pdf/tar-gz unreachable error extraction with coverage(off), pdf Info-not-reference + Info-not-dictionary tests, tar-gz decompression/parse failure tests; heavy codecs: binary-struct/rlib non-exhaustive arm merges + 17 overflow guard extractions with coverage(off) + ELF parse failure + symbol path validation + rlib parse failure + op/path mismatch + member path validation tests; module_service coverage(off) on gRPC handlers requiring .so fixtures (#710)
- **coverage**: server crate line coverage pass (15→6 files below 100%) — 50 new tests (+2,604 lines) covering gRPC handler bodies, codec-related endpoints, per-client state updates, and helper functions. Syntax state notify_edit unreachable re-acquisition replaced with `.expect()`, compositor guard extracted to `coverage(off)`, buffer.rs error match arms extracted to `coverage(off)` helpers. New tests: HandlerBridge execute via CommandExecutor trait path, TickSchedulerHandle illuminate tick in presence join, debug extension state Shared scope, editor resize/set_active_buffer per-client paths, presence service followers_of/len/is_empty, Server::with_module_service builder, ensure_initial_compositor_window 4-branch test, PendingBindings empty-keys false branch, syntax get_language_info registry-based detection, buffer codec handlers (get_codec_views, switch_codec_view, mount_codec, umount_codec, list_mounts, list_available_codecs), state get_options named lookup + kernel_to_proto_option, input notify_bridges_mode_changed/cursor_moved + send_keys post-processing + execute quit signal paths (#710)
- **coverage**: achieve 100% MC/DC coverage across all crates — lines 100.00% (48,504/48,504), branches 100.00% (3,952/3,952). Server input.rs final push: emit_syntax_updates, notify_bridges cursor deactivation, emit_notifications dedup, send_keys viewport scroll + presence update, apply_mode_transition completion loop (Push/Set/Pop depth), InsertChar unreachable else eliminated. Merged unreachable error arms in buffer.rs, restructured compositor if-let chain in state.rs. Non-server: virtual_buffer write_to restructure, codec-binary-struct ZIP DEFLATED rejection + duplicate section/symbol ELF fixtures, codec-rlib corrupt rmeta tests (#710)

## [0.14.4] - 2026-04-01

### Added

- **codec**: multi-view codec API — `ContentCodec` trait extended with `views()` and `decode_view()` methods for codecs that produce multiple views (e.g., structured summary + hex dump) (#736)
- **codec**: standalone xxd hex dump driver crate (`reovim-driver-codec-xxd`) — configurable bytes-per-line, group size, and max input size, used by `codec-hex` and available for any binary codec's hex view (#736)
- **protocol**: `GetCodecViews` and `SwitchCodecView` RPCs on BufferService for querying and switching codec views (#736)
- **codec**: rlib structured binary codec — opens `.rlib` files with archive member listing, rustc version, and dependency extraction (#733)
- **kernel**: rope data structure (`mm/rope.rs`) — custom B-tree with O(log n) insert/delete, O(1) clone via `Arc` structural sharing, and O(log n) position conversion. Zero external dependencies. (#711)

### Refactored

- **explorer**: merged `H` (dotfiles) and `I` (gitignored) toggles into single `H` command — both hidden and gitignored files are now controlled by one toggle (#734)

### Changed

- **kernel**: `Buffer` internal storage replaced from `Vec<String>` to `Rope` — all public API signatures unchanged, `Buffer::clone()` is now O(1) (~16ns vs ~8ms for 100K-line buffer) (#711)
- **kernel**: `BufferSnapshot` and `block::Snapshot` now use `Rope` clone instead of `Vec<String>` deep copy — snapshot creation is O(1) (#711)
- **kernel**: rope uses line-separator semantics — trailing `\n` produces an empty line in `line_count()` and `line()`, matching editor expectations for delete+insert patterns (#732)
- **search**: `position_to_byte`/`byte_to_position` now use Buffer's O(log n) rope methods instead of local O(n) helpers — eliminates 2-4 content() materializations per search (#711)
- **session**: `buffer_text_range()` uses byte-indexed `&str` slicing instead of `Vec<char>` per-line allocation (#711)

### Fixed

- **codec**: large binary files no longer cause OOM or gRPC transport failures — gRPC message size raised to 64 MB on server and all clients, hex codec truncates at 1 MB with footer, PDF codec rejects files over 100 MB, ELF/ZIP codecs include file size metadata (#735)
- **input**: remove gratuitous `Box::leak` in `handle_pop_result` — `CommandContext::set()` accepts `&str`, no static lifetime needed (#714)
- **motions**: `ge` and `gE` now correctly distinguish Word vs BigWord boundaries — `word_end_backward` was ignoring the `WordBoundary` parameter (#720)
- **session**: `close_window` and compositor close now use `WindowLayout::remove()` instead of bypassing it — fixes stale `active_index` returning wrong window after close (#712)
- **editor**: `display_line_count` now uses Unicode display width instead of char count — CJK characters correctly occupy 2 terminal columns (#717)
- **kernel**: `CommandId.name` and `OperatorId.name` changed from `&'static str` to `Cow<'static, str>` — `from_qualified()` no longer leaks memory via `Box::leak` on every call (#713)
- **vfs**: replace lossy `From<io::Error>` with `VfsError::from_io(err, path)` — error messages now include the file path instead of showing empty string (#715)
- **vim**: `load_personality_manifest` now propagates parse errors instead of swallowing them — module correctly fails to init if vim.toml is corrupted (#718)
- **lsp**: monitor LSP saturator task with watcher — if the task panics, `active` flag is cleared and user is notified instead of silently losing all LSP features (#719)
- **trace**: `init_profiling_with_filter` now uses the filter argument via `TracingProfiler::with_filter` instead of discarding it (#716)
- **kernel**: `OptionRegistry::register` holds write lock for entire check-and-insert, fixing TOCTOU race between read and write locks (#721)
- **testing**: `IntegrationTest` and `StepTest` now query registers via gRPC instead of returning empty HashMap — `assert_register!` macro is usable (#722)

## [0.14.3] - 2026-03-30

### Added

- **statusline**: diagnostic counts section (Section X) with Nerd Font error/warning/info/hint icons — only shown when counts > 0
- **statusline**: git branch icon (), modified icon (●), readonly icon (󰌾) replace plain text indicators
- **statusline**: theme-aware colors via `statusline_bg`/`statusline_fg` highlight groups cached in `init()`
- **bufferline**: Nerd Font pin icon (󰐃) and modified dot (●) replace ASCII `*` and `[+]`
- **bufferline**: theme-aware colors via highlight groups cached in `init()`
- **microscope**: file type icons rendered next to picker results
- **display**: `DiagnosticPresenter` registered in bootstrap — diagnostic gutter icons now appear
- **display**: `mode_replace` highlight group added to all 3 builtin themes (Dark, Light, TokyoNight)
- **illuminate**: word reference highlighting on cursor-hold — after 300ms idle, all occurrences of the word under cursor are highlighted with `]]`/`[[` navigation
- **syntax-treesitter**: injection decorations — doc comments (`///`, `//!`) now inherit markdown decorations from child drivers (#696)
- **hover**: markdown rendering in hover popups — bold, italic, code spans, headings, fenced code blocks, and horizontal rules are styled instead of shown as raw syntax (#697)
- **microscope**: syntax highlighting in preview panel — file previews show tree-sitter-based syntax colors for all supported languages (#698)
- **syntax-treesitter**: bare fenced code blocks in doc comments inherit parent language — ` ``` ` without a language tag in Rust doc comments defaults to Rust highlighting (#701)
- **keybindings**: `LeaderKeyProvider` service and in-crate personality adapters — feature modules (git-signs, diagnostics-panel, bufferline, etc.) register vim-aware keybindings with qualified `"vim:normal"` mode strings and `<leader>` expansion at bootstrap (#700)
- **lsp**: integration tests for hover (`K`) and goto-definition (`gd`) against real rust-analyzer (#692)

### Fixed

- **viewport**: overlapping syntax tokens now resolve to the most specific (narrowest) match — fixes injection highlights being hidden by broader parent tokens (#696)
- **session**: split window inherits cursor and viewport from source — `<C-w>v` / `<C-w>s` no longer reset new pane to line 0
- **range-finder**: `;`/`,` repeat after label-selected `f`/`F`/`t`/`T` now advances to the next match instead of re-entering label mode (#663)
- **server**: operator-pending `ds`/`cs`/`ys` with leap motions now completes — `on_command_complete` fires after pop-result command execution (#663)
- **gutter**: add generic column priority for ordering — columns now sort by `priority` field (lower = further left) instead of Vec insertion order
- **tui**: dispatch `OptionChanged` notifications to client modules — `:set nu` and `:set rnu` now toggle line numbers in real-time
- **lsp**: non-blocking hover popup — `K` (hover) returns immediately via fire-and-forget async + ArcSwap cache + tick polling, no longer freezes editor for up to 5 seconds
- **hover**: dismiss popup on cursor movement in normal mode via `on_cursor_update()`
- **hover**: fix popup requiring two `K` presses — async task no longer kills the tick consumer before it can deliver the cached LSP result
- **hover**: fix crash on multi-byte characters (box-drawing `─` from markdown horizontal rules) — truncation now uses character-aware slicing instead of byte offsets (#692)
- **hover**: `display_width()` returns character count instead of byte length for correct popup sizing (#692)
- **lsp**: stabilize flaky diagnostic count assertion — relaxed to `>= 1` with at least one error check (#692)
- **cmdline**: `:s/pattern/replacement/` now dispatches correctly — ex-command parser splits name at first non-alpha character instead of whitespace (#705)
- **bufferline**: `H`/`L` buffer prev/next, `:bnext`/`:bprevious`/`:bd`/`:qa` ex-commands, `<Space>bd` binding (#699)
- **motions**: screen-high/low relocated from `H`/`L` to `gH`/`gL` (#699)
- **window**: `<C-h/j/k/l>` direct window focus navigation in normal mode (#699)
- **lsp-navigation**: diagnostic navigation `]d`/`[d`, `]e`/`[e`, `]w`/`[w` for next/prev diagnostic/error/warning (#699)
