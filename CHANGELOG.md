# Changelog

All notable changes to Reovim will be documented in this file.

## [Unreleased]

### Performance

- **Lazy tree-sitter query compilation** (Issue #93) - Defer query compilation from startup to first use
  - Queries now compiled on-demand when first accessed, not during language registration
  - `register_all_languages` benchmark improved from ~72ms to ~11µs (99.98% faster)
  - Startup blocking time reduced from ~72ms to near zero
  - First file open per language incurs one-time ~26ms compilation cost
  - Added `QueryCache::get_or_compile()` with interior mutability (`RwLock`) for thread-safe lazy compilation
  - Simplified `TreesitterManager::register_language()` from ~60 lines to ~5 lines

- **Background query pre-compilation** (Issue #94) - Complement to lazy compilation
  - Spawns background thread during `boot()` phase to pre-compile all queries
  - First file open is instant if background compilation has finished
  - Falls back to on-demand compilation if background hasn't completed yet
  - Added `TreesitterManager::precompile_all_queries()` helper method

### Added

- **EventScope for event lifecycle tracking** (Issue #70, #98) - GC-like scope tracking for deterministic event synchronization
  - `EventScope` struct with atomic in-flight counter and async completion notification
  - `ScopeId` for unique scope identification
  - `increment()` / `decrement()` for tracking event lifecycle
  - `wait()` / `wait_timeout()` for async completion waiting
  - Tracing support for debugging stuck scopes (`REOVIM_LOG=trace`)
  - `DynEvent` now carries optional scope for lifecycle tracking
  - `HandlerContext` propagates scope to child events automatically
  - `EventBus::emit_scoped()` for scoped event emission

- **Event bus and treesitter initialization benchmarks** (Issue #97) - Add benchmarks to measure latency sources
  - `event_bus/dyn_event_new` - DynEvent creation overhead (~10.6ns)
  - `event_bus/dispatch/handlers/*` - Dispatch latency by handler count (1: ~26ns, 100: ~360ns)
  - `event_bus/dispatch_multi_type/*` - HashMap lookup with multiple event types (~25ns)
  - `event_bus/dispatch_priority/*` - Priority-sorted handler dispatch (~42-88ns)
  - `event_bus/dispatch_emit/*` - Cascading event emission (0: ~25ns, 10: ~106ns)
  - `event_bus/dispatch_consumed/*` - Early exit on consumed events (~25-27ns)
  - `event_bus/rwlock/*` - RwLock acquisition overhead (1: ~26ns, 100: ~124ns)
  - `event_bus/subscribe` - Handler registration time (~200ns)
  - `event_bus/bottleneck_blocking/*` - Slow handler blocking impact (100µs-10ms)
  - `event_bus/bottleneck_queue/*` - Queue depth behind slow handler (~1ms baseline)
  - `event_bus/bottleneck_sequential/*` - Multiple slow handlers (accumulates linearly)
  - `event_bus/bottleneck_comparison/*` - Fast event latency with/without slow handlers (~25ns)
  - `treesitter/query_compile/highlights/*` - Query compilation time per language (rust: ~26ms)
  - `treesitter/register_language/*` - Full language registration time (rust: ~35ms)
  - `treesitter/register_all_languages` - Total startup cost (~72ms)
  - Made `HandlerContext::new()` public for benchmark access

- **Dynamic content in DisplayRegistry** (Issue #57) - Allow plugins to show contextual status line info
  - `DisplayContext` struct for render context with `PluginStateRegistry` access
  - `DynamicDisplayFn` type alias for dynamic display callbacks
  - `DisplayInfo::with_dynamic()` method for fluent configuration
  - `DisplayInfoBuilder::dynamic()` builder method
  - `DisplayRegistry::display_string()` now takes `PluginStateRegistry` and invokes dynamic callbacks
  - Enables dynamic status like "Explorer (3 files)" or "Telescope (42 results)"

- **Common UI rendering helpers** (Issue #56) - New `ui` module with shared utilities for plugins
  - `ui::text` module with Unicode-aware text manipulation functions
  - `display_width()` - Get display width of string (handles CJK, emoji)
  - `truncate_end()` - Truncate with ellipsis at end ("Hello...")
  - `truncate_start()` - Truncate with ellipsis at start ("...file.rs")
  - `align()` - Align text (left/center/right) within width
  - `pad_left()` / `pad_right()` - Pad text with fill character
  - Re-exports border utilities from `screen::border` for unified access
  - Added `unicode-width` dependency for proper Unicode handling

- **Window resize operations** (Issue #54) - Implement direction-specific window resizing
  - `SplitNode::adjust_ratio_in_direction()` - Find innermost split matching direction and adjust ratio
  - `Screen::resize_window(direction, delta)` - Public API for resizing active window
  - `WindowAction::Resize` handler now functional (5% ratio change per unit delta)
  - Vertical direction adjusts width, Horizontal direction adjusts height

- **Mouse click and scroll support** (Issue #59) - Basic mouse interaction for cursor positioning and scrolling
  - Left click: Move cursor to clicked position and focus window if different
  - Scroll wheel: Move viewport up/down by 3 lines, cursor adjusts to stay visible
  - Coordinate translation: Screen position to buffer position with gutter and scrollbar awareness
  - Server mode support: Mouse events forwarded in dual-output mode

- **Theme customization/override system** (Issue #55) - Allow users to override individual colors/styles in TOML config
  - `ThemeOverrides` struct for storing style path -> override mappings
  - `StyleOverride` struct with fg, bg, underline_color, and attribute options
  - `parse_color()` function supporting hex, rgb(), ansi:N, and named color formats
  - `Theme::from_name_with_overrides()` and `Theme::apply_overrides()` methods
  - `editor.theme_overrides` field in profile config schema
  - Example config:
    ```toml
    [editor.theme_overrides]
    "statusline.background" = { bg = "#1a1b26" }
    "gutter.line_number" = { fg = "#565f89" }
    "statusline.mode.normal" = { fg = "#1a1b26", bg = "#7aa2f7", bold = true }
    ```

- **Consolidated mode display logic into DisplayRegistry** (Issue #53) - Centralize all mode display logic in one place
  - New `display/icons.rs` module for icon constants
  - `DisplayRegistry::mode_icon()` - Get icon for current mode state
  - `DisplayRegistry::mode_style()` - Get style for current mode from theme
  - `DisplayRegistry::mode_name()` - Get display name ("Normal", "Insert", etc.)
  - `DisplayRegistry::short_mode_name()` - Get short name ("NORMAL", "INSERT", etc.)
  - `DisplayRegistry::hierarchical_display()` - Build "Kind | Mode | SubMode" string
  - Removed duplicate logic from `Screen` (was `get_mode_style()`, `get_mode_name()`)
  - Removed `ModeState::hierarchical_display()` - use `DisplayRegistry` instead
  - `ModeSnapshot::from_mode()` added for RPC serialization with display registry

- **Metadata-driven interactor input behavior** (Issue #46) - Refactored `accepts_char_input()` to use registry instead of hardcoded checks
  - `InteractorConfig` struct for configuring input behavior per interactor
  - `InteractorRegistry` for storing and querying interactor configurations
  - `accepts_char_input_with(registry)` method on `ModeState` for registry-based lookup
  - `PluginContext::register_interactor()` for plugins to register their input behavior
  - Window mode registered via `InteractorConfig::using_keymap()` instead of hardcoded exclusion
  - Default behavior: unregistered interactors accept character input (safe default)

- **HandlerContext mode change helpers** (Issue #44) - Reduce mode change boilerplate
  - `enter_interactor_mode(component_id)` - Enter interactor sub-mode for text input
  - `exit_to_normal()` - Return to normal editor mode
  - `set_mode(mode)` - Set arbitrary mode state for edge cases
  - Updated Range-Finder plugin: 5 mode changes simplified
  - Updated Microscope plugin: 1 mode change simplified

- **Subscription helper macros** (Issue #41) - Reduce plugin boilerplate with three new macros
  - `subscribe_state!` - Simple state mutation + render pattern (4 variants)
  - `subscribe_state_mode!` - State mutation + mode change + render (2 variants)
  - `subscribe_state_conditional!` - Conditional render based on return value (4 variants)
  - Migrated Explorer plugin: 23 subscription blocks (~180 lines saved)
  - Migrated Range-Finder plugin: 5 subscription blocks (~30 lines saved)
  - Total: ~210 lines of boilerplate removed across 28 subscription blocks

- **`subscribe_targeted()` API for EventBus** (Issue #45) - New method to automatically filter events by component ID
  - `TargetedEvent` trait for events with a `target: ComponentId` field
  - Implemented for `PluginTextInput`, `PluginBackspace`, and `RequestFocusChange`
  - Eliminates boilerplate target-checking code in plugin event handlers
  - Migrated 7 subscriptions across 4 plugins (Explorer, Microscope, Range-finder, Which-key)

### Changed

- **Remove dead status line code** (Issue #50) - Delete unused `StatusLineComponent` and consolidate duplicates
  - Deleted `lib/core/src/component/status_line.rs` (303 lines) - `StatusLineComponent` was never instantiated
  - Deleted `lib/core/src/screen/status_line.rs` (256 lines) - `StatusLineRenderer` trait was never called
  - Removed `impl StatusLineRenderer for Screen` (31 lines) - Only consumer of deleted functions
  - Active rendering uses `Screen::render_status_line_to_buffer()` which remains unchanged
  - Fixes OperatorPending style bug (was in dead code path using wrong style)

- **Remove unused cursor and animation theme features** (Issue #49) - Dead code cleanup
  - Removed `CursorStyles` struct (line, column, glow, pulse fields never accessed)
  - Removed `EffectConfig` struct (never instantiated)
  - Removed `AnimationConfig` struct (actual animation system uses hardcoded frame rate)
  - Removed `HighlightGroup::CursorEffect` enum variant (never applied)
  - Updated docs/animation-system.md to remove future config section

- **RAII-based cursor synchronization** (Issue #48) - Centralize cursor sync between windows and buffers
  - Added `Screen::save_cursor_to_active_window()` for pre-split cursor save
  - Added `Screen::switch_active_window()` for full cursor handoff during navigation
  - Refactored `handle_window_split()` to use centralized API (15 lines → 1 call)
  - Refactored `handle_window_navigate()` to use centralized API (50 lines → direction lookup + 1 call)
  - Eliminates manual cursor sync scattered across handlers, reducing error risk

- **Split large subscribe() methods** (Issue #52) - Improve plugin maintainability by splitting monolithic subscribe() methods
  - Explorer plugin: Split 426-line method into 9 focused sub-methods (raw_input, navigation, tree_operations, clipboard, file_operations, input_handling, visual_mode, focus_visibility, popup)
  - Range-Finder plugin: Split 191-line method into 5 focused sub-methods (jump_mode_handlers, jump_input_handler, fold_handlers, cleanup)
  - Removed `#[allow(clippy::too_many_lines)]` pragmas from both plugins

- **Decomposed window render_to_buffer() method** (Issue #51) - Extract focused helper methods for better testability
  - `compute_line_number_width()` - Calculate line number column width
  - `render_fold_marker_to_buffer()` - Render collapsed fold indicators
  - `render_empty_lines_to_buffer()` - Fill viewport with tilde markers
  - Main method reduced from ~106 to ~82 lines with clearer separation of concerns

- **Declarative mode metadata** (Issue #42) - Replace hardcoded `mode_for_command()` with `resulting_mode()` trait method
  - Added `resulting_mode()` method to `CommandTrait` with default `None` return
  - Implemented for 32 mode-changing commands (mode, window, operator, command-line)
  - Replaced 60-line string matching function with 6-line trait-based lookup
  - Provides compile-time safety: no more missing match arms causing flaky tests
  - Fixed bug: `enter_visual_line_mode` was missing from old implementation

- **Unified active buffer tracking** (Issue #47) - Removed redundant `active_buffer_id` field from Runtime
  - Screen is now the single source of truth for active buffer ID
  - `Runtime::active_buffer_id()` method derives value from `Screen::active_buffer_id()`
  - Eliminates dual tracking and manual synchronization between Runtime and Screen
  - Simplifies buffer switching, file opening, and window navigation code

- **Window move commands use SwapDirection** (Issue #54) - 8 move commands now use `SwapDirection` instead of `MoveDirection`
  - `WindowMoveLeft/Down/Up/Right` commands and their window-mode variants now perform actual swaps
  - Consistent behavior between move and swap operations

### Removed

- **`WindowAction::MoveDirection`** (Issue #54) - Removed redundant enum variant
  - Was identical in purpose to existing `SwapDirection` but never implemented
  - 8 commands updated to use `SwapDirection` directly

### Fixed

- **Event bus performance under parallel load** (Issue #85) - Fix 300ms auto-pair delay under parallel test load
  - Changed event bus processor from `tokio::spawn` to `std::thread::spawn`
  - Dedicated OS thread avoids tokio scheduler starvation under contention
  - Uses `blocking_recv()` instead of async `recv().await`
  - No API changes; event bus channel and dispatch remain unchanged

- **Light theme syntax highlighting** (Issue #43) - Light theme now uses proper One Light colors
  - Added `light_palette` module with Atom One Light colors (red, blue, purple, green, cyan, orange, yellow)
  - Created `TreesitterTheme::light()` with comprehensive capture mappings
  - Fixed `from_theme_name()` to correctly return light theme instead of dark
  - 3 new tests for light theme functionality

- **RegisterOption events not processed** (Issue #39) - Options registered via `bus.emit(RegisterOption)` are now properly added to the option registry
  - `:set` commands now work for all core options (signcolumn, number, tabwidth, etc.)
  - Fix: Create OptionRegistry before plugins subscribe and add event handler

## [0.7.10] - 2026-01-06

### Fixed

- **reo-cli REPL help text** (Issue #36) - Help text now auto-generated from clap definitions
  - Fixed incorrect `screen` command description (said "dimensions" but returned content)
  - Removed non-existent `screen-content` command from help
  - Added `screen-size` command for getting dimensions
  - Added `capture` as visible alias for `screen` command
  - Help text now always matches implementation (single source of truth)
  - Added 10 unit tests for command parsing and help generation

### Added

- **Sign column and virtual text configuration** (Issue #29) - Phase 3 of diagnostics system (#21)
  - Sign column modes: `auto`, `yes`, `no`, `number`
    - `auto`: Show sign column only when signs are present (width 2)
    - `yes`: Always show sign column (default, width 2)
    - `no`: Never show sign column
    - `number`: Display signs as background color on line numbers
  - Virtual text options: `:set virtual_text`, `virtual_text_prefix`, `virtual_text_max_length`, `virtual_text_show`
    - `virtual_text` (bool): Enable/disable virtual text display
    - `virtual_text_prefix` (string): Custom prefix (empty = use severity icons)
    - `virtual_text_max_length` (number): Maximum length before truncation (default: 80)
    - `virtual_text_show` (string): "first", "highest", or "all" mode
  - Settings menu integration for all new options under "Diagnostics" section
  - 14 new tests for sign column modes and virtual text configuration

- **Virtual text system for inline diagnostics** (Issue #28) - Display diagnostic messages after line content
  - `VirtualTextEntry` type with text, style, and priority fields
  - `VirtualTextStyles` in theme for severity-based styling (error, warn, info, hint)
  - Rendering with automatic truncation and ellipsis when exceeding viewport
  - LSP integration: severity icons (● error, ◐ warning, ⓘ info, · hint) + message
  - Priority-based resolution (ERROR 404 > WARNING 403 > INFO 402 > HINT 401)
  - 44 new tests covering all virtual text functionality

- **Multi-instance server support** (Issue #19) - Multiple reovim servers can now run concurrently
  - Port fallback: When default port (12521) is in use, server automatically tries 12522, 12523, etc.
  - Port files: Each server writes `~/.local/share/reovim/servers/<pid>.port` for discovery
  - `reo-cli list` command: Lists all running reovim server instances with PID and port
  - Auto-discovery: `reo-cli` auto-connects to single server, prompts when multiple servers running
  - Clean shutdown: Port files automatically removed when server exits

- **Ex-command registry system** - Plugins can now register custom ex-commands dynamically
  - `ExCommandRegistry` for thread-safe command registration and dispatch
  - `RegisterExCommand` event for plugin-based command registration
  - Support for three command patterns: zero-arg, single-arg, and subcommand
  - All handlers include descriptions for help/completion systems
- **Profile trait system** - Framework for extensible configuration profiles
  - `Configurable` trait for components that participate in profile save/load
  - `ProfileRegistry` for coordinating serialization across all registered components
  - `RegisterConfigurable` event for plugin components to register for profile management
  - Profile events: `ProfileLoadEvent`, `ProfileSaveEvent`, `ProfileListEvent`
- **ProfilesPlugin** - New plugin for profile command handling
  - `:profile list` - Open profile picker
  - `:profile load <name>` - Load a profile by name
  - `:profile save <name>` - Save current settings as a profile
- **Health check system** - Diagnostic system for verifying core and plugin status
  - `:health` and `:checkhealth` commands to open health check modal
  - Modal UI (z-order 700) with category grouping and navigation (j/k, r, q/Esc)
  - Expandable details view - press Space/Enter to toggle full details for selected check
  - `RegisterHealthCheck` event for plugins to register custom health checks
  - Built-in checks for runtime, plugin system, event bus, terminal, and keybindings
  - Plugin-decoupled architecture - zero core modifications using v0.7.9+ ex-command registry
- **Plugin-specific status line styling** - Plugins can now provide custom colors for status line
  - Each plugin defines its own `Style` with custom fg/bg colors
  - Visual identity: Explorer (orange), Microscope (blue), Settings (gray), Leap (green), Health (teal), Jump (gold)
  - Status line shows `[INTERACTOR][MODE]` as separate colored sections
  - `Color` type exported from `reovim_core::highlight` for plugin use
- **LSP debugging tools** (Issue #26) - Tools for debugging LSP communication
  - LSP health check: Shows server status, document count, diagnostic stats, and timestamps in `:health` modal
  - `:LspLog` command to open the latest LSP log file directly in the editor
  - Dedicated LSP log: `--lsp-log` flag for capturing JSON-RPC messages to separate file
    - `--lsp-log=default` creates timestamped `lsp-<timestamp>.log` in data directory
    - `--lsp-log=/path/to/file.log` for custom path
    - `--lsp-log=default:LEVEL` or `--lsp-log=/path:LEVEL` for configurable verbosity
    - Supported levels: error, warn, info, debug, trace (default: trace)
    - Logs show `->` for outgoing and `<-` for incoming messages
- **Request-driven cursor movement with operator support** - Plugin-to-runtime communication for cursor positioning
  - `RequestCursorMove` event allows plugins to request cursor movement
  - `Motion::JumpTo { line, column }` variant for absolute positioning
  - `InnerEvent::MoveCursor` for runtime event loop handling
  - Automatic operator integration: `d`/`y`/`c` + jump creates `OperatorMotionAction`
  - Runtime detects `OperatorPending` mode and applies operators to jump targets
  - Enables jump navigation, LSP goto-definition, and other plugin-driven navigation with full vim operator semantics

### Fixed

- **LSP diagnostics not received after opening files** (Issue #26)
  - rust-analyzer uses LSP 3.17 "pull diagnostics" instead of push notifications
  - Now request diagnostics immediately after `textDocument/didOpen`
  - Handle `workspace/diagnostic/refresh` requests from server

### Changed

- **DisplayInfo API (BREAKING)** - Simplified and made styling mandatory
  - `DisplayInfo::new()` now requires `style: Style` parameter
  - Removed `EditModeKey`, `SubModeKey` enums (mode-specific displays removed)
  - Removed `DisplayInfoBuilder::when_mode()` and `when_sub_mode()` methods
  - `DisplayRegistry` simplified to single HashMap lookup
  - `register_builtins()` now requires `theme` parameter
  - Plugins must update to provide styles when registering display info

- **Ex-command architecture refactored** - Full decoupling of core from plugin-specific commands (Issue #13)
  - Removed `Settings`, `ProfileLoad`, `ProfileSave`, `ProfileList` variants from `ExCommand` enum
  - Added generic `Plugin { command: String }` variant for all plugin-registered commands
  - Settings menu now registers `:settings` command via `RegisterExCommand` event
  - Profile commands now handled by dedicated ProfilesPlugin
  - Core handlers dispatch to registry instead of hardcoded match arms
- **Paste animation** - Visual pulse animation feedback for paste operations
  - Cyan pulsing glow (600ms, 2 cycles) when pasting with `p` or `P`
  - Distinct from yank's gold fade animation
  - Works with both characterwise and linewise paste
  - Helps locate pasted content in large files
- **Performance logging reduced** - [RTT] performance logs lowered from debug to trace level
  - Render timing, event loop, pipeline execution now require TRACE log level
  - Reduces noise in DEBUG logs while still available for performance analysis
  - Affects 10+ log statements across runtime, screen, and render modules

### Fixed

- **Yank animation for linewise motions** - `yj` and `yk` now correctly highlight entire lines
  - Previously only highlighted partial lines (current line for `yj`, upper line for `yk`)
  - Now properly animates all yanked lines from start to end
  - Affects all linewise motions: `j`, `k`, `gg`, `G`
- **Markdown decorations update immediately on insert/change** - Fixed stale cache bug in saturator
  - Saturator now calls `decorator.refresh()` before `decoration_range()` to update cached parse tree
  - Mirrors the pattern from syntax highlighting (`update_highlights()`)
  - Heading icons, bullets, checkboxes, and emphasis concealment now update in real-time
- **Decorations disabled on cursor line in insert mode** - Show raw markdown syntax while typing
  - When in insert mode, decorations are filtered out on the current line
  - Allows seeing raw markers (`# Heading`, `**bold**`, etc.) while editing
  - Decorations reappear when returning to normal mode

### Changed

- **Code cleanup in markdown plugin** - Removed dead code and streamlined query loading
  - Deleted `markdown/mod.rs` (685 lines of dead code, obsolete LanguageRenderer implementation)
  - Converted embedded query constants to `.scm` files via `include_str!()` (57 lines → 2 lines)
  - Queries now editable in `.scm` files with syntax highlighting support
  - Total reduction: ~740 lines

- **Range-Finder plugin** - Merged Leap and Fold plugins into unified navigation system
  - Combined jump navigation and code folding into single `reovim-plugin-range-finder` plugin
  - New architecture: `src/jump/` (navigation subsystem) + `src/fold/` (visibility subsystem)
  - Multi-char search: `s` + 2 chars + label to jump (leap-style navigation)
  - Enhanced f/t motions: `f`/`F`/`t`/`T` now use label selection for multiple matches
  - Smart auto-jump: 1 match → instant, 2-676 matches → show labels, >676 → cancel
  - Code folding: `za`/`zo`/`zc`/`zR`/`zM` (unchanged functionality)
  - Home row priority labels: `sfnjklhodweimbuyvrgtaqpcxz`
  - Single-char labels for <=26 matches, two-char labels for >26 matches (up to 676)
  - Two-char label input: first character validates, second completes the jump
  - Label overlay rendering with z-order 200 (above editor, below popups)
  - **BREAKING**: `f`/`F`/`t`/`T` keys now trigger leap-style label selection (replaces default vim single-char find/till)
  - Removed separate `reovim-plugin-leap` and `reovim-plugin-fold` plugins
  - Total plugin size: ~2,000 lines (vs 2,295 lines for separate plugins)

## [0.7.9]

### Added

- **Microscope syntax highlighting** - Preview panels now show syntax-highlighted code
  - File picker (`Space f f`) previews display full syntax highlighting
  - Grep picker (`Space f g`) shows highlighted matches with context
  - Recent files picker (`Space f r`) includes syntax highlighting
  - Language injection support (markdown code blocks, etc.)
  - On-demand highlighting via `SyntaxFactory` integration (no background tasks)
  - Graceful degradation when treesitter unavailable (plain text fallback)
  - New `syntax_helper` module in pickers crate for highlight computation
  - Updated `Picker::preview()` trait to accept `PickerContext` with factories

- **Landing page animation (WIP)** - Animated ASCII lion on startup dashboard
  - Three size variants: Large (roar), Medium (sleep), Small (breathing)
  - Responsive size selection based on terminal dimensions
  - Animation controller with Loop, PingPong, Once modes
  - This is the initial implementation; more dashboard enhancements planned

- **Reo color system** - 45-color HSL-organized palette (Tokyo Night + Catppuccin + One Dark)
- **Extended Rust highlighting** - Pattern types, operator expressions, async/unsafe, type syntax captures
- **Documentation** - `docs/color-system.md`, `docs/syntax-highlighting.md`

### Fixed

- **Characterwise paste positioning** - Fixed `p` command to paste AFTER cursor for characterwise yanks
  - `Y` (yank to end of line) followed by `p` now correctly pastes after cursor position
  - Previously pasted at cursor position instead of after it (breaking Vim compatibility)
  - Added cursor movement logic (`buf.cur.x += 1`) before characterwise paste when `before=false`
  - All yank operations now properly track yank type (linewise vs characterwise)

- **Linewise paste behavior** - Fixed `dd`/`yy` paste to use proper linewise semantics
  - `p` now pastes BELOW current line (Vim behavior)
  - `P` now pastes ABOVE current line (Vim behavior)
  - Previously pasted at cursor position, corrupting text for linewise operations
  - Added `insert_linewise()` method to buffer for proper linewise paste handling

- **Yank type tracking** - Extended yank/delete operators to track linewise vs characterwise
  - Added `YankType` enum and `RegisterContent` struct to register system
  - Updated `Delete` operator to detect linewise motions (`dj`, `dk`, `dd`)
  - Updated `Change` operator to detect linewise motions (`cj`, `ck`, `cc`)
  - Extended `CommandResult::ClipboardWrite` with `yank_type` field
  - All 26 operator tests pass with correct yank/paste behavior

- **Yank animations** - Added visual feedback for `Y` and `yy` commands

- **Terminal cleanup on exit** - Fixed afterimage and "%" marker issues when closing reovim
  - Implemented RAII `TerminalGuard` to guarantee cleanup even on panic
  - Now uses alternate screen mode to isolate editor UI from shell
  - Cursor is properly restored and final newline printed on exit
  - Terminal state is always restored, preventing broken terminal after crashes
  - Added `lib/core/src/command/terminal/terminal_guard.rs` with Drop trait implementation
  - `Y` (yank to end) now highlights from cursor to end of line
  - `yy` (yank line) now highlights entire current line
  - Extended `CommandResult::ClipboardWrite` with `yank_range` field for animation support
  - Animation start position adjusted (`x+1`) to avoid highlighting extra character on left
  - Visual feedback matches Vim's yank highlight feature

- **Jump list navigation (Ctrl+O/Ctrl+I)** - Fixed non-functional jump navigation (#4)
  - Added Tab as fallback keybinding for jump-newer (works in all terminals)
  - Enabled Kitty keyboard enhancement protocol for modern terminals (kitty, WezTerm, foot)
  - Added Ctrl+O/Ctrl+I/Tab bindings to insert mode (jump navigation works during editing)
  - Jump points recorded when **leaving** insert mode (records where editing finished, not started)
  - Fixed `current_index` calculation bug where first Ctrl+O did nothing
  - Fixed duplicate detection causing `current_index` to become invalid
  - Fixed duplicate detection truncating jump history (entries were lost on subsequent pushes)
  - Fixed INSERT LEAVE truncating all previous jump entries (now preserves full history)
  - Fixed Ctrl+O requiring double press (now automatically skips current position)
  - Marked LSP goto definition/references commands as jumps (gd, gr)
  - Marked leap and buffer navigation commands as jump-recording actions (s/S, H/L)
  - Root cause: Ctrl+I was indistinguishable from Tab in traditional terminals
  - UX improvement: Jump back to where you **finished** editing, not where you started

- **Landing page animation alignment** - All frame lines now have consistent widths to prevent horizontal shifting during animation

- **Landing page size thresholds** - Adjusted thresholds so "Large" requires actually large terminals (50×24+), "Medium" for normal windows (35×16+)

- **Notification tests failing locally** - LSP plugin now skips auto-start when `REOVIM_TEST` env var is set, preventing progress notifications from polluting test state

- **Tree-sitter Rust query errors** - Fixed invalid node types (`rest_pattern`, `type_bound_list`, etc.)

## [0.7.8] - 2025-12-27

### Fixed

- **Settings menu border rendering** - Now uses the plugin window system's border utilities
  - Replaced manual box-drawing character rendering with `render_border_to_buffer()`
  - Uses `BorderConfig::new(BorderStyle::Rounded).with_title("Settings")`
  - Consistent with other plugin windows (explorer, telescope)

- **Settings menu changes not applying** - Settings now apply immediately to the editor
  - Added decoupled event flow: `OptionChanged` → `Request*` events → runtime capabilities
  - CorePlugin maps option names ("number", "theme", etc.) to capability requests
  - Runtime subscribes to `RequestSetLineNumbers`, `RequestSetTheme`, etc.
  - Avoids tight coupling between runtime and option names

- **LSP hover popup not dismissing on cursor movement** - Core runtime now emits `CursorMoved` events when commands change cursor position, enabling hover dismissal and cursor-aware plugin features

- **LSP commands (gd/gr/K) not working** - Fixed saturator deadlock by spawning tokio tasks instead of blocking await in select! loop

### Added

- **Settings capability request events** - New core events for runtime capabilities
  - `RequestSetLineNumbers` / `RequestSetRelativeLineNumbers`
  - `RequestSetTheme` / `RequestSetScrollbar` / `RequestSetIndentGuide`
  - Corresponding `InnerEvent` variants for runtime handling
  - Enables plugins to change settings without knowing runtime internals

- **LSP progress notifications** - Display rust-analyzer indexing progress in notification popup and statusline

- **LSP hover with markdown rendering** - Hover popups render formatted markdown (bold, italic, code blocks)

- **LSP multiple definition picker** - When `gd` returns multiple locations, show picker for user selection

- **Statusline Plugin** (`reovim-plugin-statusline`) - Section-based API for statusline extensions
  - `StatuslineSection` type with id, priority, alignment, and render callback
  - `SharedStatuslineManager` with thread-safe section registration
  - Implements core's generic `StatuslineSectionProvider` trait
  - Events: `StatuslineSectionRegister`, `StatuslineSectionUnregister`, `StatuslineRefresh`
  - Other plugins can register dynamic sections (e.g., LSP status, diagnostics count)

- **Notification Plugin** (`reovim-plugin-notification`) - Toast notifications and progress bars
  - `NotificationLevel` variants: Info, Success, Warning, Error with colored icons
  - `ProgressNotification` with percentage bar or indeterminate spinner
  - `SharedNotificationManager` for state management
  - Plugin-local `NotificationStyles` (not in core Theme)
  - Events: `NotificationShow`, `NotificationDismiss`, `ProgressUpdate`, `ProgressComplete`
  - `NotificationPluginWindow` with z-order 500 (above most UI)
  - Configurable position: TopRight, TopLeft, BottomRight, BottomLeft, TopCenter, BottomCenter

- **`StatuslineSectionProvider` trait** - Generic core API for statusline extensions
  - Added to `lib/core/src/plugin/statusline.rs`
  - `StatuslineRenderContext` with plugin state and screen dimensions
  - `RenderedSection` output type with text, style, alignment, priority
  - Provider hook in `render_status_line_to_buffer()` for generic section rendering
  - Follows same pattern as `SyntaxFactory` - core is plugin-agnostic

## [0.7.7] - 2025-12-27

### Changed

- **Which-key decoupled from core** - Complete removal of plugin-specific code from reovim-core
  - Deleted `lib/core/src/which_key.rs` - event now defined in plugin
  - Removed hard-coded `?` key dispatch from command handler
  - Removed `ComposableId::WhichKey` enum variant (use `Custom("which_key")`)
  - Removed `disable_which_key` behavior modifier
  - Removed `STATE_WHICHKEY` RPC constant
  - Plugin now registers all keybindings via `build()` method

- **Renamed `hint`/`group` to `description`/`category`** - General-purpose keybinding metadata
  - `KeyBinding.hint` → `KeyBinding.description`
  - `KeyBinding.group` → `KeyBinding.category`
  - `KeyMapInner::with_hint()` → `KeyMapInner::with_description()`
  - `KeyMapInner::group()` → `KeyMapInner::with_category()`

### Added

- **Plugin Decoupling Policy** - Documented architectural principle
  - Added policy to CLAUDE.md: "Never add plugin-specific code to core"
  - Added "Plugin Decoupling Principles" section to docs/plugin-system.md
  - Added decoupling note to docs/DEVELOPMENT.md
  - If API is insufficient, propose extension rather than adding coupling

- **`WhichKeyTrigger` inline command** - Plugin-owned keybinding trigger
  - Carries prefix for context-specific bindings (e.g., `g?`, `<Space>?`)
  - Uses `CommandRef::Inline(Arc<dyn CommandTrait>)` pattern
  - Emits `WhichKeyOpen` event via `CommandResult::EmitEvent`

## [0.7.6] - 2025-12-27

### Added

- **Completion fuzzy filtering** - Nucleo-based fuzzy matching with score threshold
  - Filters items based on typed prefix using `nucleo` crate
  - Minimum score threshold (`prefix.len() * 10`) prevents irrelevant matches
  - Sets `CompletionItem::score` and `match_indices` for highlighting

- **Completion match highlighting** - Per-character styling for matched characters
  - Added `match_indices: Vec<u32>` field to `CompletionItem`
  - Added `match_fg` style to `PopupStyles` (yellow/bold)
  - Matched characters rendered with highlight color

- **Completion UI columns** - Kind and source indicators in popup
  - Kind column (left): Colored abbreviation (`fn`, `mod`, `st`, `var`, etc.)
  - Source column (right): Dimmed source name (`buffer_words`, `lsp`, etc.)
  - Format: `[kind] [label] [source]`

- **RequestInsertText prefix deletion** - Replace typed prefix on completion confirm
  - Added `delete_prefix_len: usize` field to `RequestInsertText` event
  - Runtime deletes prefix before inserting completion text
  - Fixes `pkg` → `CARGO_PKG` instead of `pkgCARGO_PKG`

- **Completion auto-popup** - Automatic completion after 300ms debounce
  - Triggers on text insertion in insert mode
  - Uses generation counter for proper debounce cancellation
  - Sends `CommandEvent` to trigger completion with buffer context

- **EditorContext active window fields** - Window context for popup positioning
  - `active_window_anchor_x/y`, `active_window_gutter_width`, `active_window_scroll_y`
  - `cursor_screen_x()` and `cursor_screen_y()` helper methods
  - Enables accurate screen coordinate calculation for plugin windows

### Fixed

- **Completion popup position** - Now correctly positioned relative to cursor
  - Uses EditorContext to transform buffer coordinates to screen coordinates
  - Accounts for window anchor, line number gutter, and scroll offset

- **Completion ghost text position** - Inline preview now at correct cursor location
  - Uses `cursor_screen_x/y()` helpers for accurate positioning

- **Completion prefix extraction** - Proper word boundary detection
  - Added `is_word_boundary()` function with programming delimiters
  - Correctly extracts prefix for cases like `foo.bar.hel` -> `hel`

- **Completion live update** - Popup updates as user types
  - Re-triggers completion immediately when active (no debounce delay)
  - Dismisses on whitespace, mode change, or empty prefix

- **Completion dismiss behavior** - Proper cleanup on various events
  - Dismisses when leaving insert mode (via `ModeChanged`)
  - Dismisses when typing whitespace (space/enter)
  - Validates cursor position when completion re-triggers

- **Microscope backspace** - Backspace now works in microscope interactor mode
  - Added `PluginBackspace` subscription to microscope plugin
  - Previously, backspace events were silently dropped in microscope query input

### Changed

- **reo-cli keys response** - Enhanced feedback with parsed keys list
  - Response now includes `"keys": ["i", "H", "e", "l", "l", "o", "<Esc>"]`
  - Helps debugging key parsing issues

## [0.7.5] - 2025-12-27

### Added

- **LSP hover popup** - Floating documentation window on `K` keypress
  - Lock-free `HoverCache` using `ArcSwap` for non-blocking render
  - `HoverPluginWindow` with bordered popup (z-order 300)
  - Auto-dismiss on cursor move or mode change

- **LSP references picker** - Show references in microscope picker on `gr`
  - `LspReferencesPicker` implementing `Picker` trait
  - File preview with context around reference location
  - Navigation via `MicroscopeAction::GotoLocation`

## [0.7.4] - 2025-12-27

### Added

- **Which-key plugin** - Shows available keybindings in a popup panel
  - Press `?` after any prefix key (e.g., `g?`, `<Space>?`, `<C-w>?`) to see available bindings
  - Displays key sequences with descriptions from command registry
  - Type additional characters to filter the binding list
  - Backspace to remove filter, Escape to close
  - Background saturator for non-blocking UI updates
  - Integrated with `PluginStateRegistry` for keymap/command access

### Fixed

- **Plugin loader extraction bug** - All plugins now boot correctly
  - Fixed algorithm that caused only 1 plugin to have `boot()` called
  - Plugins with background saturators (completion, which-key) now work properly

- **Flaky test timing** - Fixed intermittent test failure
  - `test_vsplit_cursor_movement_after_navigate` now waits for cursor movement

## [0.7.2] - 2025-12-26

### Added

- **Extensible option system** - Plugin-extensible settings with full validation
  - Plugins can register their own options via `PluginContext::option()` builder API
  - Type-safe `OptionValue` variants: `Bool`, `Integer`, `String`, `Choice`
  - Constraint validation: min/max ranges, string length limits
  - Dynamic `:set` commands: `:set plugin.treesitter.timeout=200`
  - Query options: `:set optionname?`, reset to default: `:set optionname&`
  - TOML persistence in profiles under `[plugin.{plugin_name}]` sections
  - `RegisterOption` and `OptionChanged` events for plugin communication
  - Thread-safe `OptionRegistry` with alias support (short names)

- **`--log` CLI option** - Configurable log output destination
  - Custom file path: `--log=/path/to/file.log`
  - Stderr output: `--log=-` or `--log=stderr` (with ANSI colors)
  - Disable logging: `--log=none` or `--log=off`
  - Default behavior unchanged when not specified

- **Animation system** - Visual effects framework for UI feedback
  - Status line flash on mode change (smooth color transition)
  - Idle shimmer effect on status line after 3 seconds of inactivity
  - Yank blink effect - brief flash on yanked text (both operator motions and visual mode)
  - `AnimatedColor` types: pulse, shimmer, transition with easing functions
  - `SweepConfig` for position-based glow effects (glowing reflection)
  - Background animation controller at 30 FPS (only ticks when effects active)
  - `AnimationRenderStage` for applying effects in render pipeline

- **Microscope RPC support** - JSON-RPC endpoint for fuzzy finder state
  - `state/microscope` RPC method returns picker state (active, query, items, selection)
  - Integration test infrastructure for microscope plugin
  - Test port range changed to 17000-17099 for parallel test execution

### Fixed

- **Microscope preview** - Preview panel now displays file content
  - Preview loads on picker open and updates on navigation (j/k)
  - Files picker shows file content preview for selected item

- **Microscope event handlers** - Add missing command handlers
  - Confirm (Enter), ClearQuery, DeleteWord, cursor movement, page navigation
  - MicroscopeConfirm opens selected file and closes picker

- **Microscope UI cleanup** - Cleaner status display
  - Remove [N]/[I] mode indicator from title bar
  - Status line shows "Editor | Normal" instead of "microscope"

- **Preview highlighting infrastructure** - Prepare for syntax highlighting
  - Add StyledSpan type and styled_lines field to PreviewContent
  - render_preview_panel supports highlight_line and per-character styles

## [0.7.0] - 2025-12-25

### Added

- **Window mode (Ctrl-W)** - Hierarchical status line display for window navigation
  - `<C-w>h/j/k/l` for directional window navigation
  - `<C-w>v` for vertical split, `<C-w>s` for horizontal split
  - `<C-w>c` to close window, `<C-w>o` for only window
  - Status line shows current mode context

- **Pair plugin** - Comprehensive bracket matching and highlighting
  - Rainbow bracket coloring based on nesting depth (6-color cycle)
  - Matched pair highlighting when cursor is inside brackets
  - **Bold + underline** when cursor is directly ON a bracket
  - Unmatched bracket warning with red underline
  - Auto-pair insertion: typing `(`, `[`, or `{` automatically inserts closing bracket
  - Cursor positioned between brackets after auto-insertion

- **Language injection highlighting** - Embedded code blocks now have proper syntax highlighting
  - Markdown fenced code blocks (` ```rust `, ` ```python `, etc.) display with language-specific colors
  - Rust doc comments (`///`, `//!`, `/** */`) inject markdown for rich rendering
  - Added `saturate_injections()` to `SyntaxProvider` trait for eager injection computation
  - Injection highlights merge with decoration backgrounds (code blocks have both syntax colors AND grey tint)

- **Per-buffer saturator** - Non-blocking syntax highlighting architecture
  - Each buffer has its own saturator for independent highlighting
  - Prevents blocking on large files

- **Icon system** - Improved file and UI icons

### Architecture

- **Buffer-centric syntax highlighting** - Syntax highlighting is now buffer-centric
  - Highlights computed per-buffer, not per-window
  - Better separation of concerns

- **Plugin system decoupling** - Complete separation of plugins from core
  - Plugins communicate via events only
  - No direct core dependencies in plugin code

- **Render system refactor** - Improved rendering pipeline
  - Cleaner separation of rendering stages
  - Better performance characteristics

- **Remove UIComponent and Overlay systems** - Simplified component architecture
  - Replaced with more focused abstractions

### Fixed

- **Visual selection yank/delete** - Fixed yank and delete operations in visual mode

- **Scroll issues** - Fixed various scrolling edge cases

- **Explorer file operations** - Fixed character input not working in explorer file operations
  - Character input (create file, rename, etc.) now correctly captures typed characters
  - Backspace works correctly in input mode

- **Render issues** - Fixed various rendering edge cases

### Improved

- **reo-cli capture** - Simplified screen capture command
  - `capture` command prints raw screen content directly
  - Removed JSON wrapper for better usability and piping


---

For older versions (0.6.x and earlier), see [CHANGELOG-archive.md](./docs/CHANGELOG-archive.md).
