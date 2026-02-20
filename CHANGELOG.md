# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.5-dev

### Added

- **Extension state bridge foundation (#514)**: Server-side infrastructure for
  exposing session extension state to clients via gRPC. Introduces
  `ExtensionStateBridge` trait and `BridgeRegistry` in the driver layer,
  `CmdlineBridge` as the first concrete bridge for command-line state, a new
  `ExtensionService` gRPC service with `GetState` and `ListExtensions` RPCs,
  `ExtensionUpdatedPayload` notification (field 26), and automatic CmdlineState
  change detection during key resolution. Adds `Session::with_client_extensions()`
  to access per-client extensions without cloning (works around
  `EditingState::clone()` creating empty `ExtensionMap`). Enables client
  extensions like which-key (#468) and cmdline UI (#469) to query and receive
  updates for server-side state.

- **Command query service for command completion (#453)**: Adds `ExCommandInfo`
  struct and `ExCommandQueryService` trait to the driver layer for querying
  ex-commands by prefix, name, and listing all commands. Implements the trait on
  `ExCommandRegistry` with deduplication. Registers `CommandQuerySnapshot` in
  bootstrap for keybinding command queries. Defines `CommandService` gRPC service
  with `SearchCommands` (prefix search with source filtering: all/ex/keybinding)
  and `CompleteArgs` (argument completion delegation) RPCs. Wires
  `CommandServiceImpl` into the server router. Enables cmdline UI (#469) to
  discover and complete commands via gRPC.

- **Cmdline UI core rendering (#469)**: Command-line bar (`:`, `/`, `?`) for TUI
  and web clients. Server emits per-keystroke `ExtensionUpdatedPayload`
  notifications while cmdline is active (fixes toggle-only detection from #514).
  TUI renders cmdline bar above the statusline with yellow prompt, input text,
  and inverse-video block cursor. Terminal hardware cursor repositioned to cmdline
  row in interactive mode. Web client handles `extensionUpdated` notification and
  renders cmdline using existing `.cmdline-*` CSS classes. Completions, history,
  and wildmenu deferred to follow-up issues.

### Changed

- **Explicit shared vs client extensions in resolver API (#474)**: Split the
  single `extensions: &mut ExtensionMap` parameter in `ModeKeyResolver` trait
  methods into `shared_extensions` and `client_extensions`. Modules now
  explicitly choose which extension map to use. `VimSessionState` and
  `OperatorPendingState` moved to `client_extensions` (per-client isolation),
  fixing a bug where all operator+textobject combinations (`diw`, `ciw`, `yiw`,
  `di"`, `ci{`, etc.) were broken because text objects wrote to per-client
  extensions via `runtime.ext_mut()` but operator resolvers read from shared
  `self.app.extensions`. Affected methods: `resolve_with_extensions`,
  `resolve_with_session`, `on_command_complete`. Server call sites use a
  placeholder `ExtensionMap` for `SessionRuntime` (resolvers access session
  only via `SessionApiDyn`, not `ExtensionApi`), freeing `client_extensions`
  to be passed separately without borrow conflicts.

- **Extra module loading via `REOVIM_EXTRA_MODULES` env var (#474)**:
  Integration tests can now explicitly load modules not in the defaults bundle.
  `TextObjectsModule::init()` fixed to self-register commands via
  `CommandHandlerStore` (was a no-op). `TestServerHarness::spawn_with_modules()`
  and `IntegrationTest::with_modules()` pass the env var to the spawned server.
  Four text object integration tests (`diw`, `daw`, `di"`, `ci{`) un-ignored
  and passing.

## [0.9.4-dev]

### Added

- **Per-client compositor refactor (#474)**: Each client now owns an independent
  compositor clone, eliminating the cross-namespace window ID mismatch between
  shared compositor IDs and per-client WindowLayout IDs. The compositor moves
  from `SessionShared` into per-client `EditingState` at join time via
  `boxed_clone()`. `SessionRuntime` accepts the per-client compositor as a
  parameter, and all `CompositorApi` methods operate on it. Notification builder
  reads per-client compositor for layout notifications. This is foundational
  for multi-client TUI rendering where each client has independent viewports.

- **Comprehensive unit test suite (#497)**: Added ~4500 unit tests across the
  entire workspace, bringing the total from ~3250 to 7752 passing tests (0
  failures). Coverage spans all layers: kernel (mm, ipc, core, block, sched,
  api), drivers (input, session, command, vfs, search, syntax, buffer, undo,
  clipboard, ffi), modules (vim, editor, motions, textobjects, commands, keymap,
  mode-manager, options, window-ops, buffer-ops, scratch-buffer, defaults,
  clipboard, search, undo, vfs-local, treesitter), shared libraries (arch, log,
  net, protocol, trace), clients (tui, cli, display drivers), and the server
  crate (grpc, session management, registry). All clippy lints pass under the
  project's strict `deny` policy (all, pedantic, nursery).

- **Server 100% line coverage (#499)**: Added ~560 tests to `reovim-server`,
  achieving 100% line coverage (0 DA:0 lines in LCOV, 13087/13087 instrumented
  lines hit). All 31 source files in the server crate are fully covered.
  Genuinely untestable code paths (tokio::spawn task bodies, OnceLock globals,
  gRPC stream lifecycle) are extracted into named functions with
  `#[cfg_attr(coverage_nightly, coverage(off))]`. Production code refactored:
  extracted `capture_error_to_status` (state.rs), `forward_token_updates`
  (syntax.rs), `require_debug_ring` (debug.rs), `on_notification_stream_dropped`
  (notification.rs), and `log_client_disconnect` (session.rs).

- **OT-lite conflict resolution for multi-client undo (#495)**: When multiple
  clients edit a shared buffer, each client's undo now correctly adjusts
  positions through intervening edits from other clients. Kernel provides
  general-purpose `transform_position()` (inclusion transformation),
  `TextDimensions`, `delete_end()`, `Edit::transform()`, and
  `UndoTree::edits_since()`. The undo module's `undo_for_client()` uses these
  to transform inverse edits and cursor positions through all edits that
  occurred after the target node. Without this, Client 0 undoing after Client 1
  inserted text before Client 0's edit would incorrectly remove Client 1's text.

- **Code coverage infrastructure**: Added `scripts/coverage.sh` for local
  coverage with four modes (`line`, `branch`, `mcdc`, `server`) using
  `cargo-llvm-cov`. CI generates MC/DC coverage for non-server crates and
  line coverage for `reovim-server`, merged into a single Codecov upload.
  Codecov PR annotations show coverage impact (informational, non-blocking).
  `reovim-server` excluded from branch/MC/DC due to LLVM bug
  [#119558](https://github.com/llvm/llvm-project/issues/119558)
  (`getInstantiationGroups` SIGSEGV on branch coverage data from
  `#[tonic::async_trait]` service implementations). Use `server` mode for
  text-only branch coverage of the server crate. Test code (`#[cfg(test)]`
  modules) is automatically stripped from LCOV via `scripts/lcov-filter-tests.sh`
  so coverage metrics reflect production code only.

- **Floating cursor labels for remote clients (#474)**: TUI and Web clients now
  show a colored background overlay above each remote client's cursor, making it
  easy to identify who is editing where. TUI labels use `apply_style` to preserve
  buffer content underneath (colored background without hiding text). Labels use
  the CBF-8 colorblind-friendly palette and truncate long names via Unicode-safe
  `truncate_end()`. Web labels replace the previous browser-native tooltip with
  a persistent floating `<div>`.

### Fixed

- **Cursor position not updating during insert mode (#505)**: The TUI statusline
  showed 1:1 for the cursor position throughout insert mode, only updating after
  leaving insert mode with Esc. Root cause: the fallback cursor recording in
  `send_keys` had a flawed guard `!affected_buffers.contains(&buffer_id)` that
  was defeated when `record_buffer_modified()` already added the buffer. Fix:
  always record cursor movement when any key is handled (the call is idempotent).
  Also added explicit `record_cursor_move` in the InsertChar handler.

- **One-way presence visibility (#474)**: When Client B joined after Client A,
  A could not see B's cursor. Root cause: `ClientPresence::new()` initialized
  `buffer_id: None`, so the `PresenceJoined` notification lacked the buffer_id.
  A's render engine skipped B (different-buffer filter). Fix: server now reads
  the new client's active window buffer_id before broadcasting the notification.

- **Visual selection highlighting past EOL (#474)**: Char and block mode visual
  selections highlighted empty space beyond end-of-line content, unlike vim which
  only highlights actual characters. Fix: `render_selection_range` now accepts
  buffer lines and clamps `col_end` to actual line length for char/block modes.
  Line mode (`V`) retains full-width highlighting to match vim behavior.

- **Web remote cursors disappearing on self move (#474)**: Moving the local cursor
  in the web client caused remote cursors and selections to vanish. Root cause:
  `renderMultiWindow()` and `renderSingleWindow()` rebuild the DOM, destroying
  remote cursor/selection elements. Fix: call `renderRemoteCursors()` at the end
  of both render paths to re-create remote presence after DOM rebuilds.

- **Web cursor scroll offset in single-window mode (#474)**: Cursor and remote
  presence positions in the web client's single-window mode used raw buffer
  coordinates without subtracting the viewport scroll offset. Fix: added `topLine`
  to `EditorState`, updated from `viewportUpdated` notifications, subtracted in
  `renderCursor()`, `renderRemoteCursors()`, and `renderRemoteSelection()`.

- **Resize propagation to all TUIs (#474)**: `ResizeRequestPayload` had no
  `target_client_id` field, causing all TUI clients to resize when any one
  client resized. Fix: added `target_client_id` to the proto, server sets it
  from the authenticated token, TUI handler filters by target. Also reordered
  `connect_common()` to call `resize()` after `join()` so the token is attached.

### Security

- **Connection-bound client identity (#483)**: Replaced self-reported `client_id`
  request fields with server-side token-based authentication. `Join()` returns a
  `session_token`; clients send it via `x-reovim-token` metadata header; the
  `AuthInterceptor` resolves tokens to `ClientId` server-side. Caller-identity
  RPCs (`send_keys`, `leave`, `update_presence`, `set_sync_mode`) now require
  token authentication — the body `client_id` field is removed and reserved.
  State-query RPCs (`get_mode`, `get_cursor`, `get_layout`) use the token for
  caller authentication while the body `client_id` selects the target (0 = self).

### Removed

- **Dead `CmdlineBuffer` from `AppState` (#452)**: Removed unused `CmdlineBuffer`
  field from server-level `AppState` and deleted `app/cmdline.rs`. The cmdline
  buffer was superseded by `CmdlineState` in the driver layer during v0.9.2.
  This completes the cmdline state consolidation started in #452.

---

## Version History

- v0.9.3 - Per-client architecture, TUI unification, presence rendering, integrated mode - see [CHANGELOG-0.9.3.md](changelog/CHANGELOG-0.9.3.md)
- v0.9.2 - Cmdline UI, themes, annotations, gRPC v2, web client, syntax service - see [CHANGELOG-0.9.2.md](changelog/CHANGELOG-0.9.2.md)
- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
