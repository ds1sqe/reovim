# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.2-dev

### Added

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
  them to `CmdlineBuffer` methods. This is Phase 1 infrastructure; floating
  popup rendering in TUI to follow. (#451)

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

### Fixed

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
