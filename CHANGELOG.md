# Changelog

All notable changes to Reovim will be documented in this file.

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
