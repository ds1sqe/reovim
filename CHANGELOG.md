# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.3-dev

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

- **Visual mode resolver missing**: Fixed bug where Visual mode keys
  (h, j, k, l, w, b, e, etc.) were not working - either inserting characters or
  being ignored. Root cause: No resolver registered for Visual modes, causing
  `ResolverRegistry.get()` to return `None` and bypass mode inheritance. Solution:
  Created `VimVisualResolver` (~350 LOC) that handles escape, count accumulation,
  and delegates motion keys to Normal mode keymap. Registered for all 3 visual
  variants (`VISUAL_ID`, `VISUAL_LINE_ID`, `VISUAL_BLOCK_ID`). Server now has 10
  resolvers (was 7). Part of Epic #465. (#465)

### Added

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
