# Changelog

All notable changes to Reovim will be documented in this file.

## [0.6.12] - 2025-12-20

### Breaking Changes

- **Removed `Focus` enum** - Use `InteractorId` exclusively for focus tracking
  - `ModeState.focus` field removed
  - `ModeState::with_focus_and_mode()` removed
  - `ModeState::with_sub_mode()` (Focus-based) removed
  - `ModeState::with_focus()` removed

### Architecture

- **InteractorId-based Focus System** - Complete migration from closed enum to extensible IDs
  - `ModeState` now contains only `interactor_id`, `edit_mode`, `sub_mode`
  - Pattern matching replaced with `InteractorId` equality checks
  - Enables plugins to define custom focus targets without core changes

### Changed

- **modd/mod.rs** - Simplified `ModeState` struct
  - Removed `focus: Focus` field
  - `display_string()` now matches on `interactor_id`
  - All convenience constructors updated

- **Status Line Rendering** - Updated for `InteractorId`
  - `mode_icon()` uses `InteractorId` comparison
  - New `get_mode_style()` helper method
  - Both `component/status_line.rs` and `screen/status_line.rs` updated

- **Keymap Selection** - `bind/mod.rs`
  - `get_keymap_for_mode()` matches on `interactor_id` instead of `focus`
  - Changed from `const fn` to `fn` (InteractorId comparison not const)

- **Runtime Handlers** - `runtime/handlers.rs`
  - `sync_mode_with_screen_focus()` uses `InteractorId::EXPLORER`
  - Telescope handlers use convenience constructors

- **Command Dispatcher** - `dispatcher.rs`
  - `telescope_enter_normal` uses `ModeState::telescope_normal()`

### Migration Guide

```rust
// Before:
match (&mode.focus, &mode.edit_mode) {
    (Focus::Editor, EditMode::Normal) => ...
}

// After:
if mode.interactor_id == InteractorId::EDITOR {
    match &mode.edit_mode {
        EditMode::Normal => ...
    }
}

// Before:
ModeState::with_focus_and_mode(Focus::Telescope, EditMode::Normal)

// After:
ModeState::telescope_normal()
```

### Testing

- 318 unit tests passing
- All integration tests passing
- Zero clippy warnings

## [0.6.11] - 2025-12-20

### Architecture

- **DisplayComponent Trait** (`lib/core/src/component/`)
  - `DisplayComponent` trait for display-only UI components (no input handling)
  - `RenderContext` struct passed to components during rendering
  - Clean separation: Interactor for input, DisplayComponent for display

- **StatusLineComponent** - Renders mode indicator, pending keys, filename, cursor position
  - Uses `DisplayComponent` trait
  - Mode-specific icons and styling

- **TabLineComponent** - Renders tab bar when multiple tabs are open
  - Uses `DisplayComponent` trait
  - Visibility conditional on tab count

- **CommandLine Interactor** - Input handling for `:` command mode
  - `InteractorId::COMMAND_LINE` for identifying command line focus
  - `CommandLineInt` struct implementing `Interactor` trait
  - `handle_command_line_input()` enlist handler

### Changed

- **BaseLayer uses DisplayComponents** - Rendering delegated to components
  - `TabLineComponent` replaces inline tab line rendering
  - `StatusLineComponent` replaces inline status line rendering
  - Removed unused `render_tab_line()` and `render_status_line()` methods

- **Screen layer module made public** - `pub mod layer` for component access

### Files Added

- `lib/core/src/component/mod.rs` - Component module exports
- `lib/core/src/component/display.rs` - DisplayComponent trait + RenderContext
- `lib/core/src/component/status_line.rs` - StatusLineComponent
- `lib/core/src/component/tab_line.rs` - TabLineComponent

### Testing

- 318 unit tests passing
- All integration tests passing

## [0.6.10] - 2025-12-20

### Architecture

- **Enlist-Based Handler Registration** (Bevy/Zed Pattern)
  - Fully removes hardcoding from runtime dispatch - only enlist (initialization) knows about specific interactors
  - Function pointer registry: `HashMap<InteractorId, FocusInputHandler>`
  - Handlers registered at initialization via `enlist_focus_input_handler()`
  - Runtime dispatch is fully generic via handler lookup

- **Interactor System** (`lib/core/src/interactor/`) - Renamed from Focus System
  - `InteractorId` for identifying input targets (Editor, Telescope, Explorer, etc.)
  - `Interactor` trait for input-receiving components
  - `InteractorRegistry` for managing interactors and active focus
  - `InputResult` enum: `NotHandled`, `Handled`, `SendEvent(InnerEvent)`

### Changed

- **Generic Event Dispatch** - Runtime no longer matches on interactor types
  - Before: `match InteractorId::TELESCOPE => ...`
  - After: `handlers.get(&interactor_id)` - fully generic lookup
  - Adding new interactors only requires calling `enlist_*` at initialization

- **FocusInput Event** - Generic focus input event for all interactors
  - `InnerEvent::FocusInput { char, delete, clear_landing }`
  - Dispatched to registered handler based on active `InteractorId`

### Files Added

- `lib/core/src/interactor/mod.rs` - Interactor trait and registry
- `lib/core/src/runtime/enlist.rs` - Handler implementations (enlist pattern)

### Files Removed

- `lib/core/src/focus/mod.rs` - Replaced by interactor module

### Testing

- 318 unit tests passing
- 132 integration tests passing

## [0.6.9] - 2025-12-20

### Architecture

- **Compositor System** (`lib/core/src/compositor/`)
  - `ZOrder` and `ZGroup` for layered rendering priority (Base, Editor, Popup, Overlay, Modal)
  - `Bounds` for component layout bounds with collision detection
  - `ComposableId` for unique component identification

- **Modifier System** (`lib/core/src/modifier/`)
  - `Modifier` trait for context-aware styling and behavior
  - `ModifierRegistry` with caching for efficient evaluation
  - `ModifierContext` for passing window/buffer/mode context
  - `StyleModifiers` for visual style overrides (borders, decorations, colors)
  - `BehaviorModifiers` for keybinding overrides and feature flags

- **Border System** (`lib/core/src/screen/border.rs`)
  - `BorderStyle` variants (None, Single, Double, Rounded, Heavy, Custom)
  - `BorderSides` for selective borders (top, bottom, left, right)
  - `BorderConfig` builder pattern with fluent API
  - `BorderMode::Collide` and `BorderMode::Float` for multi-window layouts

- **Filetype Detection** (`lib/core/src/filetype/`)
  - Extension-based and filename-based detection
  - `FiletypeInfo` with icon and color
  - Global `FiletypeRegistry` for language detection

### Changed

- **Focus-based Input Routing** - Input events now routed via `FocusInputEvent`
  - `FocusInputEvent::InsertChar(char)` for character insertion
  - `FocusInputEvent::DeleteCharBackward` for backspace handling
  - CommandHandler emits `FocusInputEvent` for insert/command/telescope modes
  - Runtime routes events to Telescope, Explorer, or Editor based on `FocusId`

### Files Added

- `lib/core/src/compositor/mod.rs` - Compositor types and ZOrder
- `lib/core/src/compositor/types.rs` - Bounds, ComposableId
- `lib/core/src/modifier/mod.rs` - Modifier module root
- `lib/core/src/modifier/traits.rs` - Modifier trait definition
- `lib/core/src/modifier/registry.rs` - ModifierRegistry
- `lib/core/src/modifier/context.rs` - ModifierContext
- `lib/core/src/modifier/style.rs` - StyleModifiers
- `lib/core/src/modifier/behavior.rs` - BehaviorModifiers
- `lib/core/src/focus/mod.rs` - Focus system
- `lib/core/src/filetype/mod.rs` - Filetype detection
- `lib/core/src/screen/border.rs` - Border system

### Testing

- 320 unit tests passing
- 154 integration tests passing

## [0.6.8] - 2025-12-19

### Fixed

- **Transparent Background Support** - Editor now uses terminal's native background
  - Removed explicit background from `base.default` and `command_line` styles
  - Fixed visual artifacts by emitting ANSI code `49` (background reset) when `bg: None`
  - Added `flush()` after `clear()` in screen `initialize()` and `finalize()`

### Changed

- `Style::to_ansi_start()` now emits `49` reset code when background is `None`
- All themes (Dark, Light, TokyoNight) use terminal default background for text areas

## [0.6.7] - 2025-12-19

### Fixed

- **Independent Scroll Per Window** - Each window maintains its own scroll position
  - Fixed bug where scrolling in one window would sync across all windows sharing the same buffer
  - Root cause: All windows called `update_scroll()` with buffer's live cursor
  - Fix: Active window uses `buf.cur.y`, inactive windows use saved `win.cursor.y`

### Added

- **`state/windows` RPC Endpoint** - Query per-window scroll and cursor state
  - `WindowSnapshot` struct with `buffer_anchor`, `cursor`, `is_active` fields
  - Useful for testing and debugging window state

### Tests

- `test_shared_buffer_scroll_independence` - Verifies scroll isolation between splits
- `test_shared_buffer_scroll_preserved_after_navigation` - Verifies scroll preserved on window switch
- `test_three_windows_scroll_isolation` - Verifies 3-window scroll independence

## [0.6.6] - 2025-12-19

### Architecture

- **Double-Buffer Rendering Refactor** - Simplified and unified rendering architecture
  - `FrameRenderer` uses front/back buffer swap pattern for flicker-free updates
  - `front` buffer: Latest complete frame (external readers access this)
  - `back` buffer: Currently being rendered to
  - Cell-by-cell diff computed between buffers, only changed cells sent to terminal
  - After flush: `std::mem::swap(&mut front, &mut back)` for efficient buffer rotation

- **Unified Capture System** - Consolidated all RPC capture formats into `FrameBufferHandle`
  - Single capture handle provides: `RawAnsi`, `PlainText`, `CellGrid` formats
  - Added `to_ansi()` and `to_plain_text()` methods to `FrameBuffer`
  - Removed `CaptureHandle` and `DualOutput` (redundant with unified system)
  - Capture buffer updated after each flush for RPC clients

- **Rendering Pipeline Cleanup**
  - Removed unused render strategies (`cell_delta`, `dirty_region`, `virtual_buffer`)
  - Removed unused `screen/compositor.rs` - rendering now via `Screen::render_buffered()`
  - Streamlined frame module: just `buffer.rs`, `cell.rs`, `renderer.rs`
  - Direct layer rendering in z-order without intermediate compositor

### Documentation

- Updated `docs/architecture.md`:
  - Rendering flow diagram now reflects actual `Screen::render_buffered()` implementation
  - Removed stale `LayerCompositor` references
  - Documented double-buffer swap pattern
- Updated `CLAUDE.md` - Simplified RPC capture documentation

### Removed

- `lib/core/src/frame/dirty.rs` - Unused dirty tracking
- `lib/core/src/frame/strategy/` - Entire unused strategy module (4 files)
- `lib/core/src/screen/compositor.rs` - Unused z-layer compositor
- `lib/core/src/io/output.rs` - `CaptureHandle`, `DualOutput` types

### Performance

- Window render (10 lines): 10.14 µs
- Full screen render: 62.80 µs
- Char insert RTT: 108 µs
- Throughput: ~18.1k renders/sec
- Double-buffer swap: O(1) pointer swap, no data copy

## [0.6.5] - 2025-12-19

### Features

- **Smart Window Focus/Split** - `<leader>wh/j/k/l` focuses existing window OR creates split
  - Unified window operations under `<leader>w*` prefix
  - Smart behavior: navigate if adjacent window exists, split if not
  - Removed `<C-hjkl>` from normal mode (use `<leader>w*` instead)

- **Buffer Navigation** - LazyVim-style buffer switching
  - `H` / `L` - Previous/next buffer
  - `<leader>bd` - Delete current buffer
  - `<leader>b` prefix group in which-key

- **Change Line Command** - `cc` deletes line content and enters insert mode
  - Consistent with `dd` (delete line) and `yy` (yank line)

### Documentation

- **Window-Buffer Architecture** - New `docs/window-buffer.md`
  - Explains N:1 window-to-buffer relationship
  - Dual cursor state (buffer cursor vs window cursor)
  - Cursor handoff protocol on window switch

### Fixed

- **Smart Focus Cursor Bug** - `handle_focus_or_split` now uses proper cursor save/restore
  - Was calling `screen.navigate_window()` directly, bypassing cursor handoff
  - Now calls `handle_window_navigate()` for correct behavior

- **Cursor Rendering in Multi-Window** - Fixed cursor highlight appearing in wrong window
  - `render_buffered()` was calculating cursor position for ALL windows
  - Last window in loop always overwrote `cursor_pos`, ignoring active window
  - Fix: Added `win.is_active &&` check before cursor position calculation
  - Line numbers updated correctly but cursor highlight didn't follow

### New Keymaps

| Key | Action |
|-----|--------|
| `H` | Previous buffer |
| `L` | Next buffer |
| `cc` | Change line |
| `<leader>b` | +buffer (prefix) |
| `<leader>bd` | Delete buffer |
| `<leader>wh` | Smart focus left (or vsplit) |
| `<leader>wj` | Smart focus down (or hsplit) |
| `<leader>wk` | Smart focus up (or hsplit) |
| `<leader>wl` | Smart focus right (or vsplit) |
| `<leader>wv` | Vertical split |
| `<leader>ws` | Horizontal split |
| `<leader>wc` | Close window |
| `<leader>wo` | Close other windows |
| `<leader>w=` | Equalize windows |

### Removed Keymaps

- `<C-h/j/k/l>` - Window focus (now use `<leader>wh/j/k/l`)
- `<C-S-H/J/K/L>` - Window move (all window ops under `<leader>w`)

### Testing

- Add 13 window split integration tests (`window_splits.rs`)
  - 11 window split/focus tests
  - 2 shared buffer cursor preservation tests
- Update visual_snapshot tests to use `<leader>w*` instead of `<C-h/l>`
- Total: 311 tests passing

## [0.6.4] - 2025-12-19

### Features

- **Per-Window Cursor** - Each window maintains its own cursor position (Vim-like behavior)
  - `cursor` and `desired_col` fields added to Window struct
  - Cursor syncs between window and buffer on navigation
  - `effective_cursor_y()` method for rendering (active uses buffer cursor, inactive uses window cursor)

- **Inactive Line Number Styling** - Dimmed line numbers for inactive windows
  - `inactive_line_number` style in theme's gutter configuration
  - `fg_gutter` (#3b4261) and `fg_inactive` (#292e42) colors for TokyoNightOrange
  - `is_active` field on Window to track focus state

### Fixed

- **Split Window Cursor Movement** - j/k now works immediately after Ctrl-h/l window navigation
  - `active_buffer_id` now syncs when navigating between windows
  - Buffer cursor properly loads from window's saved cursor on switch

### Testing

- Add 6 split window visual tests
  - `test_vsplit_creates_two_windows` - Verify split shows content in both windows
  - `test_vsplit_navigate_right/left` - Test Ctrl-l/h navigation
  - `test_vsplit_cursor_movement_after_navigate` - Verify j/k works after switch
  - `test_vsplit_independent_cursors` - Windows maintain separate cursor positions
  - `test_vsplit_cursor_preserved_on_switch` - Cursor restored when returning to window

## [0.6.3] - 2025-12-19

### Features

- **Visual Testing Infrastructure** - Comprehensive TUI debugging for tests and AI assistants
  - ASCII art snapshots: Plain and annotated views with borders/line numbers
  - Structured visual snapshots: Cell grid, cursor info, layer visibility (JSON)
  - `with_size(width, height)` builder method for explicit screen dimensions
  - `VisualAssertions` trait: `assert_cell_char`, `assert_row_content`, `assert_contains`, etc.

### New Modules

- `lib/core/src/visual/` - Visual snapshot types and ASCII rendering
  - `VisualSnapshot`, `CursorInfo`, `LayerInfo`, `BoundsInfo`
  - `AsciiRenderConfig` for customized ASCII output
- `lib/core/src/testing/visual.rs` - Visual assertions trait

### RPC Methods

- `state/visual_snapshot` - Full visual snapshot with cells, cursor, layers
- `state/ascii_art` - ASCII art representation (plain or annotated)
- `state/layer_info` - Layer visibility and bounds information

### Testing

- Add 17 visual snapshot integration tests
- Add `visual_snapshot()`, `ascii_art()`, `layer_info()` to `ServerTestResult`
- Update `docs/TESTING.md` with visual testing examples

## [0.6.2] - 2025-12-19

### Fixed

- **Buffer Switching Not Updating Screen** - Fix 6 locations where screen state wasn't synchronized
  - Telescope buffer picker now properly switches displayed buffer
  - Telescope file picker and grep match now sync screen after open
  - `:e` command now updates screen after opening file
  - `BufferEvent::Switch` now syncs screen
  - `:split`/`:vsplit` and `:tabnew` use correct buffer ID after file open

### Testing

- Add buffer switching integration tests (10 tests)
- Add `screen()`, `buffer_list()`, `open_file()` to testing framework
- Add `assert_active_buffer_id()` and `active_buffer_id()` assertions

## [0.6.1] - 2025-12-19

### Fixed

- **Cursor Blinking During Movement** - Hide cursor during render operations
  - Wrap render with Hide/Show escape sequences to prevent visual artifacts
  - Fixes flickering cursor when navigating with hjkl

### Testing

- Add cursor visibility integration tests
- Add `screen_content_raw()` to testing framework
- Add `assert_cursor_visibility_managed()` assertion

### Chores

- Add `cargo +nightly fmt --all` step to `scripts/check.sh` for consistent formatting

## [0.6.0] - 2025-12-18

### Features

- **Diff-Based Rendering System** - Eliminates terminal flickering
  - FrameBuffer: 2D cell grid for double-buffering
  - Three render strategies: VirtualBuffer (full diff), DirtyRegion, CellDelta
  - Only changed cells sent to terminal - massive reduction in I/O

- **Overlay System** - Composable UI overlays
  - Overlay trait with z-order compositing
  - OverlayCompositor for managing overlay stack
  - OverlayGeometry for positioning and bounds

- **Layer-Based Compositor** - Structured rendering pipeline
  - Layer trait with z-order constants (BASE=0, EDITOR=2, COMPLETION=4, etc.)
  - Implementations: BaseLayer, EditorLayer, ExplorerLayer, CompletionLayer, WhichKeyLayer, TelescopeLayer, LeapLayer, SettingsMenuLayer

- **Theme Background Colors** - All UI styles now have fg AND bg for proper diff-based rendering

### Architecture

- `FrameBuffer` - 2D cell storage with dirty tracking
- `Cell` - Individual screen cell (char, fg, bg, modifiers)
- `FrameRenderer` - Coordinates render strategies
- `RenderStrategy` trait - Pluggable diff algorithms
- `LayerCompositor` - Coordinates layer rendering to FrameBuffer
- `OverlayCompositor` - Manages overlay z-order

### Files Added

- `lib/core/src/frame/` - Frame buffer module
  - `mod.rs`, `buffer.rs`, `cell.rs`, `dirty.rs`, `renderer.rs`
  - `strategy/` - VirtualBuffer, DirtyRegion, CellDelta strategies
- `lib/core/src/overlay/` - Overlay system
  - `mod.rs`, `compositor.rs`, `geometry.rs`, `render.rs`, `selectable.rs`
- `lib/core/src/screen/layer.rs` - Layer trait and z-order constants
- `lib/core/src/screen/compositor.rs` - LayerCompositor
- `lib/core/src/screen/layers/` - Layer implementations
  - `base.rs`, `editor.rs`, `explorer.rs`, `completion.rs`
  - `which_key.rs`, `telescope.rs`, `leap.rs`, `settings_menu.rs`

### Files Changed

- `lib/core/src/screen/mod.rs` - Integration with layer compositor
- `lib/core/src/screen/window.rs` - render_to_buffer method
- `lib/core/src/runtime/core.rs` - FrameRenderer initialization
- `lib/core/src/runtime/event_loop.rs` - RenderSignal handling
- `lib/core/src/highlight/theme.rs` - Background colors for all styles

### Technical Changes

- Screen rendering now goes through LayerCompositor → FrameBuffer → FrameRenderer → Terminal
- Eliminated full-screen clear on every render
- Added `*_to_buffer` methods to all renderable components

### Testing

- 256 unit tests passing (includes frame/overlay tests)
- 95 integration tests passing
- Zero warnings (build + clippy)

---

## [0.5.3] - 2025-12-18

### Features

- **Which-Key Enhancements**
  - RPC endpoint `state/whichkey` for querying which-key popup state
  - Keybinding grouping - bindings now categorized (motion, operator, mode, edit, jump, fold, telescope, window, misc)
  - Grouped rendering with section headers in which-key popup

- **Explorer File Operations**
  - Copy/Cut/Paste: `yy` (yank), `dd` (cut), `p` (paste), `D` (delete)
  - Multi-file selection: `v` (visual mode), `V` (select all)
  - Selection marker (`*`) displayed for selected items
  - Visual selection updates as cursor moves with j/k

- **Explorer Display Enhancements**
  - Human-readable file sizes: `S` toggles size column (1.2K, 3.4M, etc.)
  - Broken symlink detection: shows `! ` icon for broken links

- **Telescope Enhancements**
  - RPC endpoint `state/telescope` for querying telescope state
  - Fixed escape race condition in telescope close handling
  - Fixed buffer picker returning empty items

### Bug Fixes

- **`cw` operator enters insert mode** - Fixed race condition where change operator stayed in normal mode instead of entering insert mode after deletion
- **Explorer cursor position with tabs** - Cursor now correctly positioned when tab line is visible
- **Explorer root protection** - Prevents accidental delete/rename of root directory
- **Explorer cursor bounds** - Cursor now validated after tree modifications
- **Telescope escape handling** - Fixed race condition causing stuck mode

### Tests

- New `lib/core/tests/which_key.rs` - Which-key visibility and binding tests
- New `lib/core/tests/telescope.rs` - Telescope picker integration tests

### Technical Changes

- Added `group` field to `WhichKeyBinding` and `KeyMapInner` structs
- Added `ExplorerClipboard` and `ExplorerSelection` structures
- Added `format_size()` for human-readable byte formatting
- Added `WhichKeySnapshot` and `TelescopeSnapshot` RPC types

## [0.5.2] - 2025-12-18

### Bug Fixes

- **`e` motion implemented** - Word end motion now works (`e` moves to end of current/next word)
- **Count prefix for w/b/e motions** - `2w`, `3b`, `2e` now work correctly
- **Cross-line word motions** - `w` and `b` now cross line boundaries
- **dw/cw off-by-one fixed** - Motion inclusivity corrected; `dw` no longer deletes first char of next word
- **dd+p register populated** - Linewise delete now stores text in register for paste
- **V (visual line mode) implemented** - `V` enters visual line mode
- **Visual selection initialization** - Visual mode now properly initializes selection
- **Undo batching per session** - Insert mode edits are undone as a single unit
- **`:` bound in visual mode** - Can now enter command mode from visual mode
- **Leap auto-jump** - Single match automatically jumps to target

### Technical Changes

- Add `Motion::WordEnd` variant and `is_inclusive()` method to Motion enum
- Add `begin_batch()`/`flush_batch()` methods for undo grouping in Buffer
- Implement cross-line calculations in `word_forward_calc()` and `word_backward_calc()`
- Add `ModExtension::Line` variant for visual line mode
- Fix `cw` to use `WordEnd` motion (vim behavior: `cw` acts like `ce`)

### Tests

- Updated test suite from documenting bugs to expecting correct behavior
- All 332 tests passing

## [0.5.1] - 2025-12-18

### Features

- **Comprehensive Integration Test Suite** - 95 new server-based E2E tests
  - `word_motions.rs` (20 tests) - w, b motion tests
  - `operators.rs` (21 tests) - dd, x, yy, p, dw, cw, dj, dk, d$ tests
  - `undo_redo.rs` (11 tests) - u and Ctrl-R tests
  - `visual_mode.rs` (10 tests) - v, V, Ctrl-V mode tests
  - `command_mode.rs` (10 tests) - : command and :set tests
  - `cursor_advanced.rs` (14 tests) - gg, G, 0, $, count prefix tests
  - `leap.rs` (9 tests) - s, S leap navigation tests

### Documentation

- Tests document actual behavior vs expected vim behavior for gaps:
  - `e` motion not implemented
  - Count prefix not supported for w/b motions
  - w/b motions don't cross lines
  - dw/cw off-by-one bug (deletes first char of next word)
  - Undo is per-character, not per-insert-session
  - V (visual line mode) not implemented
  - Leap jump doesn't move cursor to match position

### Files Added

- `lib/core/tests/word_motions.rs`
- `lib/core/tests/operators.rs`
- `lib/core/tests/undo_redo.rs`
- `lib/core/tests/visual_mode.rs`
- `lib/core/tests/command_mode.rs`
- `lib/core/tests/cursor_advanced.rs`
- `lib/core/tests/leap.rs`

## [0.5.0] - 2025-12-18

### Features

- **Server Mode with JSON-RPC 2.0 Protocol** - Headless server for programmatic editor control
  - `--server` flag starts persistent server (runs until killed)
  - `--server --test` starts test server (exits when all clients disconnect)
  - Headless operation: no terminal required, captures output for RPC responses

- **Multiple Transport Options** - Flexible connectivity
  - `--stdio` - Stdio transport (stdin/stdout)
  - `--listen-tcp <port>` - TCP transport on specified port
  - `--listen-socket <path>` - Unix socket transport
  - Default TCP port: 12521 ('r'×100 + 'e'×10 + 'o')

- **RPC API Methods** - Full editor control via JSON-RPC 2.0
  - `input/keys` - Send key sequences (e.g., `{"keys": "iHello<Esc>"}`)
  - `state/mode` - Get current mode state
  - `state/cursor` - Get cursor position (x, y)
  - `state/buffer` - Get buffer content
  - `state/screen` - Get full screen state (cells, cursor, mode, dimensions)
  - `editor/quit` - Request editor quit

- **reo-cli Client Tool** - Command-line client for server interaction
  - `reo-cli mode` - Query current mode
  - `reo-cli cursor` - Query cursor position
  - `reo-cli buffer` - Query buffer content
  - `reo-cli screen` - Query screen state
  - `reo-cli keys '<keys>'` - Send key sequence
  - `reo-cli quit` - Request server quit
  - `reo-cli repl` - Interactive REPL mode
  - Configurable host/port: `--host`, `--port`

- **Hybrid Line Numbers by Default** - Line numbers now show hybrid mode (absolute for current line, relative for others) on startup

- **Server-Based Integration Testing** - E2E test infrastructure using real server
  - `ServerTestHarness` - Spawns test server with auto port allocation (ports 12600-12699)
  - `TestClient` - Async JSON-RPC client for test communication
  - `ServerTest` - Fluent builder API: `.with_content()`, `.with_keys()`, `.run()`
  - `ServerTestResult` - Assertion methods: `assert_normal_mode()`, `assert_cursor()`, `assert_buffer_contains()`
  - 1ms delay between key injections for reliable mode propagation
  - Replaces mock-based `TestRuntime` with real end-to-end testing

### Architecture

- `RpcServer` - Coordinates JSON-RPC request handling
- `TransportConfig` - Transport selection: Stdio, UnixSocket, Tcp
- `TransportReader`/`TransportWriter` - Async I/O abstraction
- `TransportListener` - Accepts connections for socket/TCP
- `ChannelKeySource` - Injects keys from RPC into runtime
- `DualOutput` - Captures screen output for headless mode
- `ServerConfig` - Server behavior configuration (headless, test mode)

### Files Added

- `lib/core/src/rpc/mod.rs` - RPC module root
- `lib/core/src/rpc/server.rs` - RpcServer implementation
- `lib/core/src/rpc/state.rs` - Screen state serialization
- `lib/core/src/rpc/transport.rs` - Transport abstractions
- `lib/core/src/rpc/types.rs` - JSON-RPC message types
- `lib/core/src/testing/client.rs` - TestClient for JSON-RPC communication
- `lib/core/src/testing/server.rs` - ServerTestHarness for spawning test servers
- `lib/core/src/testing/assertions.rs` - ServerTest builder and ServerTestResult assertions
- `main/src/server.rs` - Server mode entry point
- `tools/reo-cli/` - CLI client tool (Cargo.toml, src/main.rs, client.rs, commands.rs, repl.rs)

### Files Changed

- `Cargo.toml` - Added reo-cli workspace member, serde_json dependency
- `lib/core/Cargo.toml` - Added serde_json dependency
- `lib/core/src/lib.rs` - Added rpc, testing module exports
- `lib/core/src/io/output.rs` - Added DualOutput for screen capture
- `lib/core/src/event/inner/mod.rs` - Added RpcRequestEvent
- `lib/core/src/runtime/core.rs` - Added RPC request handling
- `lib/core/src/runtime/event_loop.rs` - Added RPC event handling
- `lib/core/src/runtime/mod.rs` - Removed TestRuntime exports
- `lib/core/src/testing/mod.rs` - Added exports for server-based testing
- `lib/core/tests/common/mod.rs` - Switched to ServerTest
- `lib/core/tests/basic_editing.rs` - Rewritten to use ServerTest
- `lib/core/tests/mode_switching.rs` - Rewritten to use ServerTest
- `main/src/main.rs` - Added server mode CLI flags
- `docs/TESTING.md` - Updated for server-based testing architecture

### Files Removed

- `lib/core/src/runtime/test.rs` - Old mock-based TestRuntime (replaced by server-based testing)

### Testing

- 213 unit tests passing
- 23 integration tests passing (basic_editing: 10, mode_switching: 8, resize: 5)
- Zero warnings (build + clippy)
- Performance: No regression in core benchmarks

---

## [0.4.18] - 2025-12-18

### Features

- **TOML-based configuration profile system** - Save, load, and switch editor configurations at runtime
  - Profiles stored at `~/.config/reovim/profiles/` (XDG Base Directory compliant)
  - Ex-commands: `:profile load <name>`, `:profile save <name>`, `:profile list`
  - Telescope picker for profile selection (via `:profile list`)
  - CLI flag: `--profile/-p <name>` to start with a specific profile
  - First-run auto-creates default profile
  - Profiles control: theme, colormode, line numbers, relative numbers, indent guides, scrollbar, tab width

- **TUI Settings Menu** - Interactive settings editor with live preview
  - Open with `Space s` or `:settings` command
  - Vim-style navigation: `j/k` to navigate, `Space` to toggle, `h/l` to cycle choices
  - Quick select choices with number keys `1-9`
  - Number settings adjustable with `+/-`
  - Immediate apply: changes take effect instantly
  - Settings organized in sections: Editor, Cursor, Window, Profile
  - Checkbox-style booleans, dropdown-style choices, number inputs

### Files Added

- `lib/core/src/config/mod.rs` - Config module root
- `lib/core/src/config/schema.rs` - TOML structs with serde
- `lib/core/src/config/loader.rs` - XDG paths, load/save utilities
- `lib/core/src/config/profile.rs` - ProfileManager struct
- `lib/core/src/telescope/picker/profiles.rs` - Profiles picker
- `lib/core/src/settings_menu/mod.rs` - Settings menu module root
- `lib/core/src/settings_menu/item.rs` - SettingItem, SettingValue, SettingSection types
- `lib/core/src/settings_menu/state.rs` - SettingsMenuState with navigation/manipulation
- `lib/core/src/settings_menu/render.rs` - TUI rendering with box drawing
- `lib/core/src/command/builtin/settings_menu.rs` - Settings menu commands

### Files Changed

- `lib/core/Cargo.toml` - Added toml, serde dependencies
- `lib/core/src/lib.rs` - Added config and settings_menu module exports
- `lib/core/src/command_line/ex_command.rs` - Added ProfileLoad/Save/List/Settings commands
- `lib/core/src/runtime/core.rs` - Added ProfileManager, settings_menu field
- `lib/core/src/runtime/event_loop.rs` - Added profile and settings menu event handling
- `lib/core/src/runtime/handlers.rs` - Added profile and settings menu handlers
- `lib/core/src/telescope/item.rs` - Added TelescopeData::Profile variant
- `lib/core/src/telescope/picker/mod.rs` - Added TelescopeAction::SwitchProfile, ProfilesPicker
- `lib/core/src/event/inner/mod.rs` - Added SettingsMenuEvent enum
- `lib/core/src/event/mod.rs` - Export SettingsMenuEvent
- `lib/core/src/modd/mod.rs` - Added Focus::SettingsMenu variant
- `lib/core/src/bind/mod.rs` - Added settings_menu keymap and Space-s binding
- `lib/core/src/command/id.rs` - Added settings menu command IDs
- `lib/core/src/command/traits.rs` - Added SettingsMenuAction, DeferredAction::SettingsMenu
- `lib/core/src/command/builtin/mod.rs` - Export settings_menu commands
- `lib/core/src/command/registry.rs` - Register settings menu commands
- `lib/core/src/screen/mod.rs` - Render settings menu overlay
- `lib/core/src/screen/status_line.rs` - Added SETTINGS icon
- `main/src/main.rs` - Added --profile/-p CLI flag

### Testing

- Zero warnings (build + clippy)
- All tests passing

### Deferred

- Runtime keybinding modification from profiles (Phase 5) - keybindings stored in TOML but not applied at runtime

---

## [0.4.17] - 2025-12-18

### Bug Fixes

- **Fixed integration test timing issues** - All 19 integration tests now pass reliably
  - Added `grace_period` to `InputEventBroker` for async event propagation
  - Added delay between keys in `MockKeySource` for mode state synchronization
  - Fixed Visual mode Escape handling to use keymap lookup instead of `is_esc_clearable_mode`

### Documentation

- **Added Integration Testing section to docs/TESTING.md** - Comprehensive guide for writing and running integration tests
  - Test architecture overview
  - `keys_from_str()` notation reference
  - `TestResult` assertion methods
  - Best practices and examples

### Testing

- All 19 integration tests now pass (was 9 passing, 10 ignored)
  - 10 basic editing tests (cursor movement, insert mode, open line)
  - 9 mode switching tests (enter/exit modes with Escape)

### Files Changed

- `lib/core/src/event/input.rs` - Added `grace_period` field and `with_grace_period()` builder method
- `lib/core/src/event/handler/command/mod.rs` - Fixed `is_esc_clearable_mode` to exclude Visual mode
- `lib/core/src/runtime/test.rs` - Use 10ms delay between keys and 50ms grace period
- `lib/core/tests/basic_editing.rs` - Removed `#[ignore]` from 6 tests, fixed `test_insert_at_beginning`
- `lib/core/tests/mode_switching.rs` - Removed `#[ignore]` from 4 tests
- `docs/TESTING.md` - Added Integration Testing section

---

## [0.4.16] - 2025-12-18

### Bug Fixes

- **Fixed telescope display jitter** - Eliminated screen flickering during telescope navigation and typing by adding a dedicated `render_telescope_only()` path that skips full screen clear

### Files Changed

- `lib/core/src/screen/mod.rs` - Added `render_telescope_only()` method
- `lib/core/src/runtime/core.rs` - Added `render_telescope_only()` wrapper method
- `lib/core/src/runtime/event_loop.rs` - Use `render_telescope_only()` for telescope internal updates
- `lib/core/src/runtime/handlers.rs` - Use `render_telescope_only()` for telescope actions

### Testing

- Zero warnings (build + clippy)
- All 186 tests passing

---

## [0.4.15] - 2025-12-18

### Features

- **Terminal resize event handling** - Editor now properly responds to terminal resize events:
  - Screen dimensions update correctly when terminal is resized
  - Window layouts recalculate to fit new dimensions
  - Explorer sidebar width clamps correctly on small screens
  - Explorer rendering uses layout width instead of hardcoded width

### Bug Fixes

- **Fixed editor content bleeding into explorer column** - All explorer lines now padded to full width to prevent editor text from showing through gaps

### Files Changed

- `lib/core/src/event/inner/mod.rs` - Added `ScreenResizeEvent` variant
- `lib/core/src/event/input.rs` - Added `with_event_sender()` to InputEventBroker
- `lib/core/src/io/input.rs` - Added `with_event_sender()` to EventStreamKeySource for resize events
- `lib/core/src/screen/mod.rs` - Added `resize()` method and unit tests
- `lib/core/src/screen/layout.rs` - Added resize behavior tests
- `lib/core/src/explorer/render.rs` - Pass layout width, pad all lines to full width
- `lib/core/src/runtime/event_loop.rs` - Handle resize events

### Testing

- Zero warnings (build + clippy)
- All tests passing
- Added 5 new integration tests for resize behavior (`lib/core/tests/resize.rs`)

### Known Issues

- **Panel background bleeding** - The content bleeding through on resize. (Ex, Editor content to Explorer)
  - Each UI component (explorer, which-key, telescope, etc.) has its own rendering system
  - Require the common-unified method for managing window and buffer.

---

## [0.4.14] - 2025-12-18

### Features

- **Integration test system with mock I/O layer** - End-to-end testing capability that verifies key input → expected output without requiring a real terminal
  - `KeySource` trait abstraction for terminal input (MockKeySource, ChannelKeySource, EventStreamKeySource)
  - `MockOutput` for capturing screen render output
  - `TestRuntime` with builder pattern for integration tests
  - `keys_from_str()` helper for readable key sequences (e.g., "ihello<Esc>")
  - Generic `InputEventBroker<K: KeySource>` to support mock input injection

### Testing

- Added integration tests for cursor movement (j, jj, l, hjkl)
- Added integration tests for mode transitions (i, a, v, :, :q<CR>)
- 9 integration tests passing, 10 ignored (timing issues documented for future work)

### Files Added

- `lib/core/src/io/mod.rs` - I/O abstraction module
- `lib/core/src/io/input.rs` - KeySource trait and implementations
- `lib/core/src/io/output.rs` - MockOutput for capturing display
- `lib/core/src/testing/mod.rs` - Testing utilities module
- `lib/core/src/testing/keys.rs` - Key event helpers
- `lib/core/src/runtime/test.rs` - TestRuntime and TestResult
- `lib/core/tests/basic_editing.rs` - Cursor movement tests
- `lib/core/tests/mode_switching.rs` - Mode transition tests
- `lib/core/tests/common/mod.rs` - Shared test utilities

### Files Changed

- `lib/core/Cargo.toml` - Added async-trait dependency
- `lib/core/src/lib.rs` - Added io, testing module exports
- `lib/core/src/runtime/event_loop.rs` - Made handle_event pub(crate)
- `lib/core/src/event/input.rs` - Made InputEventBroker generic over KeySource
- `lib/core/src/runtime/mod.rs` - Added test module export

---

## [0.4.13] - 2025-12-18

### Bug Fixes

- **Fixed treesitter theme not syncing with UI theme** - TreesitterTheme was hardcoded to default, ignoring `:colorscheme` changes. Now syncs with UI theme.
- **Fixed telescope keymaps picker showing empty results** - KeymapsPicker now populates from KeyMap::with_defaults()

### Features

- **Added telescope theme picker** - New `telescope_themes` command to select colorschemes visually with preview
- **Theme changes now rehighlight all buffers** - Switching themes immediately updates syntax highlighting

### Files Changed

- `lib/core/src/treesitter/theme.rs` - Added `from_theme_name()` method
- `lib/core/src/treesitter/highlighter.rs` - Added `set_theme()` method
- `lib/core/src/treesitter/mod.rs` - Added `set_theme()` method
- `lib/core/src/runtime/handlers.rs` - Updated `:colorscheme` to sync treesitter theme
- `lib/core/src/runtime/core.rs` - Added `rehighlight_all_buffers()`, registered ThemesPicker
- `lib/core/src/runtime/event_loop.rs` - Handle theme selection from telescope
- `lib/core/src/telescope/item.rs` - Added `Theme(ThemeName)` variant
- `lib/core/src/telescope/picker/mod.rs` - Added `ApplyTheme` action, themes module
- `lib/core/src/telescope/picker/themes.rs` - New ThemesPicker implementation
- `lib/core/src/command/builtin/telescope.rs` - Added `TelescopeThemesCommand`
- `lib/core/src/command/registry.rs` - Registered `TelescopeThemesCommand`
- `lib/core/src/telescope/picker/keymaps.rs` - Populate keymaps from `KeyMap::with_defaults()`

---

## [0.4.12] - 2025-12-18

### Bug Fixes

- **Fixed window navigation commands not working** - Window and tab commands were defined but never registered in CommandRegistry
- **Fixed C-hjkl not working in explorer mode** - Added window navigation keybindings to explorer mode
- **Fixed mode not syncing when navigating between explorer and editor** - Screen is now source of truth for focus, mode syncs after window actions

### Features

- **Added Space e toggle to explorer mode** - Can now toggle explorer from within explorer
- **Explorer included in window navigation** - C-l from explorer moves to editor, C-h from editor moves to explorer

### Removed

- Removed `q` binding from explorer (use C-l or Escape instead)

### Files Changed

- `lib/core/src/command/registry.rs` - Register window/tab commands
- `lib/core/src/bind/mod.rs` - Add explorer keybindings (C-hjkl, Space e)
- `lib/core/src/runtime/handlers.rs` - Add sync_mode_with_screen_focus()
- `lib/core/src/screen/mod.rs` - navigate_window() includes explorer as virtual window
- `lib/core/src/screen/split.rs` - Add EXPLORER_WINDOW_ID constant

---

## [0.4.11] - 2025-12-18

### Refactoring

- **Fixed all clippy cognitive complexity warnings** - Refactored 3 functions to meet complexity threshold (25):
  - `run()` in command handler: 63→<25 (extracted 8 helper methods)
  - `handle_explorer_action()` in runtime handlers: 31→<25 (extracted 5 helper methods)
  - `render()` in window: 30→<25 (extracted 5 helper methods)

### Files Changed

- `lib/core/src/event/handler/command/mod.rs` - Cognitive complexity refactoring
- `lib/core/src/runtime/handlers.rs` - Cognitive complexity refactoring
- `lib/core/src/screen/window.rs` - Cognitive complexity refactoring

### Testing

- Zero warnings (build + clippy)
- All tests passing

---

## [0.4.10] - 2025-12-18

### Bug Fixes

- **Fixed dd/yy/cc operators not working on real files**:
  - Race condition fix: Prevent `local_mode` from being overwritten when already in operator-pending mode
  - Explicit dereference in match pattern for operator type
  - Count=0 handling: `dd` now correctly deletes only the current line (was deleting 2 lines)
  - Buffer ID fix: Operators now use active buffer instead of hardcoded buffer 0
  - Paste fix: `p` command now uses active buffer instead of hardcoded buffer 0

- **Fixed which-key showing "E" instead of proper hints in operator-pending mode**:
  - Added `hint` field to `KeyMapInner` for hint-only entries
  - Operator-pending keymap now shows proper descriptions (e.g., "delete line (dd)")

### Files Changed

- `lib/core/src/bind/mod.rs` - Hint system for operator-pending mode
- `lib/core/src/buffer/mod.rs` - Count=0 fix for dd/yy
- `lib/core/src/event/handler/command/mod.rs` - Race condition fix
- `lib/core/src/runtime/handlers.rs` - Buffer ID and paste fixes

### Testing

- **159 tests passing**
- Zero warnings

---

## [0.4.9] - 2025-12-18

### Bug Fixes

- **Fixed Rust treesitter syntax highlighting** - Query syntax updated to match tree-sitter-rust v0.24:
  - Keywords `crate`, `self`, `super` now use named node syntax instead of string literals
  - Field-based patterns (`function:`, `body:`, `macro:`) properly aligned with grammar
  - Added error logging for query compilation failures
  - Added unit tests for query validation (4 new tests)

### Files Changed

- `lib/core/src/treesitter/queries.rs` - Error logging and tests
- `lib/core/src/treesitter/queries/rust/highlights.scm` - Rewritten for v0.24
- `lib/core/src/treesitter/queries/rust/textobjects.scm` - Fixed field syntax
- `lib/core/src/treesitter/queries/rust/folds.scm` - Fixed field syntax

### Testing

- **159 tests passing** (4 new treesitter query tests)
- Zero warnings

---

## [0.4.8] - 2025-12-18

### New Features

#### Unified Theme System with Sub-Structs
- **Restructured Theme with logical sub-structs** for better maintainability:
  - `base`: Default and cursor line styles
  - `gutter`: Line numbers, sign column
  - `selection`: Visual selection styles
  - `statusline`: Status line with mode-specific styles
  - `popup`: Completion popup styles
  - `telescope`: Fuzzy finder styles
  - `whichkey`: Which-key panel styles
  - `leap`: Jump navigation styles
  - `fold`: Code folding styles
  - `indent`: Indentation guide styles
  - `scrollbar`: Scrollbar with diagnostic marks
  - `search`: Search highlight styles
  - `tab`: Tab line styles
  - `window`: Window separator styles

- **ThemeName enum** with three built-in themes:
  - `Dark` - OneDark-inspired dark theme (default)
  - `Light` - Light theme for bright environments
  - `TokyoNightOrange` - Tokyo Night with orange accents

- **`:colorscheme` command** for runtime theme switching:
  - `:colorscheme dark` / `:colo dark`
  - `:colorscheme light` / `:colo light`
  - `:colorscheme tokyonight` / `:colo tokyonight`

- **TreesitterTheme::tokyo_night_orange()** - Coordinated syntax highlighting:
  - Comments: Light gray, italic
  - Keywords: Purple, bold, italic
  - Functions: Blue
  - Strings: Green
  - Types: Cyan

#### UI Enhancements
- **Enhanced statusline** with more information:
  - Mode icons: `` (normal), `` (insert), `` (visual), `` (command)
  - Position display: `line:col`
  - Filetype indicator with icon
  - Modified indicator `[+]`

- **Indent guides integration** - Visual vertical lines at indent levels:
  - `:set indentguide` / `:set ig` - Enable
  - `:set noindentguide` / `:set noig` - Disable
  - Uses `theme.indent.guide` and `theme.indent.active` styles

- **Scrollbar rendering** - Visual scroll position indicator:
  - `:set scrollbar` / `:set sb` - Enable
  - `:set noscrollbar` / `:set nosb` - Disable
  - Track character: `▕`, Thumb character: `█`
  - Uses `theme.scrollbar.track` and `theme.scrollbar.thumb` styles

#### Textobject Improvements
- **Word textobjects** for operators (delete, yank, change):
  - `iw` - Inner word (alphanumeric + underscore)
  - `aw` - Around word (includes trailing/leading whitespace)
  - `iW` - Inner WORD (non-whitespace)
  - `aW` - Around WORD (includes trailing/leading whitespace)
  - Examples: `diw`, `yaw`, `ciW`, `daW`

- **Visual mode textobject selection**:
  - `viw`, `vaw`, `viW`, `vaW` - Select word textobjects
  - `vi(`, `va{`, `vi"` - Select delimiter textobjects
  - `vif`, `vac` - Select semantic textobjects (treesitter)

- **VisualTextObjectAction event** for visual mode integration

### New Commands

| Command | Description |
|---------|-------------|
| `:colorscheme <name>` | Switch theme (dark, light, tokyonight) |
| `:colo <name>` | Short form of colorscheme |
| `:set indentguide` | Enable indent guides |
| `:set noindentguide` | Disable indent guides |
| `:set scrollbar` | Enable scrollbar |
| `:set noscrollbar` | Disable scrollbar |

### New Textobjects

| Textobject | Description |
|------------|-------------|
| `iw` / `aw` | Inner/around word |
| `iW` / `aW` | Inner/around WORD |

### Architecture

- `ThemeName` enum for theme identification
- `Theme::from_name()` for runtime theme creation
- Sub-struct organization: `BaseStyles`, `GutterStyles`, `SelectionStyles`, `StatusLineStyles`, `PopupStyles`, `TelescopeStyles`, `WhichKeyStyles`, `LeapStyles`, `FoldStyles`, `IndentStyles`, `ScrollbarStyles`, `SearchStyles`, `TabStyles`, `WindowStyles`
- `ScrollbarState` for scrollbar position calculation
- `VisualTextObjectAction` event type
- `WordTextObject` enum (Word, BigWord)

### Files Modified

- `lib/core/src/highlight/theme.rs` - Complete restructure with sub-structs
- `lib/core/src/highlight/mod.rs` - Export ThemeName
- `lib/core/src/treesitter/theme.rs` - Tokyo Night Orange syntax theme
- `lib/core/src/command_line/ex_command.rs` - New SetOption variants
- `lib/core/src/screen/window.rs` - Indent guides, scrollbar rendering
- `lib/core/src/screen/mod.rs` - Theme sub-struct access
- `lib/core/src/screen/status_line.rs` - Enhanced statusline
- `lib/core/src/textobject.rs` - Word textobject types
- `lib/core/src/buffer/mod.rs` - Word textobject methods
- `lib/core/src/event/inner/mod.rs` - VisualTextObjectAction
- `lib/core/src/runtime/handlers.rs` - New command handlers

---

## [0.4.7] - 2025-12-18

### New Features

#### Window Splits and Tab Pages
- **Vim-style window splits** - Split editor into multiple panes
  - `:sp [file]` / `:split [file]` - Horizontal split (one above the other)
  - `:vs [file]` / `:vsplit [file]` - Vertical split (side by side)
  - `:close` / `:clo` - Close current window
  - `:only` / `:on` - Close all windows except current

- **Window navigation** - Move focus between splits
  - `Ctrl-h` - Focus window to the left
  - `Ctrl-j` - Focus window below
  - `Ctrl-k` - Focus window above
  - `Ctrl-l` - Focus window to the right

- **Window movement** - Reposition windows in layout
  - `Ctrl-Shift-H` - Move window left
  - `Ctrl-Shift-J` - Move window down
  - `Ctrl-Shift-K` - Move window up
  - `Ctrl-Shift-L` - Move window right

- **Tab pages** - Multiple editor layouts
  - `:tabnew [file]` / `:tabe [file]` - Create new tab
  - `:tabclose` / `:tabc` - Close current tab
  - `:tabnext` / `:tabn` - Switch to next tab
  - `:tabprev` / `:tabp` - Switch to previous tab
  - `gt` - Next tab
  - `gT` - Previous tab

- **Tab line rendering** - Visual tab indicator when multiple tabs exist
- **Window separators** - Visual borders between split windows

### Architecture

- `SplitNode` - Binary tree for recursive window layouts
- `TabPage` / `TabManager` - Tab page management
- `WindowAction` / `TabAction` - Deferred action enums

### New Files

- `lib/core/src/screen/split.rs` - Split tree and layout calculation
- `lib/core/src/screen/tab.rs` - Tab page and manager
- `lib/core/src/command/builtin/window.rs` - Window commands
- `lib/core/src/command/builtin/tab.rs` - Tab commands

### Testing

- **152 tests passing**
- Zero warnings

---

## [0.4.6] - 2025-12-18

### Features

#### Unified Fast Benchmarking
- **New `bench` subcommand for perf-report** - Single command workflow:
  - Clears old benchmark data (`target/criterion/`)
  - Runs all benchmarks (`cargo bench -p reovim-core`)
  - Generates performance report (`perf/PERF-{version}.md`)

- **Faster benchmark execution** - Reduced from ~8 minutes to ~2 minutes:
  - measurement_time: 5s → 1s
  - warm_up_time: 3s → 200ms
  - sample_size: 100 → 30

- **`cargo run` defaults to reovim** - Added `default-members = ["main/"]` to workspace

### Usage

```bash
# Run benchmarks + generate report (unified)
cargo run -p perf-report -- bench -v X.Y.Z

# Run reovim directly
cargo run
```

---

## [0.4.5] - 2025-12-18

### Bug Fixes

#### Viewport Scrolling
- **Fixed cursor going off-screen when moving beyond viewport**
  - Root cause: `buffer_anchor` (scroll offset) was never updated when cursor moved
  - Added `Window::update_scroll()` method to keep cursor visible within viewport
  - Fixed cursor position calculation to account for scroll offset

### Technical Details

**Files modified:**
- `lib/core/src/screen/window.rs` - Added `update_scroll()` method
- `lib/core/src/screen/mod.rs` - Call `update_scroll()` before render, fix cursor_y calculation

**Implementation:**
```rust
// New method in Window
pub const fn update_scroll(&mut self, cursor_y: u16) {
    // Scroll up if cursor is above visible area
    if cursor_y < self.buffer_anchor.y {
        self.buffer_anchor.y = cursor_y;
    }
    // Scroll down if cursor is below visible area
    else if cursor_y >= self.buffer_anchor.y + self.height {
        self.buffer_anchor.y = cursor_y.saturating_sub(self.height) + 1;
    }
}

// Fixed cursor position calculation
let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
```

---

## [0.4.4] - 2025-12-17

### Performance Optimization

#### Zero-Copy Render Path
- **Eliminated buffer cloning in render loop** - Major optimization
  - Changed `Screen::render()` to accept `&BTreeMap<usize, Buffer>` instead of `&[Buffer]`
  - Removed costly `buffers.values().cloned().collect()` from every render call
  - Previous cost: 364µs (10K lines) to 1.96ms (50K lines) per render
  - Now: Zero allocation - only a reference is passed

### Technical Details

**Files modified:**
- `lib/core/src/screen/mod.rs` - Changed render signature to accept BTreeMap reference
- `lib/core/src/runtime/core.rs` - Removed buffer cloning, pass reference directly

**API Change:**
```rust
// Before:
pub fn render(&mut self, buffers: &[Buffer], ...);
let buffers: Vec<Buffer> = self.buffers.values().cloned().collect();
screen.render(&buffers, ...);

// After:
pub fn render(&mut self, buffers: &BTreeMap<usize, Buffer>, ...);
screen.render(&self.buffers, ...);
```

### Performance Results (v0.4.2 → v0.4.4)

| Benchmark | v0.4.2 | v0.4.4 | Change |
|-----------|--------|--------|--------|
| window_render/10 | 639 ns | 473 ns | **26% faster** |
| rtt/char_insert | - | 28 µs | baseline |
| rtt/move_down | 397 µs | 383 µs | **4% faster** |
| rtt/half_page_down | 410 µs | 390 µs | **5% faster** |
| rtt/goto_top | 397 µs | 390 µs | **2% faster** |
| input_mode_switch | - | 18 µs | baseline |
| stress_editing/50k | 38.99 ms | 37.60 ms | **4% faster** |

### Key Metrics (v0.4.4)

- **Window render**: 473ns (10 lines) - 2.6µs (10K lines)
- **Input RTT**: 28µs (char insert), 45µs (word motion)
- **Movement RTT**: 383-390µs (vertical), 45µs (horizontal)
- **Mode switch**: 18µs (Normal→Insert→Normal cycle)
- **Throughput**: ~400k renders/sec

---

## [0.4.3] - 2025-12-17

### New Features

#### Treesitter Integration
- **Syntax highlighting** powered by tree-sitter for accurate parsing
  - Supported languages: Rust, C, JavaScript, Python, JSON, TOML, Markdown
  - Incremental parsing with 50ms debounce for efficient re-highlighting
  - Theme-aware capture mapping (keyword, function, type, string, comment, etc.)

- **Code folding** with treesitter queries
  - `za` - Toggle fold at cursor
  - `zo` - Open fold at cursor
  - `zc` - Close fold at cursor
  - `zR` - Open all folds in buffer
  - `zM` - Close all folds in buffer
  - Fold markers show line count and preview text

- **Semantic text objects** (treesitter-based)
  - `af` / `if` - Around/inner function
  - `ac` / `ic` - Around/inner class/struct
  - Works with operators: `daf`, `yif`, `cic`, etc.

- **Indentation guides** (theme support added)
  - `indent_guide` and `indent_guide_active` theme styles

### Technical Details

**New modules:**
- `lib/core/src/treesitter/` - TreesitterManager, BufferParser, Highlighter
- `lib/core/src/folding.rs` - FoldManager, FoldState, FoldRange

**Dependencies added:**
- tree-sitter = "0.24"
- tree-sitter-rust, tree-sitter-c, tree-sitter-javascript, tree-sitter-python
- tree-sitter-json, tree-sitter-toml-ng, tree-sitter-md

### Testing

- **133 tests passing** (up from 118)
- New tests for folding operations

---

## [0.4.2] - 2025-12-17

### New Features

#### Performance Benchmarking Infrastructure
- **Criterion benchmark suite** - Comprehensive performance testing
  - Window render benchmarks (various buffer sizes)
  - Screen I/O benchmarks (buffered vs unbuffered)
  - Input simulation benchmarks (typing, scrolling, mode switching)
  - RTT (Round-Trip Time) benchmarks for latency measurement
  - Stress tests for worst-case scenarios
  - Location: `lib/core/benches/`

- **perf-report CLI tool** - Performance data management
  - `update --version X.Y.Z` - Generate versioned performance reports
  - `list` - Show current benchmark results
  - `check` - CI regression detection
  - `compare` - Diff between versions
  - Location: `tools/perf-report/`

- **Versioned performance reports** - Git-tracked benchmark data
  - Markdown + TOML format for human and machine readability
  - Full statistics: mean, median, std_dev, confidence intervals
  - Metadata: commit, date, Rust version, OS
  - Location: `perf/PERF-{version}.md`

### Benchmark Categories

| Category | Description |
|----------|-------------|
| window_render | Window::render() performance |
| screen_io | Full screen I/O with real files |
| input_* | Typing, scrolling, mode switching |
| rtt_* | Input lag, movement lag, explorer toggle |
| stress_* | Combined editing, rapid scroll, worst case |
| buffer_clone | Buffer clone overhead (bottleneck identified) |

### Performance Results (v0.3.0 → v0.4.2)

| Benchmark | v0.3.0 | v0.4.2 | Improvement |
|-----------|--------|--------|-------------|
| window_render/10 | 1.67 µs | 639 ns | **62% faster** |
| window_render/10000 | 6.60 µs | 2.48 µs | **62% faster** |
| viewport_size/24 | 3.76 µs | 1.20 µs | **68% faster** |
| viewport_size/200 | 33.41 µs | 9.73 µs | **71% faster** |
| screen_io/full_render | 27.68 µs | 5.91 µs | **79% faster** |
| file_io/buffered | 61.56 µs | 12.68 µs | **79% faster** |
| rtt/move_down | 1.35 ms | 397 µs | **71% faster** |
| rtt/half_page_down | 4.32 ms | 410 µs | **91% faster** |
| rtt/goto_top | 3.61 ms | 397 µs | **89% faster** |
| stress_editing/50k | 111.77 ms | 38.99 ms | **65% faster** |
| buffer_clone/50k | 3.90 ms | 1.96 ms | **50% faster** |
| throughput | 8.61 µs | 2.49 µs | **71% faster** |

### Key Findings

- **Average 50-80% improvement** across all benchmarks
- **Movement RTT improved 60-90%** - dramatically better responsiveness
- Window render: ~639ns-3µs (viewport-limited)
- Buffer clone: Reduced from 3.9ms to 1.96ms for 50K lines
- Buffered I/O: 10x faster than unbuffered

---

## [0.4.1] - 2025-12-17

### New Features

#### Leap Motion
- **Two-character jump navigation** - Quick cursor movement by typing 2 characters
  - `s` - Leap forward
  - `S` - Leap backward
- LeapState tracking and LeapEvent system
- Integrates with operator-pending mode (e.g., `ds{char}{char}` to delete to target)
- Location: `lib/core/src/leap/`

### Documentation

- **Full documentation rewrite** - Updated all docs to reflect actual codebase state
- Updated architecture.md with Runtime fields and feature modules
- Updated event-system.md with all InnerEvent variants
- Complete rewrite of commands.md with trait-based architecture

---

## Features Present Since v0.3.0

The following features were implemented but not documented in earlier changelogs:

### CommandTrait System
- Trait-based command architecture replacing simple enum
- `CommandTrait` interface with `execute()`, `name()`, `description()`
- `CommandRegistry` - Thread-safe command lookup with `Arc<dyn CommandTrait>`
- `ExecutionContext` - Execution parameters (buffer, count, ids)
- `DeferredAction` - Actions requiring Runtime access
- 129 registered CommandIds, ~79 implementations
- Location: `lib/core/src/command/`

### Telescope Fuzzy Finder
- Fuzzy file/buffer/grep search powered by nucleo
- 7 built-in pickers: files, buffers, live_grep, recent, commands, help, keymaps
- Normal and Insert modes for navigation/typing
- `Space f` prefix keybindings
- Location: `lib/core/src/telescope/`

### Explorer File Browser
- Tree-view file browser with expand/collapse
- 25 commands for navigation, file operations, filtering
- Create/rename/delete files and directories
- Hidden file toggle, filter mode
- `Space e` to toggle
- Location: `lib/core/src/explorer/`

### Completion Engine
- Async word completion
- Triggered with Ctrl-Space
- Ctrl-n/p for navigation, Tab to confirm
- Location: `lib/core/src/completion/`

### Which-Key Panel
- Popup showing available keybindings
- Appears after prefix keys (e.g., `g`, `Space`)
- Location: `lib/core/src/screen/which_key.rs`

### Jump List
- Ctrl-O/Ctrl-I navigation between jump locations
- Tracks cursor positions for jump commands
- Location: `lib/core/src/jump_list/`

### Operators with Motions
- `d` + motion = delete
- `y` + motion = yank
- `c` + motion = change
- Operator-pending mode with count support
- Location: `lib/core/src/command/builtin/operator.rs`

### Registers System
- Multi-register copy/paste storage
- Default register and named registers
- Location: `lib/core/src/registers/`

### Theme System
- Color mode detection (ANSI, 256, TrueColor)
- Themed styling for UI components
- Location: `lib/core/src/theme/`

### Undo/Redo
- `u` to undo, `Ctrl-r` to redo
- Per-buffer history

---

## [0.4.0] - 2025-12-17

### New Features

#### Multi-Dimensional Mode State System
- **Refactored flat `Mod` enum into structured `ModeState` system**
  - `Focus`: Where you are (Editor, Explorer, Telescope)
  - `EditMode`: How you're interacting (Normal, Insert, Visual)
  - `SubMode`: Special overlay states (Command, OperatorPending)
  - Location: `lib/core/src/modd/mod.rs`

- **Convenience constructors for common modes**
  - `ModeState::normal()` - Editor + Normal mode
  - `ModeState::insert()` - Editor + Insert mode
  - `ModeState::visual()` - Editor + Visual mode
  - `ModeState::visual_block()` - Editor + Visual Block mode
  - `ModeState::command()` - Editor + Command sub-mode
  - `ModeState::explorer()` - Explorer + Normal mode
  - `ModeState::explorer_input()` - Explorer + Insert mode
  - `ModeState::telescope()` - Telescope + Insert mode (for typing)
  - `ModeState::telescope_normal()` - Telescope + Normal mode (for navigation)
  - `ModeState::operator_pending(operator, count)` - Operator-pending sub-mode

- **State check methods**
  - `is_normal()`, `is_insert()`, `is_visual()` - Edit mode checks
  - `is_command()`, `is_operator_pending()` - Sub-mode checks
  - `is_editor_focus()`, `is_explorer_focus()`, `is_telescope_focus()` - Focus checks

- **Status line display**
  - `display_string()` method for human-readable mode indicator

#### Telescope Mode Switching
- **Telescope now supports both Normal and Insert modes**
  - Insert mode (default): For typing search query
  - Normal mode: For j/k navigation
  - ESC switches from Insert to Normal mode in Telescope
  - New keymaps: `telescope_normal` and `telescope_insert`

### Breaking Changes

- **Removed legacy `Mod` enum** - All code must use `ModeState` struct
- **Updated `CommandResult::ModeChange`** - Now takes `ModeState` instead of `Mod`
- **Updated `ModeChangeEvent`** - Now broadcasts `ModeState`

### Mode Mapping (Old -> New)

| Old Mode | New ModeState |
|----------|---------------|
| `Mod::Normal` | `ModeState::normal()` |
| `Mod::Insert(_)` | `ModeState::insert()` |
| `Mod::Visual(_)` | `ModeState::visual()` |
| `Mod::Command` | `ModeState::command()` |
| `Mod::Explorer` | `ModeState::explorer()` |
| `Mod::ExplorerInput` | `ModeState::explorer_input()` |
| `Mod::OperatorPending { .. }` | `ModeState::operator_pending(..)` |
| `Mod::Telescope` | `ModeState::telescope()` |

### Testing

- **All 115 tests passing**
- Zero warnings from `cargo build` and `cargo clippy`

### Key Files Modified

- `lib/core/src/modd/mod.rs` - New type definitions (Focus, EditMode, SubMode, ModeState)
- `lib/core/src/runtime/core.rs` - Runtime state management
- `lib/core/src/runtime/event_loop.rs` - Event handling
- `lib/core/src/runtime/handlers.rs` - Command handlers
- `lib/core/src/event/inner/mod.rs` - Event types
- `lib/core/src/event/handler/command/mod.rs` - Command handler
- `lib/core/src/event/handler/command/dispatcher.rs` - Dispatcher
- `lib/core/src/event/handler/command/count_parser.rs` - Count parsing
- `lib/core/src/event/handler/completion.rs` - Completion handler
- `lib/core/src/bind/mod.rs` - Keymap system
- `lib/core/src/screen/status_line.rs` - Status display
- `lib/core/src/screen/mod.rs` - Screen rendering
- `lib/core/src/settings/mod.rs` - Settings
- `lib/core/src/command/builtin/mode.rs` - Mode commands
- `lib/core/src/command/traits.rs` - Command traits

### Usage Example

```rust
use reovim_core::modd::{ModeState, Focus, EditMode, SubMode};

// Create common modes
let normal = ModeState::normal();
let insert = ModeState::insert();
let visual = ModeState::visual();

// Check mode state
if mode.is_insert() {
    // Handle insert mode
}

if mode.is_telescope_focus() && mode.is_normal() {
    // Telescope in navigation mode
}

// Create custom mode
let telescope_insert = ModeState::with_focus_and_mode(
    Focus::Telescope,
    EditMode::Insert(ModExtension::Normal),
);
```

## [0.3.0] - Previous Release

See git history for earlier changes.
