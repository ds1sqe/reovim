# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.14.5-dev] - Unreleased

### Added

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

- **driver**: `BufferCapabilities` bitflags in `reovim-driver-buffer` — `CONTENT_MATERIALIZABLE`, `SNAPSHOTTABLE`, `STREAMABLE`, `FILE_BACKED`, `LINE_READABLE`, `EDITABLE` with `ROPE` and `VIRTUAL` presets (#739)
- **server**: `BufferHandle` enum unifying Rope and virtual buffer access at the server layer — gRPC handlers now see both buffer types (#739)
- **protocol**: `capabilities` field (u32 bitflags) on `BufferInfo` proto message (#739)
- **kernel**: `TextGeometry` trait — polymorphic line-based read access for both `Buffer` (zero-copy `Cow::Borrowed`) and `VirtualBuffer` (`Cow::Owned`) (#739)

### Changed

- **kernel**: `MotionEngine::calculate` and `TextObjectEngine::range` accept `&dyn TextGeometry` instead of `&Buffer` — motions and text objects now work on virtual buffers (#739)
- **session**: `with_text_geometry()` on `SessionRuntime` dispatches to either buffer type — 11 motion/textobject modules migrated (#739)

- **server**: `SessionState::buffer()` returns `BufferHandle` instead of `Arc<RwLock<Buffer>>` — virtual buffers are now visible to all 9 gRPC call sites (#739)
- **server**: `ListBuffers` includes both Rope and virtual buffers with capability flags (#739)
- **snapshot**: `Snapshot` now supports both Rope and `VirtualBuffer` via internal `SnapshotContent` enum — `capture_virtual()` and `restore_virtual()` methods added (#739)
- **commands**: `:e` command detects file size before loading — large UTF-8 files use mmap path, large binaries try streaming codec, small files use existing Rope path (#739)
- **completion**: virtual buffer completion uses line-based access instead of full content materialization (#739)
- **format**: format commands skip virtual buffers gracefully (#739)
- **session**: move `TextBufferRegistry` from `reovim-driver-session` into `reovim-provider-text`, with session importing it from the provider for boundary-correct ownership (#740)
- **server**: codec orchestration extracted from the `SwitchCodecView` gRPC handler into `CodecSessionState::switch_view` — handler is now a dispatcher that resolves the buffer, calls the codec-driver helper, translates structured errors to `tonic::Status`, and writes the decoded content to the buffer. No codec-side mutation remains in `server/lib/server/src/grpc/buffer.rs` (#740)
- **codec**: `ContentCodecFactory::create` now returns `Option<Arc<dyn ContentCodec>>` instead of `Option<Box<dyn ContentCodec>>`. `ContentCodecFactoryStore::find` inherits the Arc return type. All in-tree codec module factories (`codec-utf8`, `codec-hex`, `codec-cjk`, `codec-csv`, `codec-legacy`, `codec-pdf`, `codec-rlib`, `codec-binary-struct`) migrated. Callers drop the `Box -> Arc::from` conversion dance, and Phase 5 multi-mount can cheaply share a single codec across sibling mounts (#740)
- **codec**: single-mount invariant relaxed — `InodeTable::mount_additional` lets a second (or Nth) codec view attach to an inode that already has a mount. `InodeTable::apply_edit` now marks every peer mount's `content_valid = false` after an edit lands, so the Phase 5 sub-commit 5e `StaleCheck` hook can re-decode sibling views on next read. Added `InodeTable::flush(mount_id, path_override)` signature stub that returns `io::ErrorKind::Unsupported` until sub-commit 5d wires the consolidated `:w` path (#740)
- **commands**: `:w` consolidated onto `InodeTable::flush` — the command no longer calls `ContentCodec::encode`. For buffers with a codec mount the save path flushes canonical inode bytes directly via `ByteSource::write_to`; STREAMABLE buffers without a mount still stream via `buffer_write_to`; plain scratch buffers write UTF-8 content. The entire bugclass B1 (parallel raw-bytes cache drifts out of sync with the text buffer) is deleted by architecture (#740)
- **codec**: `Inode` gained an `Option<Arc<Path>>` path field; `InodeTable::insert_with_path`, `InodeTable::set_path`, and a real `InodeTable::flush(mount_id, path_override)` body land. Path resolution: override wins; else `inode.path`; else `io::ErrorKind::InvalidInput`. A successful flush with a path override populates `inode.path` for the `:w newfile.txt` scratch flow. The live 512 MB `MMAP_PROMOTION_BUDGET` guard in `apply_byte_edit` rejects edits on inodes that exceed the budget with a user-actionable `EditError::InvalidEdit` message (#740)
- **session**: new `StaleCheck` trait in `reovim-driver-session` with a `refresh_if_stale(&self, BufferId)` contract. `SessionShared` gained `install_stale_check` / `stale_check` / `clear_stale_check` accessors, and `SessionRuntime::buffer_content` now consults the installed hook (if any) before returning decoded text. The hook is a no-op when unset, which keeps tests and headless paths unaffected (#740)
- **codec**: new `InodeStaleCheck` adapter in `reovim-driver-codec` implementing `StaleCheck`. The server-side `SessionState::new` / `with_registries` constructors install the adapter at session creation so the codec → session crate boundary stays one-way. Current adapter body is a counter + `tracing::trace!` event emitter — the full re-decode-on-read path is blocked by the `&self` hook signature vs. `ExtensionMap` interior-mutability constraint and is deferred to a follow-up (#740)

### Refactored

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

- **session**: delete `ByteUndoRegistry` module — per-buffer byte-level undo now lives on `Inode` via `InodeTable`, keeping `reovim-driver-session` codec-agnostic (#740)
- **codec**: `ContentCodec::encode` trait method deleted workspace-wide. Bytes are the source of truth; `:w` flushes `inode.bytes` directly. Every in-tree codec (UTF-8, hex, CJK, CSV, legacy, rlib, PDF, binary-struct, xxd) has its `encode` impl removed. Faithful codecs (UTF-8, CJK, CSV, legacy) retain a private inherent `encode_fragment` helper for `translate_edit` re-encoding and round-trip tests. Test mocks and round-trip tests updated. The `encode_and_write` helper in `commands/write.rs` is also deleted (#740)

### Fixed

- **commands**: `:bd` / `:bdelete` now releases per-buffer state on close — previously `BdeleteCommand` bypassed `BufferApi::delete_buffer` and called `kernel.buffers.unregister()` directly, leaking `TextBufferRegistry` and `CodecSessionState` entries on every buffer close. Routing through `delete_buffer` also restores last-buffer protection (#740)
- **commands**: `decode_file_content` returns an error when no buffer is active instead of silently dropping codec metadata — a later `:w` then fell back to a UTF-8 encode that corrupted binary files (#740)

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
