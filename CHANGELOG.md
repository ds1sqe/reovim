# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.3-dev

### Documentation

- **Documentation drift fixes (#489)**: Fixed CLAUDE.md outdated counts (13->14 drivers,
  17->19 modules), added missing crate names (reovim-testing, reovim-log, etc.), added
  web client to clients list, added syntax-treesitter driver. Added "Plan Files" section
  with naming convention. Simplified Build Commands section with reference to new
  CLI Reference document. Fixed broken links in docs/architecture/overview.md (module-mode
  -> modules/mode-inheritance, kernel.md -> kernel/overview.md, etc.). Fixed server-mode.md
  to use gRPC instead of JSON-RPC. Fixed testing.md fabricated APIs (ServerTest ->
  IntegrationTest, with_content -> with_buffer, with_keys -> send_keys). Removed
  fabricated Visual Testing section with more accurate StepTest and frame assertions docs.

- **New module documentation (#490)**: Created 7 module docs in docs/architecture/modules/builtin/:
  buffer-ops.md (buffer lifecycle events), commands.md (ex-commands), defaults.md (meta-module
  aggregating 13 modules), mode-manager.md (mode transition tracker), options.md (editor settings),
  buffer-simple.md (SimpleBufferManager), scratch-buffer.md (empty buffer on startup).

- **New driver documentation (#490)**: Created 8 driver docs in docs/architecture/drivers/:
  buffer/overview.md (BufferManager registry), clipboard/overview.md (ClipboardProvider trait),
  command-types/overview.md (CommandContext, CommandResult shared types), ffi/overview.md
  (C FFI interface + ABI versioning), ffi-python/overview.md (Python bindings via PyO3),
  search/overview.md (SearchProvider trait), syntax-treesitter/overview.md (TreeSitterDriver),
  undo/overview.md (UndoProvider trait + persistence).

- **Debug infrastructure documentation**: Created docs/architecture/kernel/panic/overview.md
  documenting panic handling (install_panic_handler, recovery, crash reports) and
  docs/architecture/server/debug/overview.md documenting server ring buffer (64KB server +
  8KB per-client), CompositeLogger, CLI log-tail command, and gRPC DebugService.

- **CLI reference guide**: Created docs/user-guide/cli-reference.md with complete command
  reference for server, CLI client, and TUI client commands extracted from CLAUDE.md.

- **Updated index files**: Updated docs/architecture/modules/builtin/README.md with all 19
  modules. Updated docs/architecture/drivers/overview.md with all 14 drivers.

### Added

- **Server debug ring buffer with panic integration (#478)**: Added comprehensive
  debug infrastructure for server-side crash reporting and post-mortem analysis.
  Two-level ring buffer architecture: 64 KB server-wide buffer for kernel/module
  events, 8 KB per-client buffers for client-specific events (keys, commands,
  mode changes). `DebugRingBuffer` implements kernel `Logger` trait, capturing
  `pr_err!`/`pr_info!` calls. `CompositeLogger` forwards to both ring buffer and
  tracing. Panic handler integration via `DebugContext` callback dumps server logs
  to crash reports. Human-readable crash report timestamps
  (`crash-2026-02-04_12-34-56-{nanos}.txt`). Thread-safe with `parking_lot::RwLock`
  and non-blocking `try_dump()` for panic handler safety. String interning for
  memory efficiency. New files: `server/lib/server/src/debug/` (ring_buffer.rs,
  composite_logger.rs), `session/ring_buffer.rs`, `session/crash_dump.rs`.
  Modified: panic handler with `DebugContext`, crash report with `server_logs`
  and `client_dump_paths` fields. 55 tests across 5 modules.

- **Complete debug infrastructure (#481)**: Wired remaining debug features from
  #478. Client disconnect handler: `Session::remove_client()` now dumps client
  ring buffer to `~/.local/share/reovim/crash/client-{id}-{timestamp}.log` and
  logs `CLIENT_DISCONNECT` entry to server ring buffer before removal. CLI
  log-tail command: `reovim cli log-tail` queries server ring buffer via gRPC
  DebugService. Supports filters: `--count` (entries), `--level` (trace/debug/
  info/warn/error), `--target` (module filter), `--grep` (message search).
  Color-coded terminal output (red=ERROR, yellow=WARN, green=INFO, cyan=DEBUG,
  gray=TRACE). JSON output with `--format json`. New proto: `debug.proto` with
  `DebugService` (LogTail, LogLevel RPCs). New gRPC handler: `DebugServiceImpl`
  in `server/lib/server/src/grpc/debug.rs`. 3 new session tests, 6 new gRPC tests.

- **Multi-client presence rendering (#474)**: Implemented client-side rendering
  of remote clients' cursors and selections. CBF-8 colorblind-friendly palette
  in `shared/arch/src/palette.rs` provides 8 distinct colors (Wong 2011) with
  deterministic assignment via `color_for_client(client_id)`. TUI client renders
  remote cursors with `'▎'` character in per-client colors, remote selections
  with dimmed background overlays. Web client renders CSS-positioned cursor bars
  and selection backgrounds. New `Screen::overlay_bg()` method for non-destructive
  selection rendering. Supports char/line/block visual modes. 5 palette tests.
  Files: `shared/arch/src/palette.rs` (new), `clients/web/src/palette.ts` (new),
  `clients/tui/src/app_v2.rs`, `clients/tui/lib/drivers/tui/src/screen.rs`,
  `clients/web/src/editor.ts`.

### Changed

- **Session struct refactoring with BootstrapState pattern (#488)**: Simplified driver-level
  `Session::new()` signature and introduced `BootstrapState` for test initialization.
  `Session::new(id)` now takes only `ClientId` (no mode parameter). Added
  `Session::bootstrap(id, mode)` returning `(Session, BootstrapState)` tuple for tests
  needing initial per-client state. `BootstrapState` contains `mode_stack`, `windows`,
  `pending_keys`, `extensions` for initialization. Updated ~60 test callsites across
  20 files. The deprecated fields remain in `Session` for now (production code still
  uses them), with full removal deferred to a follow-up issue. This refactoring makes
  the per-client state architecture from #471/#477 explicit in the type system.

- **Add home_mode to SessionShared (#491)**: Added `home_mode: ModeId` field to
  `SessionShared` for proper session-level storage of the home mode used to initialize
  new clients. `Session::new(id, home_mode)` now takes home_mode parameter and stores
  it in `SessionShared`. Added `SessionShared::home_mode()` accessor. Fixed WindowApi
  implementation to use per-client `self.windows` instead of deprecated `self.session.windows`.
  Deprecated `SessionContext` in favor of `SessionRuntime` which properly supports
  per-client state isolation. Updated 50+ test callsites across server and modules.
  This completes the architectural separation: session-level config (home_mode) lives
  in `SessionShared`, per-client state lives in `EditingState`.

- **Remove deprecated Session fields (#491 Phase 7)**: Completed the per-client state
  migration by removing deprecated fields from driver-level `Session` struct. Removed
  `windows`, `mode_stack`, `pending_keys`, `extensions` fields - these now live in
  server-level `EditingState` per-client. Removed `Session::current_mode()` method.
  Deleted `SessionContext` (deprecated type with zero production usage). Removed
  delegation methods from `SessionState` (`mode_stack()`, `extensions()`, etc.).
  Updated notification_builder to use per-client windows for buffer_id lookup.
  Updated all callers to use `home_mode()` or per-client `EditingState`. After this
  change, `Session` contains only `id: ClientId` and `shared: SessionShared` - truly
  minimal shared infrastructure. ~20 files modified, 366+ tests passing.

- **Per-Client State Architecture Consolidation (Phase 0)**: Complete per-client
  cursor isolation by consolidating `SessionRuntime` constructors and removing
  deprecated compatibility shims. `SessionRuntime::new()` now requires 6 arguments
  with per-client state REQUIRED (mode_stack, windows, extensions). Added
  `SessionShared` type for truly shared session infrastructure. Removed kernel-level
  cursor concept from `Buffer` (cursor now lives in `Window`). 100% command migration
  complete: editor (cursor, delete, display_line, insert_edit, paste, replace, yank),
  motions (line, search, word), textobjects (bracket, paragraph, quote, word), vim
  (change, find_char, mode, visual/*). Removed deprecated methods: `effective_windows()`,
  `scroll_offset()`, `window_active_buffer()`, `CommandRegistry::execute()`,
  `SessionState::execute_command()`. Architecture: SessionRuntime always operates
  on per-client state, enabling true multi-client isolation. Parts of #465.

- **Remove Silent Fallbacks - Fail Loud Policy (#479, #484, #485, #486)**: Eliminated
  all silent fallback patterns in gRPC handlers. Server now returns explicit errors for
  invalid `client_id` instead of silently falling back to shared state. Clients panic
  on failure (misconfigured clients crash rather than silently using wrong state).

  **Phase 1-5 (#479)**: Removed `if req.client_id > 0` guards from all state handlers
  (`get_mode`, `get_cursor`, `get_layout`, `get_visible_lines`, `get_selection`). Replaced
  `unwrap_or_else()` fallbacks in input.rs with `.expect()` for invariant violations.
  Added `*_or_panic()` methods to TUI client. CLI now auto-joins via `ensure_joined()`.

  **Phase 6 (#484)**: Replaced `let _ = execute_command` patterns with proper error
  handling. Command failures now log to per-client ring buffer and emit warnings.

  **Phase 7-8 (#485)**: Eliminated ID sentinel value ambiguity (`map_or(0, ...)`). Made
  proto fields optional: `LayoutChangedPayload.focused_window_id`, `WindowInfo.buffer_id`,
  `WindowLeaf.buffer_id`, `ClientPresence.buffer_id`. Updated all handlers to use
  `Option<u64>` instead of 0 as "no value" sentinel. Timestamps now use `.expect()`
  instead of `map_or(0, ...)`.

  **Phase 9 (#486)**: Fixed notification_builder.rs to use optional types for buffer_id.
  All timestamp patterns use `.expect("system time before UNIX_EPOCH")`.

  Files: `server/lib/server/src/grpc/*.rs`, `shared/protocol/proto/reovim/v2/*.proto`,
  `clients/tui/src/*.rs`, `clients/cli/src/*.rs`.

- **Per-client module state (extensions) isolation (#477)**: Added per-client `ExtensionMap`
  to `EditingState` for complete isolation of vim state between clients. Previously, ALL
  module state lived in shared `Session.extensions`, causing bugs where Client A pressing
  `5` (sets `pending_count=5`) would cause Client B's `j` motion to move 5 lines instead
  of 1. Solution: Extended the per-client architecture from #471 (mode_stack/windows) to
  include extensions. `SessionRuntime` now has `client_extensions: Option<&mut ExtensionMap>`
  field. `ExtensionApi` implementation checks per-client extensions first via `ext()` and
  `ext_mut()`, with automatic fallback to shared `session.extensions` for buffer-scoped
  state like `SyntaxSessionState`. `EditingState` has manual `Clone` impl that creates
  fresh `ExtensionMap` (intentional for follower clients). All 48 call sites using
  `ext::<VimSessionState>()` automatically use per-client storage with zero changes.
  Future modules (range-finder, popups, telescope) will automatically get per-client
  isolation. Files: `server/lib/server/src/session/client.rs` (extensions field, manual
  Clone), `server/lib/drivers/session/src/runtime.rs` (client_extensions, ExtensionApi),
  `server/lib/server/src/session/state.rs` (2 call sites), `server/lib/server/src/registry/command.rs`
  (1 call site). Part of Epic #465. (#477)

- **Generic Input Target System (#482)**: Refactored character input routing to eliminate
  string-based mode detection (`mode_name.contains("command")`). New `InputTarget` enum
  specifies where characters go: `Buffer` (default) or `Extension(TypeId)`. Resolvers now
  use helper methods: `ResolveResult::insert_char(c)` for buffer insertion,
  `ResolveResult::insert_char_to::<CmdlineState>(c)` for extension routing. Added
  `TextInputSink` trait for extensions that accept text input, `SessionExtensionDyn` trait
  for object-safe runtime access, `ExtensionMap::get_text_input_sink_by_id()` for TypeId-
  based lookup. `CmdlineState` now implements `TextInputSink` with `as_text_input_sink()`
  override. Runner routes via `insert_char_by_target()` matching on target instead of mode
  name. Enables any module to define input targets without runner changes (e.g., hypothetical
  chess/tetris/search modules). Files: `server/lib/drivers/input/src/resolver.rs`,
  `server/lib/drivers/session/src/extension.rs`, `server/lib/drivers/session/src/api/cmdline.rs`,
  `server/lib/server/src/grpc/input.rs`, `server/modules/vim/src/resolvers/*.rs`.

- **Client Architecture Unification**: Unified three inconsistent client models
  into a single `ClientRelation`-based architecture. Server layer: All clients
  now have `EditingState` (not optional), `ClientRelation` enum with `Following`
  and `Sharing` variants replaces scattered role/sync logic. State machine with
  `TransitionResult` enum handles validation (cycle detection, self-targeting,
  target existence). Protocol layer: New `SetRelation` RPC, `ClientRelationType`
  enum, `ClientInfo`/`ClientViewState`/`ClientMetadata` messages, `TransitionError`
  enum. Backward compatible via `peers_v2`/`clients_v2` fields alongside deprecated
  legacy fields. Wire layer: New unified types in `shared/clients/model/src/wire/client.rs`
  with WASM/TypeScript support. (#480)

### Changed

- **Directory restructure**: Major codebase restructure to clarify
  server/client boundaries. New directory layout: `server/` (kernel, drivers,
  modules), `clients/` (tui, cli), `shared/` (protocol, arch, net, log, trace,
  module-macros, testing), `apps/` (thin runner binary). Server-side drivers
  (14 total) moved to `server/lib/drivers/`. Server-side modules (17 total)
  moved to `server/modules/`. Client-side modules (7 total: layout, pair,
  cmdline, statusline, which-key, undotree, example) archived to
  `archive/post_kernel/modules/` - will be reimplemented as client-side plugins.
  TUI drivers (display, tui) moved to `clients/tui/lib/drivers/`. Window
  command IDs moved from archived `layout` module to `window-ops` module
  (server-side). Updated all workspace paths and dependencies. Part of Epic #465. (#465)

- **Legacy cleanup**: Removed legacy crates from workspace as part of
  aggressive v2 migration cleanup. `runner/` (legacy binary with v1 JSON-RPC server)
  and `lib/clients/core/` (v1 JSON-RPC client library) removed from workspace -
  both archived to `archive/post_kernel/`. TUI client (`lib/clients/tui/`) now
  uses only gRPC v2 (`TuiAppV2`). Module tests (vim, undo, search, which-key)
  archived - to be rewritten for v2 testing infrastructure (`lib/testing/`).
  TUI simplified: removed deprecated v1 args (`--tcp`, `--socket-path`, `--instance`),
  now uses only `--grpc` for gRPC v2 connections. Part of Epic #465. (#465)

### Fixed

- **Visual selection not visible in TUI (Phase 17)**: Fixed bug where entering
  visual mode (`v`) showed no highlighting. Root cause: `CommandRegistry::execute()`
  created `SessionRuntime` for command execution but never called `take_changes()`,
  so selection changes from `EnterVisualMode::execute()` were dropped when runtime
  went out of scope. Fix: Changed `execute()` return type from `Option<CommandResult>`
  to `Option<(CommandResult, StateChanges)>` and propagate changes to callers.
  TUI enhancements: Added `SelectionState` struct for tracking per-window selections,
  `window_selections: HashMap<u64, SelectionState>` in `TuiState`, `SelectionChanged`
  notification handler, magenta background highlighting in `render_line_with_syntax()`.
  Also added `CursorPosition` struct and `window_cursors` map for per-window cursor
  tracking. Selection now uses exclusive end semantics (like Rust ranges) - to include
  character at cursor, `end = cursor + 1`. Part of Epic #465. (#465)

- **Visual mode resolver missing**: Fixed bug where Visual mode keys
  (h, j, k, l, w, b, e, etc.) were not working - either inserting characters or
  being ignored. Root cause: No resolver registered for Visual modes, causing
  `ResolverRegistry.get()` to return `None` and bypass mode inheritance. Solution:
  Created `VimVisualResolver` (~350 LOC) that handles escape, count accumulation,
  and delegates motion keys to Normal mode keymap. Registered for all 3 visual
  variants (`VISUAL_ID`, `VISUAL_LINE_ID`, `VISUAL_BLOCK_ID`). Server now has 10
  resolvers (was 7). Part of Epic #465. (#465)

### Added

- **Web Client Capture Test Infrastructure (Phase 16)**: Headless web client and
  capture relay for programmatic testing. CaptureHandler (`src/capture/handler.ts`):
  handles `captureRequest` notifications from server, formats state as text matching
  TUI capture format, enables `reovim cli capture` to work with web client. Shared
  by browser Editor and Node.js HeadlessWebClient (mechanism vs policy separation).
  HeadlessWebClient (`src/headless/client.ts`): Node.js-only client using native
  gRPC transport (`@connectrpc/connect-node`), maintains internal state without DOM,
  API mirrors TUI headless: `connect()`, `sendKeys()`, `capture()`, `waitFor()`,
  `getMode()`, `getCursor()`, `getBuffer()`. Test infrastructure: WebTestServerHarness
  spawns reovim server on dynamic port with auto-cleanup, WebIntegrationTest provides
  fluent builder for tests, frameAssertions helpers extract mode/cursor from captures.
  Integration tests (`tests/integration/headless.test.ts`): 17 tests (11 passing,
  6 skipped pending module loading) covering connection, state, capture, and frame
  assertions. Vitest config updated with 30s test timeout for server startup.
  Dependencies: `@connectrpc/connect-node@^1.7.0`, `@types/node@^22.0.0`.
  Part of Epic #465. (#465)

- **Automated E2E Testing Infrastructure (Phase 15)**: Comprehensive E2E testing
  framework for validating client-server interactions. TUI headless testing
  (`clients/tui/tests/headless_tests.rs`): 6 tests using `TuiAppV2Headless` with
  `connect_with_size()` for controlled terminal dimensions, `capture(ScreenFormat)`
  for frame assertions, `wait_for()` with timeout and predicate for async verification,
  `send_keys()` for input simulation. Frame assertion helpers (`shared/testing/src/frame.rs`):
  `assert_frame_contains()`, `assert_frame_not_contains()`, `assert_frame_line_contains()`,
  `assert_statusline_mode()`, `assert_frame_line_count()`, `assert_frame_min_width()`,
  plus non-panicking variants `frame_contains()`, `frame_line_contains()`, `get_statusline()`,
  `get_line()`. 12 unit tests for frame helpers. `IntegrationTest` harness uses
  `TestServerHarness` with OS-assigned gRPC ports and auto-cleanup. vim_commands.rs
  tests (34 total) documented as blocked pending `:e` command implementation - harness
  requires `:e {path}<CR>` for buffer setup. Tests deferred: 2 TUI tests (module loading),
  3 streaming presence tests, 34 vim command tests (`:e` command). Part of Epic #465. (#465)

- **Multi-client presence awareness (Phase 14)**: Implemented `PresenceService`
  gRPC for collaborative editing scenarios. Protocol layer (`presence.proto`):
  6 RPCs (`Join`, `Leave`, `StreamPresence`, `UpdatePresence`, `SetSyncMode`,
  `ListClients`), `ClientPresence` message with client_id, client_type, display_name,
  cursor, visible_lines, mode, sync_mode fields, `SyncMode` enum (INDEPENDENT,
  FOLLOW, PRESENT), `PresenceUpdate` oneof for streaming (joined/updated/left).
  Session layer (`presence.rs`): `SyncMode` enum, `ClientPresence` struct with
  `joined_at_ms()` helper, `PresenceMap` thread-safe container with RwLock for
  join/leave/update/get/list/followers_of operations. `ClientId` generation via
  `AtomicUsize` counter in `SessionRegistry` (lock-free). Session holds
  `PresenceMap` for per-session client tracking. gRPC layer (`presence.rs`):
  `PresenceServiceImpl` with all 6 RPC implementations, direct notification
  emission (not via `StateChanges`) for presence events, `presence_joined`,
  `presence_left`, `presence_updated` notification payloads (slots 23-25).
  Disconnect handling: clean disconnect via `Leave()` RPC, stream-based cleanup
  documented for future enhancement. `StateChanges` extended with `presence_changed`
  flag and `presence_updates` vector for future cursor sync scenarios. Tests:
  32 unit tests (18 PresenceMap + 14 gRPC methods) covering join/leave lifecycle,
  sync mode transitions, follower tracking, thread safety, error paths. 13
  integration test stubs for future CLI client support. Part of Epic #465. (#465)

- **TUI Theme Engine (Phase 13.0)**: Complete theme engine for TUI client with
  TOML-based user themes and token-to-style integration. Theme file format:
  `~/.config/reovim/themes/*.toml` with palette section for color reuse, syntax/ui/
  diagnostic/gutter sections for style definitions. `ThemeLoader` with multi-path
  search (`~/.config/reovim/themes/`, `/usr/share/reovim/themes/`), theme caching,
  and platform-aware paths. `ThemeManager` with 4-tier lookup (overrides → theme →
  module defaults → fallback) and hierarchical fallback (`keyword.control` →
  `keyword` → default). `TokenCache` for client-side syntax token caching with
  byte-to-position conversion (handles UTF-8 correctly), multi-line token splitting,
  and LRU eviction (max 10 buffers). `TokenCacheManager` for per-buffer token
  management. Integration with `StreamTokens` RPC for real-time token updates.
  Runtime theme switching via `--theme` CLI argument and `colorscheme` option
  notification. 88 tests (76 style + 12 syntax) with 100% pass rate. Part of
  Epic #465. (#470)

- **Web Theme Engine (Phase 13.1)**: Complete theme engine for web client mirroring
  TUI Phase 13.0 design but adapted for browser environments. `ThemeManager` with
  same 4-tier lookup (overrides → theme → module defaults → fallback) and hierarchical
  fallback (`keyword.control` → `keyword` → default). `ThemeProvider` interface matches
  TUI trait. 42 highlight groups (13 syntax, 17 UI, 4 diagnostic, 8 gutter) defined as
  constants. `parseTheme()` resolves palette references to hex colors. Three built-in
  themes matching TUI: Dark (OneDark-inspired), Light (high-contrast), Tokyo Night
  Orange. `StyleGroupRegistry` for module-provided defaults. `applyToCSSVariables()`
  applies all 42 groups as CSS custom properties (`--theme-*`). Theme choice persisted
  to localStorage. `TokenCache` for web with byte-to-position conversion and LRU
  eviction (max 10 buffers). `TokenCacheManager` for per-buffer token management.
  `BufferRenderer` integration for syntax-highlighted rendering via `setThemeManager()`
  and `setTokenCache()`. CSS files (`main.css`, `layout.css`, `overlay.css`) migrated
  from hardcoded colors to theme variables. 102 tests (53 theme + 49 syntax) with
  100% pass rate. Part of Epic #465. (#465)

- **Common Client Model**: Created `reovim-client-model` crate at
  `shared/clients/model/` - a platform-agnostic abstraction layer for all clients
  (TUI, Web, Android). Separates wire format types (from server) from rendered state
  (client-side interpretation). Foundation types (`ScreenPosition`, `Size`, `Rect`,
  `Direction`, `SplitDirection`). Wire format types (`Anchor` enum for positioning,
  `LogicalOverlay`, `OverlayState`, `LogicalLayout` tree, `ViewportState`,
  `ClientPresence`, `SyncMode`). Rendered state types (`RenderedOverlay`, `OverlayStack`
  with z-order, `Window`, `WindowTree` traversal, `PanelState`). Interaction types
  (`Interaction` enum, `InteractionResult` for overlay input handling). Core traits
  (`Panel`, `Layout`, `OverlayRenderer` with object-safety for `Box<dyn>`,
  `OverlayManager`, `FocusManager`, `LayoutInterpreter`). Sync types (`LayoutSyncMode`,
  `OverlaySyncMode`, `PresenceTracker`). Total: 147 unit tests with comprehensive
  coverage. No I/O dependencies - pure types and traits. Part of Epic #465. (#465)

- **TUI Adapter for Common Client Model**: Integrated `reovim-client-model` with
  TUI client using hybrid approach (common model for data interchange, TUI keeps
  its compositor). Added `clients/tui/src/adapter/` module (6 files, ~1200 LOC) with:
  `TuiLayoutAdapter` (wraps `RootCompositor`, implements `Layout` trait with split/close/focus
  operations), `TuiPanel` (wraps `View`, implements `Panel` trait with scroll/cursor/visible-range),
  `TuiOverlayManager` (implements `OverlayManager` trait with renderer registry, ID mapping,
  position resolution), `TuiFocusManager` (implements `FocusManager` trait with panel/overlay
  focus transitions), `AnchorContext` and `convert_anchor()` (wire anchor to TUI anchor conversion
  with normalized-to-absolute coordinate mapping, cursor context, overlay ID resolution).
  Type conversions: `WindowId`/`BufferId` (usize) to u64 and back, `Direction`/`SplitDirection`
  to TUI equivalents, `wire::Anchor` variants to `layout::Anchor`. `TuiAdapterFactory` for
  convenient adapter creation from compositor. Added `BufferId` re-export from display driver.
  77 adapter-specific tests (153 total TUI tests). Part of Epic #465. (#465)

- **WASM-based multi-window support for web client**: Integrated `reovim-client-model`
  with the web client using a hybrid WASM approach - Rust core types compiled to WASM
  with auto-generated TypeScript types via `tsify-next`, and web-specific rendering in
  TypeScript. Rust changes: Added `serde` derives to all 25+ types across 14 files in
  `shared/clients/model/`. Added WASM feature with `wasm-bindgen` and `tsify-next`
  dependencies. Created `wasm.rs` (~140 LOC) exporting layout interpreter, geometry
  helpers, and direction utilities. WASM binary: 87KB. Web changes: Added WASM bridge
  layer (`src/wasm/`) with `bindings.ts` (WASM init + 12 exported functions),
  `helpers.ts` (9 tree traversal utilities + type guards), `convert.ts` (proto-to-model
  conversion handling discriminated unions). Added rendering layer (`src/render/`)
  with `LayoutRenderer` class for WindowTree-to-DOM conversion and `BufferRenderer`
  for line/selection/cursor rendering. Refactored `editor.ts` for dual-mode operation:
  multi-window (WASM) or legacy single-window with graceful fallback. Added
  `layout.css` (227 LOC) with styles for splits, tabs, windows, separators. Added
  `vitest.config.ts` with JSDOM for DOM tests. Test suite: 109 TypeScript tests
  (25 wasm-helpers, 27 wasm-convert, 17 render-layout, 40 existing keymapper) +
  148 Rust tests. Build pipeline: `npm run build:wasm` triggers wasm-pack before
  Vite build. Part of Epic #465. (#465)

- **Server-managed viewports (Phase 11)**: Implemented server-authoritative
  window layout with full command support. Server changes: Implemented
  `GetLayout` RPC (was returning unimplemented) - returns window tree with
  `WindowNode` (leaf/split) structure, focused window ID, and per-window
  viewport dimensions. Implemented `GetVisibleLines` RPC - returns visible
  line range (`first_line`, `last_line`, `viewport_height`) for viewport
  scrolling support. Window-ops module: Added 19 command handlers implementing
  `CommandHandler` trait: focus navigation (h/j/k/l, 4 commands), focus cycling
  (w/W, 2 commands), splitting (s/v/n, 3 commands), closing (c/o, 2 commands),
  resizing (+/-/>/<=/5 commands), float zone toggle/raise/lower (3 commands).
  Commands delegate to `CompositorApi` methods on `SessionRuntime`. TUI already
  integrated: receives `LayoutChanged` notifications via streaming, updates
  `state.windows` and `focused_window_id`, triggers redraw. End-to-end flow:
  `<C-w>v` → server executes `SplitVertical` → `CompositorApi::split()` →
  `record_window_created()` → `layout_changed` notification → TUI updates.
  Part of Epic #465. (#465)

- **Web client ViewportService integration (Phase 11.1)**: Extended web client
  to use server-managed viewports with incremental updates and overlay support.
  Cache layer (`clients/web/src/cache/`): `ViewportCache` class stores viewport
  state (scroll position, cursor) and applies incremental `ViewportUpdate`s from
  server notifications. `BufferCache` class stores buffer content with version
  tracking, reduces redundant RPC calls by caching visible lines. Notification
  handlers: Added `viewportUpdated` handler that applies incremental updates to
  cached state without RPC calls. Updated `bufferModified` to invalidate cache
  only for affected buffer. Updated `layoutChanged` to use `refreshLayoutOnly()`
  which fetches layout tree but uses cached buffer content for existing windows.
  Overlay rendering (`clients/web/src/render/overlay.ts`): Created `OverlayRenderer`
  class (~530 LOC) for floating UI elements. Supports 4 overlay types: completion
  (listbox with items, selection), cmdline (command-line input with cursor),
  hover (tooltip), signature (function help). Handles 5 anchor types: Cursor,
  Center, Buffer position, Screen (normalized), Below (relative to overlay).
  Proper ARIA roles for accessibility. CSS (`styles/overlay.css`, ~170 LOC):
  Catppuccin-inspired dark theme with glass-morphism effects. Server changes:
  Added `ViewportUpdatedPayload` to notification.proto with optional fields
  (viewport_id, top_line, left_col, cursor_line, cursor_col). Added
  `build_viewport_notification()` in notification_builder.rs (~20 LOC). Emits
  viewport updates when `scroll_changed` flag set in `StateChanges`. Performance:
  ~70-90% reduction in RPC calls during scrolling/cursor movement by using
  cached state. Part of Epic #465. (#465)

- **TUI ViewportService integration (Phase 11.2)**: Integrated TUI client with
  server-managed viewports using passive `ServerLayoutMirror` approach. Created
  `clients/tui/src/layout_mirror.rs` (~150 LOC) with `ServerLayoutMirror` struct
  that passively mirrors server layout state - simpler than implementing full
  `RootCompositor` (20+ methods). `WindowPlacement` type stores window bounds
  (x, y, width, height) and focused state. Key methods: `apply_layout_changed()`
  replaces all placements from `layout_changed` notification, `set_screen()` for
  resize, `placements()` for rendering, `has_multiple_windows()` for separator
  logic. Integrated into `TuiAppV2`: (1) Mirror field initialized with screen
  dimensions in `connect()`, (2) `apply_layout()` updates mirror from initial
  `GetLayout` RPC, (3) `handle_notification()` updates mirror on `LayoutChanged`
  payload, (4) `render_windows()` uses `layout_mirror.placements()` instead of
  `state.windows`, (5) Resize events call `layout_mirror.set_screen()`. Focus
  indicator uses `layout_mirror.focused_id()` and `has_multiple_windows()`.
  6 unit tests covering single/multi window, focus tracking, resize, and filtered
  windows. Follows same pattern as web client's `ViewportCache` (Phase 11.1).
  Part of Epic #465. (#465)

- **Per-client state architecture (Phase 11.2 extension)**: Extended session model
  to support per-client editing state for collaborative editing scenarios. Client
  roles (`session/client.rs`, ~300 LOC): `Client` enum with Owner/Follow/Share
  variants - Owner has own `EditingState` (mode_stack, pending_keys, cursor,
  viewport, selection), Follow is read-only spectator (input ignored), Share enables
  bidirectional co-editing with owner. `effective_state()` resolves state through
  chains with depth limit for cycle protection. Session changes (`session.rs`):
  Added `clients: RwLock<HashMap<ClientId, Client>>` for per-client tracking.
  Methods: `add_client()` (defaults to Owner), `remove_client()`, `get_client()`,
  `set_client_role()`, `client_state()`, `update_client_state()` (routes input
  based on role), `with_clients()`/`with_clients_mut()` for direct access.
  Protocol changes (`input.proto`, `presence.proto`): Added `client_id` to
  `SendKeysRequest` for per-client input routing, added `ClientRole` enum
  (OWNER/FOLLOW/SHARE) and `SetRole` RPC with `SetRoleRequest`/`SetRoleResponse`.
  Input routing (`grpc/input.rs`): Extracts `client_id` from request, auto-creates
  client as Owner if not exists, ignores input for Follow clients, normal processing
  for Owner/Share. Client-side empty layout handling: TUI (`app_v2.rs`) creates
  default window when server returns empty layout (via `create_default_window()`),
  Web (`editor.ts`) uses `createDefaultView()` for same scenario - both fetch active
  buffer and render single-window fallback. Foundation for spectator mode (Follow)
  and pair programming (Share). 28 unit tests (14 Client + 14 gRPC). Part of Epic
  #465. (#465)

- **Web client unit tests (Phase 11.3)**: Added comprehensive unit tests for
  Phase 11.1 cache and overlay components. Test files: `cache-viewport.test.ts`
  (25 tests) covers ViewportCache get/set, applyUpdate partial updates, delete,
  clear, iteration methods. `cache-buffer.test.ts` (40 tests) covers BufferCache
  version tracking, needsRefresh staleness detection, invalidation, getVisibleLines
  slicing with bounds clamping, getLine/getLineCount helpers, getStats metrics.
  `render-overlay.test.ts` (62 tests, jsdom) covers OverlayRenderer show/hide/remove,
  all 4 overlay types (completion items/selection, cmdline prefix/cursor, hover text,
  signature with docs), all 5 anchor types (Cursor, Center, Buffer, Screen, Below),
  ARIA accessibility roles (listbox/textbox/tooltip/dialog), updateState selection
  changes, custom config (charWidth, lineHeight, padding). Total: 127 new tests,
  bringing web client to 236 tests (was 109). All tests pass in 1.7s. Addresses
  Telemetry B grade from Phase 11.1 landing review. Part of Epic #465. (#465)

- **SyntaxService Token Data API (Phase 12)**: Created gRPC SyntaxService to expose
  syntax tokens for client-side syntax highlighting. Proto definition (`syntax.proto`,
  ~90 LOC): `GetTokens` RPC returns tokens for buffer range with language detection,
  `StreamTokens` RPC for real-time token streaming (stub for now), `GetLanguageInfo`
  RPC returns language metadata (id, name, extensions, parser availability).
  `TokenSpan` message uses byte offsets and category strings for theme flexibility.
  Service implementation (`grpc/syntax.rs`, ~500 LOC): `SyntaxServiceImpl` with
  session registry pattern matching other services. `highlight_group_to_category()`
  function maps all 47 `HighlightGroup` variants to TextMate-style category strings
  (keyword, function.builtin, string.escape, etc.) using existing `SyntaxHighlight`
  trait. Language detection from file extensions (~50 languages including Rust, Python,
  TypeScript, Go, etc.). 5 unit tests covering category mapping, language detection,
  extension lookups, and error cases. Server changes: Added `reovim-driver-syntax`
  dependency. Registered `SyntaxService` in gRPC router. Philosophy: Server provides
  token categories (mechanism), client applies colors via theme (policy). Foundation
  for Phase 13 (TUI Theme Engine). Part of Epic #465. (#465)

- **Syntax Driver Integration (Phase 12.1)**: Wired tree-sitter drivers into server
  so `GetTokens` returns real syntax tokens. Architecture follows self-registration
  pattern: modules register factories during `init()`, bootstrap extracts via
  `ServiceRegistry`. Three-layer structure: `reovim-driver-syntax` (traits, ~100 LOC
  `SyntaxFactoryStore`), `reovim-driver-syntax-treesitter` (~900 LOC, generic
  `TreeSitterDriver` with thread-safe parsing, `CaptureMapper` with 120+ mappings),
  `reovim-module-treesitter-rust` (~350 LOC, Rust grammar + highlights.scm query).
  Server integration: `SyntaxSessionState` stores per-buffer drivers, updated
  `GetTokens` to use `ensure_driver()` and `driver.highlights()`, `GetLanguageInfo`
  reports parser availability via factory. Bootstrap: `configure_syntax_highlighting()`
  extracts factory from `SyntaxFactoryStore` without importing language modules
  (kernel purity). 29 tests across all layers, 95%+ coverage. Architecture verified:
  zero tree-sitter deps in driver crate, mechanism/policy separation maintained,
  matches Linux kernel VFS pattern. Part of Epic #465. (#465)

- **Injection Highlighting and Fold Detection (Phase 12.3)**: Completed tree-sitter
  syntax infrastructure with injection highlight merging and fold detection.
  `InjectionManager` integration: Added `injection_manager` field to `TreeSitterDriver`,
  initialized when `injections_query` is provided via `with_queries()`. Wired into
  `highlights()` method with proper lock ordering (parent highlights first, then
  injections) to avoid deadlocks. `InjectionLayerFactory` trait (~30 LOC): Defined
  in driver crate (not traits crate) to keep tree-sitter types isolated from trait
  boundaries. Enables language modules to register injection layer factories for
  embedded language highlighting. `InjectionLayerStore` (~80 LOC): Global registry
  following `SyntaxFactoryStore` pattern, implements `Service` trait for `ServiceRegistry`
  compatibility. Fold detection: Implemented `folds()` method with tree-sitter query
  support. Created `folds.scm` for Rust (~55 LOC) covering functions, closures,
  structs, enums, impl blocks, traits, modules, match expressions, loops, if blocks,
  block comments, and macros. `node_to_fold_kind()` maps 14+ node types to `FoldKind`
  variants. `RustSyntaxFactory` updates: Added folds query, implemented
  `InjectionLayerFactory`, updated `init()` to register with both `SyntaxFactoryStore`
  and `InjectionLayerStore`. 51 tests across driver (28) and module (23) layers.
  All tests pass, zero warnings. Part of Epic #465. (#465)

- **Doc Comment Injection and Indentation Hints (Phase 12.4)**: Completed syntax
  service infrastructure with Rust doc comment injection and AST-based indentation.
  Doc comment injection: Created `injections.scm` (~34 LOC) for Rust doc comments
  (`///`, `//!`, `/** */`, `/*! */`) that injects Markdown highlighting. Implemented
  `InjectionLayerFactory` trait for `MarkdownSyntaxFactory` and registered with
  `InjectionLayerStore` during module init. Doc comments now display Markdown syntax
  highlighting (headings, code blocks, links). Indentation hints: Created `indents.scm`
  (~60 LOC) with 21 indent-increasing constructs (functions, closures, structs, enums,
  impl blocks, match expressions, loops, blocks, etc.) following nvim-treesitter
  conventions (@indent, @indent_end, @branch, @ignore). Implemented `indent_for(line)`
  method in `TreeSitterDriver` that walks up the AST tree counting @indent captures,
  returns suggested indentation in spaces (4 per nesting level). Minor cleanups:
  Removed outdated `#[allow(dead_code)]` from `highlight_group_to_category()` (now
  used by GetTokens/StreamTokens), updated TODO comment in injection.rs. 7 new tests
  for injection detection and indentation verification. All 69 syntax tests pass.
  Part of Epic #465. (#465)

- **Selection rendering for web client**: Implemented visual selection
  highlighting in the web client, validating Unix philosophy of server-mechanism /
  client-policy separation. Server changes: Implemented `GetSelection` RPC in
  `StateService` (was returning unimplemented) - extracts selection from buffer,
  normalizes start/end positions, maps `SelectionMode` to string ("char", "line",
  "block"). Added 6 unit tests covering no-selection, no-buffer, char/line/block
  modes, and reverse selections. Web client changes (`clients/web/`): Extended
  `EditorState` with selection fields (`hasSelection`, `selectionAnchor`,
  `selectionCursor`, `visualMode`). Added `selectionChanged` notification handler
  in `handleNotification()`. Created `isPositionSelected()` helper (~60 LOC)
  handling all three visual modes with forward/reverse selection normalization.
  Added `renderLineWithSelection()` for efficient DOM rendering using span batching
  (groups consecutive selected characters into single spans). CSS: Added
  `--selection-bg` variable and `.selected` class with semi-transparent purple
  highlight. Updated cursor styling for visual mode (border instead of solid
  background, no blink animation). Part of Epic #465. (#465)

- **Web client PoC with gRPC-Web support**: Added minimal web client that
  connects to reovim server via gRPC-Web, validating multi-platform architecture.
  Server changes: Added `grpc-web` feature to `reovim-server` with `tonic-web` middleware
  layer and CORS support (~44 LOC, feature-gated so existing gRPC unaffected). Web client
  (`clients/web/`): TypeScript + Connect-Web + Vite stack with generated TypeScript from
  existing proto files (15 generated files). Components include `keymapper.ts` (browser key
  to vim notation), `editor.ts` (state management and DOM rendering), `input.ts` (keyboard
  handler), `client.ts` (gRPC-Web transport). 40 keymapper tests (P0 critical, exceeds 25+
  requirement). Architecture decision: gRPC-Web over WebSocket - reuses existing proto
  (zero new protocol code), type-safe generated clients, same port for native gRPC and
  gRPC-Web. Production bundle: 22.53 KB gzip. Part of Epic #465. (#465)

- **Pytest infrastructure for Python testing module**: Added formal pytest
  suite for `tools/reovim-testing/` with 171 total tests (87 unit, 84 integration).
  Unit tests use mocking (no server required, 0.12s): `test_errors.py` (20 exception tests),
  `test_discovery.py` (19 binary/port tests), `test_client.py` (24 CLI wrapper tests),
  `test_capture.py` (24 dataclass tests). Integration tests require running server (~45s):
  `test_editor.py` (27 lifecycle tests), `test_cleanup.py` (6 zombie prevention tests),
  `test_battle.py` (51 battle-tested scenarios). Infrastructure includes `conftest.py`
  with module-scoped `binary_info` fixture, function-scoped `editor` with auto-cleanup,
  pytest markers (`integration`, `slow`, `binary`), and auto-skip when binary unavailable.
  Fixed `screen_capture_demo.py` imports. Added comprehensive documentation to `input.rs`
  fallback logic explaining when it triggers, what it does, and its limitations.
  Part of Epic #465. (#465)

- **Python testing module and capture relay**: Added comprehensive Python
  testing library at `tools/reovim-testing/` with zero-config `Editor` class featuring
  fluent API and context manager lifecycle. Includes `Capture` dataclass for rich state
  snapshots (frame, mode, cursor, buffer, registers), auto-discovery of binary and free
  ports, and 59 battle test scenarios. Implemented capture relay pattern (CLI → Server →
  TUI → Server → CLI) where server coordinates but has no screen (correct separation)
  and TUI owns viewport. Added resize relay from CLI to TUI via server notification.
  Fixed clippy pedantic issues (doc comments, `map_or`, `Error::other`). Module bootstrap
  now creates scratch buffer on startup. Part of Epic #465. (#465)

- **Server/client notification pipeline**: Completed the core server/client
  architecture with working command execution and notification emission. Pillars
  implemented: (1) `CommandExecutor` trait signature changed from `&mut KernelContext`
  to `&KernelContext` (interior mutability via `Arc<RwLock>` enables this), enabling
  `SessionRuntime::execute_command()` to work; (2) Created `notification_builder.rs`
  to convert `StateChanges` to gRPC notifications (mode, cursor, buffer, layout,
  selection, option changes); (3) Added `scroll_left` to `Viewport` for horizontal
  scroll tracking; (4) Created `TuiAppV2Headless` (775 LOC) for headless TUI testing
  with capture, resize, and event loop support; (5) Created E2E test infrastructure
  with `vim_commands.rs` (503 LOC) and `notifications.rs` (190 LOC). Macro recording
  (Pillar 6) deferred. Part of Epic #465. (#465)

- **NotificationService infrastructure**: Implemented gRPC v2
  NotificationService with server-to-client streaming for real-time notifications.
  Uses `tokio::sync::broadcast` channel pattern - Session holds broadcast sender,
  clients subscribe via `subscribe_notifications()`. `NotificationServiceImpl`
  wraps broadcast receiver in `async_stream::stream!` with event type filtering
  via `SubscribeRequest.event_types`. Created `TuiGrpcClient` module in
  `lib/clients/tui/` ready for future TUI migration (wraps Input, State, Buffer,
  Notification service clients with `subscribe()` method returning
  `Streaming<Notification>`). Added `grpc` feature to TUI Cargo.toml. This phase
  lands infrastructure only - full TUI migration deferred. Part of Epic #465. (#465)

- **gRPC v2 CLI client**: Created `lib/clients/cli/` crate
  (reovim-client-cli v0.9.3-dev) as the gRPC v2 command-line client. Deprecates
  JSON-RPC v1 for CLI usage. Implements four gRPC services in `lib/server/`:
  `InputServiceImpl` (SendKeys with vim notation parsing), `StateServiceImpl`
  (GetMode, GetCursor - other methods return unimplemented), `ServerServiceImpl`
  (Ping, Info with uptime/buffer count - Kill returns unimplemented). CLI crate
  contains `GrpcClient` wrapper over tonic clients, command implementations
  (keys, mode, cursor, buffers, buffer, ping, version), and clap-based argument
  parsing with `--grpc` address option and `--format` (plain/json) output.
  Basic character insertion supported; full vim key resolution deferred.
  Part of Epic #465. (#465)

- **TUI crate extraction**: Created `lib/clients/tui/` crate
  (reovim-client-tui v0.9.3-dev) as the standalone TUI client library. Contains
  terminal user interface with crossterm for rendering, async event loop with
  notifications, and embedded CLI panel. Includes 11 modules: `TuiApp` (main
  event loop, ~1800 LOC), `HeadlessClient` (CI/capture mode, ~630 LOC),
  `CliPanelState`/`CliExecutor`/`CliRender` (embedded CLI, ~1250 LOC combined),
  `LogBuffer`/`LogPanel`/`LogRender` (log panel, ~945 LOC combined),
  `InputHandler` (keyboard handling, ~224 LOC), `Renderer` (crossterm wrapper,
  ~186 LOC), `RenderState` (frame capture, ~335 LOC). Uses "copy" approach -
  runner keeps its own copy unchanged. All 87 unit tests pass. Part of Epic #465. (#465)

- **Client core extraction**: Created `lib/clients/core/` crate
  (reovim-client-core v0.9.3-dev) as the standalone client connection library.
  Provides TCP/Unix socket connection abstraction (`Connection`, `ConnectionConfig`),
  server discovery via port scanning (`list_servers`, `ServerInfo`), and JSON-RPC
  v1 client (`RpcClient`, `RpcWriter`). Used "copy" approach - runner keeps its
  own copy unchanged while new crate provides identical functionality. This enables
  future TUI and CLI extraction in Phases 6-7. All 18 unit tests pass. Part of
  Epic #465. (#465)

- **New thin runner**: Created `apps/reovim/` crate (reovim-app v0.9.3-dev)
  as the new architecture binary. This thin CLI wrapper uses `lib/server/` directly,
  demonstrating the server/client split. Binary named `reovim-new` for parallel
  installation during migration. Supports server mode with `--tcp`, `--grpc` (feature-
  gated), and `--socket` (Unix) transport options. Establishes `apps/` directory
  pattern for future applications (GUI, web). Part of Epic #465. (#465)

- **Server crate extraction**: Created new `lib/server/` crate
  (reovim-server v0.9.3-dev) as the foundation for server/client split. Implements
  session management with `SessionRegistry` (lock-free via `ArcSwap`), `Session`
  (named editing context), and `SessionState` (kernel wrapper). Server supports
  multiple transports: TCP with fallback, specific TCP port, Unix socket (Unix),
  and gRPC (feature-gated). Bridges to gRPC v2 protocol via `BufferServiceImpl`.
  New crate coexists with old runner - no breaking changes. Part of Epic #465. (#465)

- **gRPC v2 transport**: Added gRPC transport listener as parallel
  transport option alongside existing TCP/JSON-RPC. Server can start with
  `--grpc <PORT>` flag. Implemented `BufferService` gRPC service with methods:
  `GetRawContent` (raw buffer lines), `GetLineCount`, `GetAnnotations` (stub),
  `List` (all open buffers). Other methods (`OpenFile`, `WriteFile`, `SetContent`)
  return unimplemented status. The gRPC transport follows v2 protocol
  philosophy: server provides raw data, client renders. Part of Epic #465. (#465)

- **InstanceRegistry migration**: Moved `InstanceRegistry`, `InstanceInfo`, and
  `TransportInfo` from `runner/src/server/instance/` to `lib/protocol/src/instance/`.
  These are protocol-level abstractions used by both client and server. Fixed
  architectural violation where client code imported from `server::instance`.
  Part of Epic #465. (#465)

---

## [0.9.2] - v0.9.2-dev

### Added

- **Command-line UI**: Implemented cmdline state management and popup
  rendering for search (/, ?) and Ex command (:) modes. Characters route to
  cmdline buffer when active. Supports editing keys (Backspace, Delete, Left,
  Right, Home, End, Ctrl+A, Ctrl+E). Floating popup renders at screen top with
  box drawing characters showing prompt and input with block cursor. Popup
  appears on activation and hides on execution (Enter) or cancellation (Esc).
  Both interactive TUI and headless mode support cmdline rendering. (#451)
- **Line number display modes**: Added support for `:set number` and `:set
  relativenumber` options with absolute, relative, and hybrid display modes.
  Server-side rendering via `state/screen_content` with reactive notifications
  via `OPTION_CHANGED`. Ex commands `:set number`, `:set nonumber`, and toggle
  via `:set number!` are supported. (#445)
- **Option query RPC**: New `state/options` RPC method queries editor options
  with optional filtering by name and window scope. (#445)
- **TUI frame capture via RPC relay**: Added `reovim cli capture` command and
  `tui/capture` RPC method for capturing TUI frames with ANSI colors. Flow:
  CLI → Server → TUI → Server → CLI. Requires a connected TUI client. (#447)
- **Headless TUI mode**: Added `--headless` flag to TUI client for CI/scripting.
  Runs without TTY, responds to capture requests only. (#447)
- **Shared render core**: Extracted `RenderCore` and `RenderState` for shared
  rendering between interactive and headless TUI clients. (#447)
- **Window mode**: Added `<C-w>` prefix for window management commands. Supports
  navigation (h/j/k/l), splitting (s/v), closing (c/q/o), resizing (+/-/>/</=),
  and cycling (w/W/p). Each command executes and returns to normal mode. (#438)
- **Compositor infrastructure**: Layout module now self-registers 15 window
  commands via `CommandHandlerStore`. Runner no longer imports from policy
  modules, fixing architecture violation. (#438)
- **Floating window layer**: Implemented float zone within the nested compositor
  architecture. Windows can toggle between tiled and floating with `<C-w>f`.
  Float windows are rendered above tiled windows and can be reordered with
  `<C-w>]` (raise) and `<C-w>[` (lower). Navigation (h/j/k/l) and cycling (w/W)
  work seamlessly between tiled and float windows using spatial proximity. (#398)
- **Overlay/popup layer**: Implemented overlay zone within the nested compositor
  for temporary UI elements (popups, menus, tooltips, autocomplete). Overlays
  use anchor-based positioning: Cursor (below text cursor), Screen (absolute),
  Center (screen center), or Below (below a window). Supports up to 50 overlays
  per layer with automatic clamping to screen bounds. Overlays render above all
  tiled and float windows without stealing focus. Infrastructure only; consumer
  systems (autocomplete, hover, command palette) to follow. (#399)
- **Color and theme system**: Implemented centralized theme system with 3 built-in
  themes (dark, light, tokyo-night-orange). Themes define 41 highlight groups for
  syntax (keyword, function, string, comment, etc.), UI elements (statusline,
  line numbers, cursor, selection, borders), diagnostics (error, warning, info,
  hint), and rainbow brackets (6 colors + unmatched). Switch themes at runtime
  with `:colorscheme <name>`. Theme system follows mechanism/policy separation:
  driver layer provides `ThemeProvider` trait and `ThemeManager`, module layer
  provides `:colorscheme` command. Supports 16, 256, and true color terminals
  with automatic color mode detection. (#439)
- **Pair plugin**: Added bracket assistance module with three features: (1)
  Auto-close brackets - typing `(`, `[`, `{`, backtick, `'`, or `"` inserts the
  closing character with cursor positioned between. Symmetric pairs (quotes)
  use atomic tracking to prevent infinite recursion. (2) Rainbow bracket
  highlighting - nested brackets colored by depth using a 6-color palette that
  cycles. (3) Matched pair indicator - cursor on a bracket highlights its match
  with bold + underline. Module registers `SharedPairState` service and subscribes
  to `CursorMoved`, `BufferModified`, `BufferClosed` events. (#440)
- **Command query service**: Added `CommandQueryService` trait and `CommandInfo`
  struct for command discovery and completion. Enables modules to query commands
  by name prefix (`search_by_prefix`), exact name (`find_by_name`), or list all
  ex-commands (`list_ex_commands`). Registered in `ServiceRegistry` for module
  access. Foundation for cmdline tab-completion and help systems. (#453)
- **Cmdline UI module**: Created `modules/cmdline/` for command-line popup UI.
  Provides `SharedCmdlinePopupState` service for popup visibility and content
  tracking. UI rendering includes rounded border characters (╭╮╰╯), prompt
  display (:/?), input text with cursor indicator (█), and horizontal scrolling
  for long input. Input handler now intercepts cmdline editing keys (Backspace,
  Delete, Left, Right, Home, End, Ctrl+A/E/H) when cmdline is active, routing
  them to `CmdlineBuffer` methods. This is initial infrastructure; floating
  popup rendering in TUI to follow. (#451)
- **Extensible statusline system**: Implemented lualine-inspired statusline with
  sections (A-B-C | X-Y-Z), pluggable components, and mode-specific theming.
  Mechanism layer (`lib/drivers/display/src/statusline/`) defines traits:
  `ComponentProvider`, `StatuslineProvider`, with types for sections, height
  calculation, multi-row layout, and truncation. Policy layer (`modules/statusline/`)
  provides `DefaultStatuslineProvider` with built-in components: mode (with
  NORMAL=blue, INSERT=green, VISUAL=magenta coloring), filename (with [+]/[RO]
  indicators), position (line:col with percentage), and filetype. Dynamic height
  adapts to screen size with overflow strategies (Truncate, Wrap, Redistribute).
  Powerline-style separators with proper color transitions. Cross-module
  extensibility via `ComponentProviderRegistry` - other modules can register
  custom components (example: BranchComponent for git, DiagnosticsComponent for
  LSP). Component visibility conditions support WhenModified, WhenInGitRepo,
  InModes, etc. Priority-based truncation respects component importance. (#441)
- **Generic annotation system**: Implemented unified architecture for displaying
  per-line information in the gutter. Core types: `AnnotationKind` (hierarchical
  identifiers like `"diagnostic.error"`), `AnnotationTarget` (line/range/point/buffer),
  `AnnotationPayload` (number/text/severity/state). Architecture follows mechanism/
  policy separation: display driver provides traits (`AnnotationSource`,
  `AnnotationPresenter`), storage (`AnnotationStore`), and composition
  (`GutterComposer`); vim module provides policy implementations
  (`LineNumberSource`, `LineNumberPresenter`). Data flow: Sources -> Store ->
  Presenters -> Composer -> Gutter Cells. Added `GutterRenderer` high-level
  integration helper. Added 8 new highlight groups: `sign_column`,
  `gutter_separator`, `git.add`, `git.change`, `git.delete`, `fold.open`,
  `fold.closed`, `bookmark`. (#455)
- **Which-key plugin**: Added which-key module that displays available keybindings
  in a popup overlay after a configurable timeout when a prefix key is pressed.
  Features include: (1) Timeout popup - after pressing a prefix key (like `g`),
  shows available bindings after 500ms (configurable). (2) Immediate popup -
  press `?` after a prefix (like `g?`) to show bindings immediately. (3) Filtering -
  type to narrow down displayed bindings. (4) Lock-free rendering via ArcSwap
  for render thread performance. (5) Bottom-positioned overlay that adapts to
  screen size. Module provides `WhichKeyService` for timer management and
  `WhichKeySessionExt` for per-session state. (#442)
- **gRPC v2 protocol foundation**: Added protobuf schemas and tonic-build codegen
  for v2 protocol. Protocol v2 uses gRPC with raw-data model (server provides
  buffer content, cursor, options; client renders). Defines 7 services:
  InputService, StateService, BufferService, EditorService, ModuleService,
  ServerService, NotificationService (streaming). Feature-gated behind `grpc`
  feature flag. This is foundation-only; runtime gRPC transport followed.
  Part of Epic #465 (server/client crate split). (#465)

### Changed

- **BREAKING**: `REOVIM_MODULE_PATH` now prepends to search paths instead of
  appending. This gives the environment variable highest priority, matching
  the convention of `LD_LIBRARY_PATH`, `PYTHONPATH`, etc. Users who relied
  on the previous append behavior should adjust their module organization. (#433)
- **Display driver mechanism/policy separation**: Refactored display driver to
  remove all policy leakage. (1) Decoration registry now uses string-based
  `DecorationSourceKey` instead of enum variants - modules define their own keys
  (e.g., `"pair.rainbow"`). (2) `StyleGroupRegistry` allows modules to register
  default styles - rainbow bracket colors moved from driver to pair module.
  (3) Added idiomatic `FromStr`/`Display` traits for `Color` and `Attributes`
  with proper error types. (4) `Style::from_wire()` provides unified RPC
  deserialization, eliminating ~100 lines of duplicated parsing code. The
  `ThemeManager` now uses 4-tier lookup: user overrides → theme → module
  defaults → theme fallback. (#440)
- **Annotation system wiring**: Wired the generic annotation system (#455) into
  the screen handler rendering pipeline. (1) Added `GutterRendererKey` and
  `GutterRendererRegistry` following Epic #417 `ServiceRegistry` pattern. (2)
  Vim module registers `GutterRenderer` with `LineNumberSource`/`LineNumberPresenter`
  during init. (3) Extended `AnnotationContext` with optional `line_number_mode`
  for dynamic mode switching from options. (4) Screen handler uses `render_gutter()`
  helper that queries registry and falls back gracefully when not available. (5)
  Removed duplicate `calculate_gutter_width()` and `format_line_number()` functions
  from screen handler - now delegated to annotation system. Proper mechanism/policy
  separation: display driver provides registry traits, vim module provides policy
  implementations. (#458)

### Fixed

- **Shared OptionRegistry**: Fixed module-registered options not being accessible
  in the session. Modules registering options during `init()` wrote to a different
  `OptionRegistry` than the one used by the session for `:set` commands. Solution:
  create shared `OptionRegistry` before module init and pass to both
  `KernelContext::with_event_bus_services_and_options()` and
  `real_kernel_context_with_options()`. This enables `:set number` to work
  correctly with the annotation system. (#458)
- Integration tests now correctly use worktree-built modules instead of
  globally installed modules. Test harness sets `REOVIM_MODULE_PATH` to
  `target/debug/` automatically. (#433)
- **Delete operator**: Fixed linewise deletion of last line leaving an empty
  trailing line. `dd` on the last line now correctly includes the preceding
  newline, matching Vim behavior. (#434)
- **Delete operator**: Fixed undo for last-line deletion. The undo now correctly
  restores the preceding newline. (#434)
- **RPC handler**: Fixed `PopResult::ExecuteCommand` not setting `buffer_id` in
  command context, which caused delete/yank operators to fail silently. (#434)
- **CLI kill command**: Fixed `cli kill` not terminating the server. The shutdown
  signal now properly bridges from RPC handler to accept loop using a watch
  channel instead of Notify, which preserves state during accept. (#446)
- **CLI list command**: Fixed `cli list` only finding servers on ports 12522-12531.
  Now scans /proc for reovim processes listening on any TCP port, enabling
  discovery of servers started with `--tcp 0` (random port). (#446)
- **Module FFI symbols**: Fixed hot-reload-demo module missing FFI entry points
  by enabling the `dynamic` feature by default. This resolved 9 failing
  module_loading integration tests. (#448)
- **Pair plugin cursor position**: Fixed cursor position after auto-pair insertion.
  Now cursor is positioned between brackets `(|)` instead of after `()|`. Added
  `set_position()` call after `insert_at()` to restore cursor position. (#440)
- **Pair plugin decoration wiring**: Wired rainbow bracket decorations into TUI
  rendering pipeline. Created `BufferDecorationSourceRegistry` and
  `BufferDecorationSourceKey` in display driver for generic decoration lookup.
  Pair module registers in registry during init. Screen handler queries registry
  and embeds colors in `cell_grid` format. TUI parses cell_grid and applies
  per-cell styles to frame buffer. Proper mechanism/policy separation maintained:
  display driver defines traits, pair module implements them. (#440)

### Removed

- **PromptType enum**: Removed duplicate `PromptType` enum from runner layer.
  `CmdlinePrompt` in driver layer is now the single source of truth for prompt
  type. Conversion code eliminated from 3 locations. (#452)

### Refactored

- **Cmdline state architecture**: Simplified `CommandLineState` to `CmdlineBuffer`
  storing only input text and cursor position. Active state, prompt type, and
  cancellation flag now read from `CmdlineState` session extension (driver layer).
  This establishes clear SSOT: driver layer owns state flags, runner layer owns
  input buffer. Removed ~30 lines of redundant code. (#452)

---

## Version History

- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
