# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.5-dev

### Added

- **Which-key filter command refinement (#459)**: Threads binding metadata
  (description, category, layer) through the full input pipeline from
  `KeybindingRegistration` to `WhichKeyBridge`. Introduces `BindingInfo` struct
  in `reovim-driver-input` as the mechanism-layer metadata carrier. Adds
  `WhichKeyFilterConfig` session extension with three filter axes: command
  category (e.g., "motion", "operator"), key display pattern (substring match),
  and binding layer (`UserOnly`, `DefaultsOnly`, `Specific`). `WhichKeyBridge`
  applies active filters before emitting JSON hints, and emits richer fields
  (description, category, layer) alongside existing key/command data.
  `KeymapRegistry::bindings_with_prefix()` now returns `Vec<(KeySequence,
  BindingInfo)>`. 88 tests cover all filter combinations and metadata
  round-trips.

- **Which-key popup color styling (#461)**: Makes which-key popup colors
  configurable in both TUI and web clients. TUI: adds `WhichKeyStyleConfig`
  struct with 5 color fields (border, title, key, desc, category) replacing
  hardcoded `Color::*` constants. Defaults preserve original appearance.
  `WhichKeyExtension::with_style()` constructor accepts custom configs.
  Web: introduces `--whichkey-*` CSS custom properties on `.whichkey-popup`
  with fallbacks to theme variables, enabling per-element color overrides.
  8 new TUI tests + 2 new web tests cover all color slots.

- **Which-key binding category grouping (#460)**: Groups keybinding hints by
  category in the which-key popup for both TUI and web clients. Hints are sorted
  by a fixed category order (motion, operator, textobject, window, buffer) with
  unknown categories sorted alphabetically after. Category headers render above
  each group; when all hints lack categories, headers are suppressed for backward
  compatibility. TUI uses `grouped_hints()` pure function with `BTreeMap`-based
  grouping; web uses `Map`-based grouping with DOM `.whichkey-group-header`
  elements. 12 new TUI tests and 3 new web tests cover all grouping paths.

- **Notification plugin - toast messages and progress indicators (#443)**:
  Non-blocking notification system with server-side state management and
  client-side rendering. Server module (`server/modules/notification/`) provides
  `NotificationState` session extension with `push()`, `push_with_body()`,
  `push_progress()`, `update_progress()`, and `dismiss()` API. `NotificationBridge`
  serializes state to JSON for gRPC transmission via `ExtensionStateBridge`.
  TUI extension (`clients/tui/extensions/notification/`) renders stacked toasts
  in top-right corner with four levels (info, success, warning, error), auto-dismiss
  after 4s (configurable), progress bars with percentage and detail text, and
  MAX_VISIBLE=5 cap. Web extension (`clients/web/src/extensions/notification.ts`)
  provides TypeScript equivalent with DOM rendering and `setInterval`-based
  auto-dismiss. Deterministic testing via `TestClock` injection (same pattern as
  #462 which-key). 45 server tests + 48 TUI tests + 27 web tests, 100% coverage.

- **Which-key show-delay with testable clock injection (#462)**: Adds a
  configurable 500ms delay before showing the which-key popup, preventing
  visual noise during fast key sequences. Introduces `Clock` trait,
  `SystemClock`, and `TestClock` in `reovim-arch` for deterministic time
  testing. TUI extension uses a state machine (IDLE/WAITING/SHOWING) driven
  by a new `TuiExtension::tick()` method called on the 16ms redraw timer.
  Web client uses `setTimeout`/`clearTimeout` for equivalent behavior.
  22 Rust tests and 33 web tests cover all timing paths with zero real sleeps.

- **Per-client register isolation and type-safe Register enum (#515)**: Refactors
  shared state to an explicit local/shared model. Registers (`RegisterBank`),
  clipboard history (`HistoryRing`), and local marks (`MarkBank`) move from
  shared `KernelContext` to per-client `EditingState`, fixing multi-client
  isolation. Introduces kernel-level `Register` enum with six variants
  (`Default`, `Slot`, `History`, `System`, `Session`, `PeerHistory`) for
  compile-time register addressing. Decouples `ClipboardProvider` from register
  routing into a standalone `ClipboardApi` trait. Adds session-scoped shared
  registers (A-Z) via `SessionState.session_registers`. Expands `HistoryRing`
  capacity from 10 to 256 entries. Updates gRPC `GetRegistersRequest` with
  `client_id` field. Vim module operators use type-safe `Register` in
  `OperatorContext` with `char_to_register`/`option_char_to_register` bridge
  functions. E2E register and clipboard tests relocated to their policy module
  crates (`reovim-module-vim`, `reovim-module-clipboard`).

- **Layer transparency and color passthrough (#400)**: Adds per-window opacity
  support for layered rendering. Introduces `color_blend` module in the TUI
  display driver with `blend_cell_colors()` for alpha-compositing foreground
  and background colors against a configurable default background. Adds
  `ansi_to_rgb()` and `rgb_lerp()` utility functions for 256-color and
  true-color blending. `RenderConfig` struct encapsulates line number mode and
  opacity for clean parameter passing. `render_line_number()` and cell
  rendering paths use opacity-aware blending when opacity < 1.0. Includes
  30 unit tests covering all color variants, edge cases, and opacity levels.

- **Tab pages for window layout grouping (#401)**: Implements a tmux-like tab
  page system for organizing window layouts. Adds `TabPageSet` and `TabPage`
  types in the session driver with full lifecycle management (create, close,
  switch, reorder). Each tab owns an independent set of `WindowId`s. Tab
  operations (`tab_new`, `tab_close`, `tab_next`, `tab_prev`, `tab_goto`)
  wired into `SessionRuntime` and `CompositorApi`. New command IDs in
  `reovim-module-window-ops`: `TAB_NEW`, `TAB_CLOSE`, `TAB_NEXT`, `TAB_PREV`,
  `TAB_GOTO`, `TAB_COUNT`, `MOVE_TO_NEW_TAB`. Normal mode keybindings `gt`
  (next tab) and `gT` (previous tab), window mode `T` (move to new tab).
  `TabPageInfo` proto message added to gRPC v2 protocol with `active_tab_id`
  and `tabs` fields in `LayoutChanged` notifications and `GetLayoutResponse`.
  TUI stores tab state from server notifications. Per-client `TabPageSet`
  stored in `EditingState` and threaded through `ClientContext`. 263 unit
  tests across all layers.

- **Headless web capture via Playwright (#516)**: `reovim cli capture --format png
  --web-url URL` produces pixel-perfect screenshots by running the real web client
  in headless Chromium via Playwright. Adds a Node.js capture script
  (`clients/web/src/cli/capture.ts`) supporting PNG and HTML output with
  configurable viewport, DPR, and timeout. Rust CLI routes `png`/`html` formats to
  the Playwright script while `plain_text`/`raw_ansi`/`cell_grid` continue through
  the existing gRPC relay. Web client server address now uses a 3-tier resolution
  chain: `window.__REOVIM_SERVER` (Playwright/test injection), `VITE_GRPC_ADDRESS`
  (build-time env), or default (`hostname:12521`). E2E test fixtures updated to use
  `addInitScript` injection, removing the `?port=` query parameter.

- **Stateless CLI with DebugService (#468)**: CLI no longer joins as a client.
  All client-targeting operations (SendKeys, Capture, GetMode, GetCursor,
  ListClients, GetExtensionState, ListExtensions) moved to `DebugService`
  (no auth required). `InputService.target_client_id` field reverted and
  reserved. CLI commands `clients`, `extension-state`, `extensions` added.
  `Presence` subcommand replaced with `Clients`.

- **Which-key hints for operator modes (#468)**: Pressing `d`, `y`, or `c` now
  shows which-key popup with available continuations (motions, text objects).
  `ModeTransition::Push` populates `PendingBindings` with all bindings for the
  target mode + parent (inherited motions). Operator resolvers (`delete.rs`,
  `yank.rs`, `change.rs`) expose `pending_keys()` returning accumulated keys.
  `WhichKeyBridge` snapshot combines `mode_prefix` + `pending_keys` for display.
  Pressing `di` narrows hints to inner text objects (`iw`, `i(`, `i"`, etc.).

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

- **Cmdline popup rendering and headless E2E tests (#469)**: TUI-side cmdline
  extension crate with floating popup rendering, completion display, and search
  prompt visualization. Headless E2E tests verify cmdline activation, typing,
  escape, and search prompt display against a real server.

- **Noice-style cmdline UI (#451)**: Transforms the bottom-line cmdline bar
  (#469) into a centered floating popup inspired by noice.nvim with rounded
  Unicode box-drawing borders (`╭─╮│╰─╯`). Adds enhanced command-line editing
  (`<Left>`, `<Right>`, `<Home>`/`<C-a>`, `<End>`/`<C-e>`, `<BS>`, `<Del>`,
  `<C-w>` delete-word, `<C-u>` delete-to-start), command history navigation
  (`<Up>`/`<C-p>`, `<Down>`/`<C-n>` with separate command/search histories,
  deduplication, and max 100 entries), and tab-completion (`<Tab>`/`<S-Tab>`
  cycling through `ExCommandQueryService` prefix matches with completion list
  rendered inside the popup). 12 new CommandId constants, 12 command handlers,
  and 19 keybindings in `vim:command` mode. `CmdlineState` extended with cursor
  movement, editing, history, and completion methods in the driver layer.
  `CmdlineBridge` updated to expose completions state via gRPC.

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

- **Vim-specific comments in mechanism layer (#515)**: Replaced 5 Vim-leaking
  comments in `session/state.rs` (operator interception, mode push, command
  complete flow) with generic mechanism descriptions that reference modes and
  resolvers without naming specific keys (`d`, `y`, `c`) or Vim types
  (`VimSessionState`).

- **Server crate patch coverage restored to 100% (#515)**: Added 25 tests
  across 6 server crate files covering 97 previously uncovered lines:
  `PendingBindings` population on Pending/Push/Completed results with parent
  mode inheritance (state.rs), debug service session paths (debug.rs), extension
  service get/list (extension.rs), keybinding snapshot search (command.rs),
  register client-not-found (state.rs), shared scope notification (notification_builder.rs).

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
