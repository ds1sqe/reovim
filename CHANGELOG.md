# Changelog

All notable changes to Reovim will be documented in this file.

## [Unreleased]

### Added

- **File explorer visual enhancements** (Issue #127) - nvim-tree style coloring and tree structure
  - **Dedicated FileExplorerStyles** in theme system with distinct colors for each file category:
    - Directories: blue (bold)
    - Source files: orange (for .rs, .py, .js, etc.)
    - Config files: gray/dimmed (for .toml, .yaml, .json)
    - Documentation: cyan (for README, .md)
    - Data files: magenta (for .csv, .sql, .db)
    - Media files: pink (for images, audio, video)
    - Special files: yellow (for LICENSE, .gitignore)
    - Lock files: dimmed yellow (for Cargo.lock, package-lock.json)
    - Executable files: green (for shell scripts, binaries)
    - Hidden files: dimmed gray
  - Box-drawing characters (│, ├, └) for visual tree hierarchy
  - Three tree styles: None (indent only), Simple (ASCII ">"), BoxDrawing (Unicode)
  - Hidden items count display at bottom: "(N hidden)" when hidden files are not shown
  - Explorer settings section with options:
    - `explorer.enable_colors` - Toggle file type coloring (default: true)
    - `explorer.tree_style` - Choose tree drawing style (default: "box_drawing")
    - `explorer.show_hidden` - Toggle hidden files visibility (default: false)
    - `explorer.show_sizes` - Toggle file size display (default: false)

### Changed

- **reo-cli**: Changed default output format for `keys` command from `plain_text` to `raw_ansi` for better visual feedback (Issue #134)
- **reo-cli**: Added `--format` flag to `keys` command to allow choosing output format (raw_ansi, plain_text, cell_grid)

## [0.8.0] - 2026-01-09

### Fixed

- **tokio::spawn panics in EventBus handlers** (Issue #120) - Fix plugins using `tokio::spawn()` after PR #85 moved EventBus to std::thread
  - Capture `tokio::runtime::Handle::current()` at `subscribe()` start (runs in tokio context)
  - Use `handle.spawn()` instead of `tokio::spawn()` in EventBus handlers
  - Affected: completion (BufferModified), LSP (GotoDefinition, GotoReferences, ShowHover)

### Performance

- **Priority channels for user input events** (Issue #95, #86) - Dual-channel architecture for responsive user experience
  - Separates high-priority user input from low-priority background events
  - `hi_tx/hi_rx` (64 capacity): user input, mode changes, text insertion
  - `lo_tx/lo_rx` (255 capacity): render signals, plugins, background tasks
  - Biased `select!` ensures high-priority events processed first
  - `MAX_LO_DRAIN=16` provides fairness for background events
  - Auto-pair latency reduced from ~100ms to ~92us (1000x improvement, closes #86)

### Added

- **GitHub Actions CI pipeline** (Issue #121) - Automated code quality checks on PRs and pushes to develop
  - Format check with `cargo fmt --all --check`
  - Clippy with zero-warning enforcement (`-D warnings`)
  - Build verification for entire workspace
  - Unit tests and doc tests
  - Rust cache for faster subsequent builds

- **Dynamic content in DisplayRegistry** (Issue #57) - Allow plugins to show contextual status line info
  - `DisplayContext` struct for render context with `PluginStateRegistry` access
  - `DynamicDisplayFn` type alias for dynamic display callbacks
  - `DisplayInfo::with_dynamic()` method for fluent configuration
  - `DisplayInfoBuilder::dynamic()` builder method
  - `DisplayRegistry::display_string()` now takes `PluginStateRegistry` and invokes dynamic callbacks
  - Enables dynamic status like "Explorer (3 files)" or "Telescope (42 results)"

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

- **Light theme syntax highlighting** (Issue #43) - Light theme now uses proper One Light colors
  - Added `light_palette` module with Atom One Light colors (red, blue, purple, green, cyan, orange, yellow)
  - Created `TreesitterTheme::light()` with comprehensive capture mappings
  - Fixed `from_theme_name()` to correctly return light theme instead of dark
  - 3 new tests for light theme functionality

- **RegisterOption events not processed** (Issue #39) - Options registered via `bus.emit(RegisterOption)` are now properly added to the option registry
  - `:set` commands now work for all core options (signcolumn, number, tabwidth, etc.)
  - Fix: Create OptionRegistry before plugins subscribe and add event handler


---

For older versions (0.7.x and earlier), see [CHANGELOG-archive.md](docs/CHANGELOG-archive.md).
