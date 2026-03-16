# Changelog - v0.14.0

## [0.14.0] - 2026-03-16

### Added

- **Tab / buffer bar — bufferline (#662, part of #660)**:
  Horizontal tab bar showing open buffers at the top of the editor. Two-layer
  architecture: server module (`reovim-module-bufferline`) maintains buffer list
  via EventBus subscriptions (`BufferCreated`, `BufferClosed`, `BufferModified`,
  `BufferSaved`) with `ExtensionScope::Shared` bridge. TUI client module
  (`reovim-tui-mod-bufferline`) renders at `ChromePosition::Top`. Features:
  active buffer highlight (blue), modified `[+]` indicator, pinned `*` indicator
  with pin-first ordering, per-buffer diagnostic counts (`E:n W:n`), overflow
  scroll with `<`/`>` indicators. Commands: pin toggle (`<leader>bp`), unpin
  (`<leader>bu`), close buffer (`<leader>bc`). `BufferListService` provides
  buffer metadata, `BufferlineBridge` merges with `DiagnosticSnapshot` and
  `BufferlineState` (pin list) during tick.

- **Git hunk operations (#668, part of #660)**:
  Extend git-signs module with hunk navigation and operations. `GitProvider`
  trait extended with write operations: `stage_file`, `reset_file`,
  `unstage_file`, `stage_lines`, `reset_lines`, `diff_content`, `invalidate`.
  `SubprocessGitProvider` implements all via git CLI with `run_git_with_stdin`
  helper for patch piping. `CachedGitProvider` forwards write ops and
  invalidates per-path caches. Navigation: `]h`/`[h` jump between hunks with
  wrap-around. Commands: stage hunk, reset hunk, stage buffer, reset buffer,
  unstage file, preview hunk, diff this. 9 keybindings under `<leader>gh`
  prefix. Pure navigation functions (`next_hunk_line`, `prev_hunk_line`,
  `hunk_at_cursor`) with full test coverage.

- **Diagnostics panel — trouble (#665, part of #660)**:
  Navigable diagnostics list with filtering and sorting, equivalent to
  trouble.nvim. Per-client `DiagnosticsState` as `SessionExtension` with
  `DiagnosticsPanelBridge` for JSON serialization to TUI/Web. Panel modes:
  Diagnostics, Quickfix (stub), References (stub), TODO (stub). Sort by
  severity/file/line with cycle command. Severity filter (errors only,
  warnings only, show all). `DIAGNOSTICS` mode with dedicated resolver
  (no char input, keymap-only). Commands: `:Trouble [mode]`, toggle, close,
  next/prev, select (jump), filter, sort, refresh. Keybindings: `<leader>xx`
  toggle, j/k/Enter/q/Esc in panel mode, e/w/a for filter, s for sort,
  r for refresh. Diagnostic types (`DiagnosticSnapshot`, `DiagnosticItem`,
  `BufferDiagnosticEntry`, `DiagnosticSeverity`) extracted from
  `reovim-module-lsp` to `reovim-driver-lsp` for proper layer boundaries.
  88 unit tests, zero clippy warnings.

- **Format-on-save with external and LSP formatters (#667, part of #660)**:
  Pre-save formatting via `BufferWillSave` event subscription. External CLI
  formatters (rustfmt, prettier, black, etc.) via `FormatterProvider` trait
  and `ExternalFormatter` with stdin piping, timeout handling, and `{path}`
  substitution. LSP `textDocument/formatting` with `TextEdit` application
  (sorted descending, applied in reverse to preserve offsets). Formatter
  resolution order: external > LSP > no-op. `autoformat` boolean option
  (default true) for `:set noautoformat` toggle. `:Format` and `:FormatRange`
  commands with undo recording via `replace_content()`. `FormatterRegistry`
  keyed by filetype, `FormatterConfig` types, `FormatError` enum. Per-filetype
  configuration support. `FORMATTER_PROVIDER` capability constant. 63 format
  module tests + 37 formatter driver tests, zero clippy warnings.

- **Word reference highlight — illuminate module (#664, part of #660)**:
  Highlights all references to the symbol under cursor using LSP
  `textDocument/documentHighlight` with word-match fallback. Tick-based
  cursor-hold detection (3 ticks at 100ms = 300ms delay) via bridge
  `ExtensionStateBridge`. Navigation commands `]]` / `[[` jump between
  highlighted references with wrap-around. CursorMoved kernel event
  emission from `SessionRuntime::record_cursor_move()` with accurate
  from/to positions. CursorSnapshot session extension for bridge tick
  consumption without direct window access. LSP driver document highlight
  request support with capability registration. 79 illuminate tests,
  12 CursorMoved/CursorSnapshot tests, 100% line coverage.

- **Leap-style bidirectional jump motions (#663, part of #660)**: Enhanced
  range-finder module with viewport-bounded scanning and backward search.
  `s{char}{char}` now searches forward only (was bidirectional) with matches
  limited to visible viewport lines for large-file performance. New `S`
  keybinding for backward jump search (`Direction::Backward`). Jump targets
  use `line_offset` translation for correct absolute buffer coordinates when
  viewport is scrolled. Operator-pending integration: `ds`, `ys`, `cs` with
  jump motions via deferred `on_command_complete` pattern — operator resolvers
  (delete, yank, change, case) now peek `pending_motion` and defer completion
  when cursor hasn't moved (multi-step motion pushed a mode for label
  selection). `start_with_matches()` resets `line_offset` to avoid stale
  offsets from prior viewport-bounded searches. 25 new tests across
  range-finder and vim modules.

- **H, M, L screen-position motions (#669, part of #660)**: Three new motions
  that move the cursor relative to the viewport. `H` moves to the top of the
  visible screen (with optional count for Nth line from top). `M` moves to the
  middle of the screen. `L` moves to the bottom (with optional count for Nth
  line from bottom). All three are linewise jump motions that push to the jump
  list and work in normal, visual, and operator-pending modes (dH, yM, cL).
  Viewport clamping prevents cursor from moving past buffer bounds. 32 tests.

- **Rich statusline segments (#661, part of #660)**: Rewritten TUI statusline
  from minimal mode+position to lualine-style sections (A/B/C/X/Y/Z). Section
  A: mode badge. Section C: filename with modified [+] and readonly [RO]
  indicators. Section Y: filetype and encoding. Section Z: progress percentage
  (Top/Bot/All/N%) and line:col position. Buffer metadata flows via JSON
  notification from `list_buffers()` gRPC call after layout and buffer changes.
  Background fill, section separators, and right-aligned rendering. 59 tests.

- **Client-side animations (#657, part of #653)**: Yank flash highlight and
  landing screen animations for TUI and Web clients. Server-side
  `YankFlashBridge` emits yank range via existing `extension_updated` pipeline
  (no proto changes). TUI `yank-flash` ClientModule renders 200ms background
  highlight via `inline_decorations()` with `Clock` trait for deterministic
  testing. Web `YankFlashExtension` applies CSS highlight class with
  `setTimeout` auto-dismiss. TUI landing screen gains breathing color animation
  (6-frame cycle, 500ms/frame) with periodic roar flash (4-frame, 100ms/frame,
  every 8s). Web landing screen gets CSS keyframe breathing animation with
  periodic roar class swap.

- **UI/UX visual polish (#659, part of #653)**: Comprehensive visual
  enhancements with expanded highlight groups, Nerd Font icons, and
  diagnostic gutter support. Highlight groups expanded from 42 to 73
  with 31 syntax sub-categories (keyword.control, function.macro,
  type.builtin, variable.parameter, etc.) and explicit palette entries
  in all 3 builtin themes (Dark, Light, Tokyo Night Orange).
  Completion popup shows Nerd Font kind icons (17 CompletionKind
  variants mapped to glyphs). Notification level icons replaced with
  Nerd Font (info, check, alert, close_circle). Explorer shows file
  type icons for 20+ extensions and special directory icons. Picker
  results show file type icons. DiagnosticPresenter renders severity
  icons in the gutter (error/warning/info/hint with colors). Fixed web
  DIAGNOSTIC_WARN key mismatch (diagnostic.warn -> diagnostic.warning).

- **Treesitter-based semantic text objects (#656, part of #653)**: 14 new
  treesitter-based text objects (7 kinds x inner/around): function (if/af),
  class (ic/ac), argument (ia/aa), conditional (io/ao), loop (il/al),
  comment (i//a/), block (deferred keybinding). `SyntaxDriver::textobject_range()`
  trait method with default no-op. `TreeSitterDriver` implementation using
  `QueryCursor` with smallest-node selection for nested constructs.
  `textobjects.scm` query files for Rust (functions, structs/enums/traits/impls,
  parameters, conditionals, loops, comments, blocks) and Python (functions,
  classes, parameters, conditionals, loops, comments, blocks). 14 command
  handlers in textobjects module with `SyntaxSessionState` driver lookup.
  Keybindings in all operator-pending modes (delete, yank, change) and visual
  modes. Graceful degradation when no syntax driver available. Block keybinding
  (iB/aB) deferred due to conflict with existing brace textobject.

- **Syntax highlight pipeline (#655, part of #653)**: End-to-end wiring of the
  syntax highlighting pipeline. Fixed `insert_text()` missing `BufferModified`
  EventBus emission (bug). Added `start_byte` to `Modification` enum for
  incremental tree-sitter parsing via `SyntaxEdit`. `emit_syntax_updates()` now
  uses `driver.update()` (incremental O(edit)) when edit info is available,
  falling back to `driver.parse()` (full O(n)) otherwise. Syntax drivers cleaned
  up on buffer close. TUI subscribes to real-time `StreamTokens` stream instead
  of poll-based `get_tokens()` refresh. `modification_to_syntax_edit()` conversion
  in server layer (kernel purity). 13 new tests. Search highlighting deferred.

- **Marks and jump list (#654, part of #653)**: Full mark and jump list support.
  `SetMark` (m{char}) sets local (a-z) or global (A-Z) marks at cursor position.
  `GotoMarkLine` ('{char}) jumps to mark line (column 0). `GotoMarkExact`
  (`` ` ``{char}) jumps to exact mark position. `JumpBackward` (Ctrl-O) and
  `JumpForward` (Ctrl-I) navigate the per-client jump list. Major jumps (gg, G,
  n, N, *, #, mark goto) automatically push current position to the jump list
  before moving. Mark commands use the `PendingCharOp` resolver pattern (same as
  f/F/t/T/r). `Jumplist` added to per-client state chain (`ClientContext`,
  `EditingState`, `SessionRuntime`). 22 new tests for mark and jump commands,
  33 test files updated for Jumplist cascade.

- **v0.8.1 feature parity audit (#652, part of #645)**: Comprehensive feature
  matrix document comparing the pre-kernel v0.8.1 archive against the current
  codebase across 11 categories (core editing, motions, operators, text objects,
  visual mode, search, ex commands, window management, plugins/modules, language
  support, UI features). Overall parity: ~95%. Identified 5 gaps: semantic text
  objects (treesitter-based), marks (kernel ready, vim commands stubbed), jump
  list (kernel ready, keybindings not wired), yank animation, animated landing
  mascot. Documented 20+ features new in current that did not exist in v0.8.1.

- **Web CLM rendering infrastructure (#651, part of #645)**: Chrome compositor,
  DOM render surface, and chrome dispatcher for the web client.
  `ChromeCompositor` allocates non-overlapping screen regions using TUI-matching
  edge-inward algorithm with four independent counters and priority sorting.
  `DomChromeSurface` implements `RenderSurface` for DOM-based character grid
  rendering with virtual buffer and style-to-CSS conversion. `ChromeDispatcher`
  orchestrates layout, container lifecycle, and dual rendering paths (DOM for
  existing `WebExtensionAdapter` modules, cell-grid for future native CLM).
  `Editor.extensionUpdated` handler decouples notification dispatch from
  rendering. Z-index strategy: edge chrome by allocation order, overlays at
  1000+zOrder. Fixed pre-existing `LandingExtension` test count bug in
  `extensions.test.ts`. 32 new tests across 3 test files.

- **Web CLM contracts and module lifecycle (#650, part of #645)**: TypeScript
  interfaces mirroring all Rust CLM traits (`PlatformCapabilities`,
  `RenderSurface`, `ServerHandle`, `ClmThemeProvider`, `ClientModuleRegistry`,
  `ClientServiceRegistry`, `ModuleContext`, `ClientModule`) in
  `clients/web/src/core/`. `BrowserPlatformAdapter` queries browser
  capabilities with SSR-safe guards. `GrpcServerHandle` wraps the existing
  `ReovimClient`. `ThemeProviderAdapter` bridges web `ThemeManager` to CLM
  interface. `WebExtensionAdapter` wraps all 9 existing `WebExtension`
  implementations into `ClientModule` without behavior change.
  `ClientModuleLoader` provides topological sort, 3-pass deferral init, and
  reverse-order shutdown. `Editor` and `HeadlessWebClient` upgraded from raw
  `createExtensions()` to CLM loader. 119 new tests across 9 test files.

- **ClientModule FFI event and role declaration trampolines (#648, part of #645)**:
  Extended the client module FFI boundary with 16 new trampolines for event
  dispatch (`on_notification`, `on_mode_change`, `on_cursor_update`,
  `on_buffer_focus`, `on_buffer_update`, `on_option_changed`, `tick`) and role
  declaration (`has_chrome`, `has_buffer_contrib`, `has_annotations`,
  `chrome_position`, `chrome_requested_size`, `chrome_priority`, `chrome_z_order`,
  `buffer_contrib_priority`, `annotation_priority`). `ClientModuleHandle` now
  dispatches all event and role methods through both static trait and dynamic FFI
  paths. `ClientModuleProbe` gains `capabilities` bitflags for probe-time role
  detection. `declare_client_module!` proc macro generates all new trampoline
  symbols. `CLIENT_MODULE_API_VERSION` bumped to 0.3.0.

- **ClientModule SDK refinement (#646, part of #645)**: Enriched the client driver
  crate for external module development (primary consumer: Tilth game).
  `ClientModuleError` is now an enum with `InitFailed`, `ExitFailed`,
  `NotificationParse`, `Other` variants, `Display`/`Error` impls, and `source()`
  chain. New `ScopedSurface` wrapper provides automatic coordinate offset and
  clipping for bounded chrome rendering. New `ClientServiceRegistry` (TypeId-based,
  thread-safe) and `ClientModuleRegistry` trait for cross-module service discovery
  and module state introspection. `ModuleContext` gains `services` and
  `module_registry` fields (both `Option` for FFI backward compat). `ServerHandle`
  expanded with `list_commands()` and `get_option_metadata()` (default impls).
  New `OptionMetadata` and `OptionKind` types. `Rect::intersect()` and
  `Rect::contains_point()` helpers. Notification parsing helpers
  (`parse_notification`, `parse_notification_field`) behind `serde` feature gate.
  `CLIENT_MODULE_API_VERSION` bumped to 0.2.0. 357 tests, zero clippy warnings.

- **ClientModule testing utilities (#647, part of #645)**: New `testing` module in
  `reovim-client-driver` (behind `testing` feature gate) with `RecordingSurface`
  (cell-grid with assertion methods), `WriteSurface` (write-log pattern),
  `MockPlatformCapabilities` (builder), `MockServerHandle` (option seeding,
  command recording), `MockThemeProvider` (highlight seeding),
  `MockModuleRegistry`, and `TestModuleContext` builder. Migrated 15 TUI module
  test files to shared mocks, eliminating duplicated boilerplate. Driver crate
  dogfoods its own testing API. 378 driver tests, 481 TUI tests, zero warnings.

- **Codec metadata end-to-end pipeline (#649, part of #645)**: Fixed latent bug where
  `CodecSessionState` was stored in per-client extensions instead of shared (per-buffer)
  extensions. gRPC `ListBuffers` response now populates `codec_metadata` (codec name,
  line ending, BOM flag) from `CodecSessionState`. CLI `buffers` command displays
  codec info in both plain (`[utf-8, lf]`) and JSON formats.

- **Marks and jump list user-facing commands (#654)**: Implemented complete marks and
  jump list feature for editor. `SetMark`, `GotoMarkLine`, and `GotoMarkExact` commands
  handle local (a-z) and global (A-Z) marks using kernel infrastructure. Vim module
  resolver wires `m`, `'`, and `` ` `` prefixes via `PendingCharOp` pattern. `JumpBackward`
  (Ctrl-O) and `JumpForward` (Ctrl-I) navigate the per-client jump list. Integrated
  push-on-jump into `gg`, `G`, search motions, and mark jumps. 26 new mark tests, 7 jump
  tests, integrated into 33 existing test files via `Jumplist` cascade. Zero warnings.

### Changed

- Version bump to 0.12.1-dev for Phase 13 (SDK polish, integration, web alignment)

