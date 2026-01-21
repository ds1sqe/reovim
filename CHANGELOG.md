# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.1-dev

### Added

- Dynamic module loading infrastructure (#390)
  - All 17 modules now support dynamic loading with `crate-type = ["cdylib", "rlib"]`
  - All modules now use `declare_module!` macro for FFI entry point generation
  - Added `dynamic` feature flag to gate FFI symbols (prevents duplicate symbols when modules depend on each other)
  - Added constructors (`new()`) to all modules for dynamic instantiation
  - New `scripts/build-module.sh` for building modules as shared libraries
    - Uses `--features dynamic` to enable FFI symbols
    - Platform-aware output (.so/.dylib/.dll)
    - FFI symbol verification (validates all 10 required symbols)
    - Install support (`--install` to `~/.local/share/reovim/modules/`)
    - Config install support (`--install-config` to `~/.config/reovim/`)
    - Batch build support (`--all` for all cdylib modules)
  - New `config/config.toml.example` for module configuration template
  - Startup module loading from XDG paths (`~/.local/share/reovim/modules/`)
    - Config-based loading with static module fallback
    - Modules load dynamically first, fall back to compiled-in if not found
  - Name-based module loading via RPC (`module/load undotree`)
    - Searches XDG paths for matching `.so` files
    - Supports tilde expansion (`~/path/to/module.so`)
  - New `autoload_installed` config option to load all discovered modules
    - When `true`, all modules in search paths are loaded automatically
    - Use `skip` to exclude specific modules
  - Module documentation updated with build script usage

- Empty session handler mechanism (#369)
  - New `lib/drivers/session/` driver with `EmptySessionHandler` trait
  - `EmptySessionHandlerRegistration` added to kernel API
  - `Module` trait extended with `empty_session_handlers()` method
  - `EmptySessionHandlerRegistry` in runner for handler resolution
  - `modules/scratch-buffer/` with `ScratchBufferHandler` creates empty buffer on startup
  - Sessions now start with a usable buffer instead of a blank screen

- Expand defaults module as central policy aggregator (#381)
  - `modules/defaults` now includes empty session handler (`scratch-buffer`)
  - `empty_session_handler()` factory function for centralized handler access
  - `empty_session_handlers()` method on `DefaultsModule`
  - Runner no longer directly imports `scratch-buffer` module
  - Wiring infrastructure prepared for dynamic module loading (#265)

- Comprehensive tests for count prefix (#337) and $ motion (#340)
  - Added 5 $ motion tests to `runner/tests/cursor_movement.rs`: from middle, empty line, single char, with j navigation
  - Added 6 count prefix edge case tests to `runner/tests/operators.rs`: exceeds lines/chars, 4p paste, 10yy yank, 999j clamp, combined $ and count
  - 1 test marked ignored: `test_dollar_with_count` (count prefix for $ not yet implemented)
  - Total enabled tests: 21 cursor_movement + 26 operators (11 tests added)

- E2E test suite audit and documentation (#307 Phase 1)
  - Updated test file headers with accurate status and blocking issues
  - Clarified ignore reasons with specific issue references (#338, #385)
  - Design decision: $ motion ignores count prefix (2$ = $, simplified)
  - Test status summary:
    - `edge_cases.rs`: 10 enabled, 0 ignored - **#312 complete**
    - `operators.rs`: 24 enabled, 10 ignored (need operator-pending mode)
    - `cursor_movement.rs`: 22 enabled, 0 ignored - **#309 complete**
    - `search.rs`: 2 enabled, 13 ignored (need command-line mode #338)
    - `undo_redo.rs`: 5 enabled, 3 ignored (need full undo #385)
    - `registers.rs`: 0 enabled, 5 ignored (need debug/registers RPC)
- Window layout subsystem traits - Phase 1 Part 1 of nested compositor (#397)
  - New `lib/drivers/display/src/layout/` module with trait definitions
  - `RootCompositor` trait: layer lifecycle, focus routing, z-order stacking
  - `WindowLayerCompositor` trait: per-layer tiled/float/overlay zone management
  - `TiledLayer` trait: vim-style splits with navigate, resize, equalize, cycle
  - `FloatingLayer` trait: free-positioned windows (stub for Phase 2)
  - `OverlayLayer` trait: popups and menus (stub for Phase 3)
  - `ViewManager` trait: per-window content state (cursor, scroll)
  - Core types: `Layer`, `LayerId`, `Zone`, `WindowPlacement`, `Anchor`, `View`, `Position`
  - Strong-typed index newtypes: `LineIndex`, `ColIndex` prevent mixing line/column indices
  - `WindowError` enum with vim-compatible error codes (E444, E36, E94)
  - Hyprland-inspired architecture with nested layers and z-order computation
  - Part of Epic #403 (Window Subsystem)

- Window layout subsystem implementation - Phase 1 Part 2 of nested compositor (#397)
  - `TilingLayout` implements `TiledLayer` trait with full window management
  - `SplitTree` enhanced with winnr ordering (top-to-bottom, left-to-right)
  - `HybridCompositor` implements `RootCompositor` for multi-layer management
  - `DefaultLayer` implements `WindowLayerCompositor` with tiled zone support
  - `CompositorApi` trait in session driver for high-level window operations
  - `SessionRuntime` implements `CompositorApi` with full compositor integration
  - All 15 window commands implemented: split, close, navigate, cycle, resize
  - Module provides compositor via `compositor()` method, runner extracts via `Any`
  - Keybinding configuration is policy - left to editor personality modules
  - Part of Epic #403 (Window Subsystem)

### Changed

- Unified `WindowId` to single kernel definition (#410)
  - Display driver re-exports kernel's `WindowId` (was redefinition)
  - Protocol uses `WireWindowId` wrapper with `#[serde(transparent)]` for RPC
  - All `WindowId::new(explicit_id)` replaced with `WindowId::from_raw(id)`
  - Added `PartialOrd, Ord` derives to kernel `WindowId` for sorting
  - Simplified `RuntimeAdapter` by removing type conversion boilerplate
  - Zero raw `.0` access - use `.as_usize()` accessor

- Added tmux-like session architecture documentation (#411)
  - New `docs/architecture/session-model.md` with Session/Client concepts
  - Updated architecture overview with session model diagram
  - Updated driver session docs with SSOT and ClientId sections
  - Updated runner docs with event-driven architecture and thin runner philosophy
  - Updated CLAUDE.md with session model explanation

- Wired `EventBus` for key dispatch - event-driven key handling (#408)
  - Added `KeyPressEvent` type in kernel with `SessionId` and `ClientId` for event routing
  - Created `handlers.rs` module with `register_key_handler()` infrastructure
  - `RecursionGuard` with thread-local counter prevents infinite loops (limit: 16)
  - Panic isolation via `catch_unwind` prevents handler crashes from affecting server
  - Handler subscription stored in `EventLoop` for RAII cleanup on drop
  - `EventLoop::handle_key()` now emits events to `EventBus` with fallback to direct resolver
  - Type conversion layer between driver `KeyEvent` and kernel `KeyInput`

- Simplified event loop to emit + process pattern (#409)
  - `handle_key()` now only emits - stores key and sends to channel, no resolution
  - New `process_events()` method handles resolution with full `&mut self` access
  - `run()` and `step()` now call emit + process sequentially
  - Removed fallback path from `handle_key()` - resolution only in `process_events()`
  - Added `pending_key_event` field with documented single-slot safety guarantees
  - Uses `EventBus::new_with_channel(1024)` for future async/multi-session support
  - Handler infrastructure preserved in `handlers.rs` for future plugin support
  - Added 13 new tests for emit + process pattern and conversion functions

- Deleted `AppStateRuntime` adapter (~500 lines) - thin runner architecture (#407)
  - Created `RuntimeAdapter` that combines `SessionRuntime` with `WindowRegistry`
  - `RuntimeAdapter` implements all session API traits: `ModeApi`, `BufferApi`, `WindowApi`, `RegisterApi`, `ExtensionApi`, `CommandApi`, `ChangeTracker`
  - Window operations delegated directly to `WindowRegistry` (runner-layer SSOT)
  - Mode/buffer/register/extension operations delegated to `SessionRuntime` (driver-layer SSOT)
  - Event loop now uses `RuntimeAdapter` for resolver key handling
  - Removed redundant `WindowBridge` adapter

- Refactored `SessionState` to use `driver::Session` as SSOT for session state (#406)
  - `driver_session` is now the Single Source of Truth for per-session state
  - Fields moved to `driver_session`: `mode_stack`, `pending_keys`, `extensions`, `active_buffer`, `terminal_size`
  - `AppState` now contains only runner-specific state: `kernel`, `undo_registry`, `windows`, `cmdline`
  - New delegation methods on `SessionState`: `mode_stack()`, `session_active_buffer()`, `session_terminal_size()`, etc.
  - `CommandRegistry::execute()` now takes `&mut DriverSession` for SSOT access
  - Updated docs/architecture/runner/server/sessions.md with SSOT architecture

- Clarified Session vs Client types with tmux-like semantics (#405)
  - Renamed `driver::SessionId(usize)` to `ClientId(usize)` - identifies client connections
  - `runner::SessionId(Arc<str>)` unchanged - identifies named editing sessions
  - Display format updated: "client-{id}" (was "session-{id}")
  - Clear documentation explaining multi-client session semantics
  - 15 files updated across driver, modules, and runner layers

- Unified ID types to `usize` with standardized accessor (#412)
  - All numeric ID types (`WindowId`, `SessionId`, `ClientId`) converted from `u64` to `usize`
  - Standardized accessor method: `as_usize()` replaces `raw()`, `as_u64()`, `value()`
  - Hard Typing enforced: raw `usize` fields replaced with proper newtypes (`BufferId`, `WindowId`)
  - Atomic counters updated: `AtomicU64` → `AtomicUsize` for ID generation
  - Protocol types use newtype wrappers for type-safe serialization boundaries

- SessionApi migration: Complete escape hatch elimination (#394)
  - Changed `CommandHandler::execute` signature from `&mut KernelContext` to `&mut SessionRuntime<'_>`
  - Commands now access session state via Session APIs (ModeApi, BufferApi, WindowApi, RegisterApi)
  - **Phase 1-6**: Mode, cursor, motion, paste commands migrated
  - **Phase 7**: Buffer mutation commands using existing `delete_range()`, `insert_text()` APIs
  - **Phase 8**: Visual selection API implemented (`selection()`, `set_selection()`, `swap_selection_ends()`)
  - **Phase 9**: Motion/textobject commands via `with_buffer_read()` callback pattern
  - **Phase 10**: File operations via `buffer_content()`, `buffer_file_path()`, `is_buffer_modified()`
  - Extended BufferApi with: `buffer_text_range()`, `buffer_content()`, `buffer_file_path()`, `is_buffer_modified()`, `set_buffer_modified()`
  - Added `with_buffer_read()` on SessionRuntime for callback-based buffer access (dyn-compatible)
  - Created RegisterApi trait: `get_register()`, `set_register()` for register access
  - Extended ChangeTracker trait with `record_cursor_move()` for cursor change tracking
  - **Zero escape hatches** remaining in command handler `execute()` methods
  - Test infrastructure updated: `TestSessionRuntime` with comprehensive buffer/register testing
  - ~100 commands across 30+ files migrated to pure API usage

- Type consolidation and layer clarity (#391)
  - Deleted dead code: `ModeInput` trait, `PopResult` enum (session driver), `mode_registry.rs`
  - Renamed `CommandContext` → `ExCommandContext` and `CommandHandler` → `ExCommandHandler` in modules/commands to avoid collision with driver types
  - Added `OperatorArgs` type for shared count/register fields (DRY principle)
  - Created `docs/architecture/type-layers.md` documenting type ownership across layers
  - Kernel Mode trait now owns `accepts_char_input()` - no separate ModeInput trait needed

- Strong-typed CommandId for keybindings (#377, #378)
  - `KeybindingRegistration::command_id` changed from `&'static str` to `CommandId`
  - Compile-time verification: typos in command IDs now cause compile errors
  - Created `ids.rs` in modules: editor, motions, vim, textobjects, undotree, layout
  - All keybindings now use typed `CommandId` constants instead of string literals

- CommandProvider trait for decoupled command registration (#378)
  - New `CommandProvider` trait in `lib/drivers/command/` for modules to provide handlers
  - Modules (editor, motions, vim) implement `CommandProvider`
  - Runner uses `wire_module_commands()` instead of hardcoded registration
  - Maintains kernel purity: command registration stays in driver layer

- Runner simplification: pure mechanism with zero vim-specific knowledge (#385)
  - Removed `PendingOperator` from `AppState` - moved to vim module's `VimSessionState`
  - Extended `PopResult::OperatorRange` with operator info (SSOT for operator execution)
  - Simplified event loop: resolvers handle all key-to-action mapping
  - Modules store policy state via `ExtensionMap` and `SessionExtension` trait
  - Removed outdated demo example
  - Updated runner documentation to reflect mechanism-only design

### Fixed

- Character insertion and mode switching fixes (#372)
  - Fixed command ID resolution: keybindings now default to registering module
  - Fixed vim commands not registered (enter-insert, visual operations, etc.)
  - Updated vim bindings to use editor: prefix for editor commands (cursor, scroll, undo, etc.)
  - Fixed mode lookup using id.name() instead of display_name
  - Fixed invalid keybindings: `<` and `>` now use `<lt>` and `<gt>` notation
  - Unicode character insertion (emoji, CJK) now works in insert mode

- RPC input handler now handles count prefixes for movements (#372)
  - Count digits (e.g., "3j", "10G") are accumulated before keymap lookup
  - Accumulated count is passed to commands via CommandContext
  - Fixes integration tests: `test_3j_count_movement`, `test_5g_moves_to_line_5`

- Mode ownership: VimMode now properly owned by vim module (#372)
  - Added `is_entry()` method to Mode trait for modes to declare entry status
  - ModeRegistry auto-detects entry mode during registration
  - Session starts in `vim:normal` instead of hardcoded `editor:normal`
  - Fixed keybinding mode prefixes: `editor:*` → `vim:*` in all VimModule bindings
  - Fixes keybindings not working due to mode mismatch

- TUI not receiving buffer-scoped notifications (#365)
  - TUI now calls `editor/set_active_buffer` on connect to register for notifications
  - Fixes input appearing unresponsive (cursor_moved, buffer_modified, render_complete were missed)

- Complete manager integration for server discovery (#356)
  - Port separation: manager owns 12521, servers start at 12522
  - Server auto-starts manager and registers on startup
  - CLI auto_discover() checks registry before port scanning
  - REPL: add `list` and `connect` commands for server switching

- Server now emits `render/complete` notification (#320)
  - Emitted after any visual state change (mode, cursor, buffer modified)
  - Buffer-scoped when active buffer exists, session-wide otherwise
  - TUI clients use this to know when to refresh display

- Ghost statusline on resize in TUI debug mode (#362)
  - Clear both buffers in `FrameRenderer::resize()` to remove stale content
  - Tick handler now triggers full refresh instead of independent flush/swap
  - Prevents ghost from ping-ponging between double buffers after resize

### Added

- TUI local shortcuts, error display, and embedded CLI panel (#368)
  - Error display: RPC errors shown in statusline (red, auto-clear after 5s)
  - Prefix mode: `<C-b>` enters prefix mode (2s timeout, `^B-` indicator in debug mode)
    - `<C-b>d` detaches from server without killing it
    - `<C-b>;` toggles embedded CLI panel
    - `<C-b><C-b>` sends literal Ctrl+B to server
  - CLI panel: embedded command panel for quick commands
    - Fire-and-forget commands: `keys`, `resize`, `open`, `kill`, `help`
    - Command history navigation with Up/Down arrows
    - Line editing: Left/Right, Home/End, Backspace, Delete

- CLI panel query commands with response correlation (#371)
  - Query commands now work: `mode`, `cursor`, `screen`, `buffers`, `buffer`, `modules`
  - Debug queries: `version`, `uptime`, `registers`, `marks`, `log-level`, `log-tail`
  - Raw RPC: `call <method> [params]` for arbitrary server queries
  - Pending state: yellow "Querying..." indicator while awaiting response
  - Timeout: 10-second timeout with automatic error display
  - Max 16 concurrent pending requests to prevent memory issues

- CLI panel bug fixes and enhancements (#371)
  - Fix: Unicode characters (Korean, Chinese, etc.) no longer cause panics
  - Fix: Query command responses now routed correctly (fixes timeout issue)
  - Panel now uses half screen height for better visibility
  - Added PageUp/PageDown for history scrolling, Shift+G to scroll to bottom

- Unified debug flags in integrated mode (#364)
  - `reovim --debug` now works (previously only `reovim tui --debug`)
  - Added `--debug-dir` and `--debug-name` flags to main CLI
  - More verbose help descriptions

- TUI Debug Mode with statusline and frame buffer capture (#358)
  - `--debug` flag enables debug statusline with timestamp, server, mode, modules
  - Frame buffer capture every 5 seconds to `~/.local/share/reovim/logs/tui/frame-buffer/`
  - LLM-friendly capture format with metadata header and ANSI-styled screen content
  - Wired TUI to use `FrameRenderer` double-buffer for accurate capture
  - Session logging to `~/.local/share/reovim/logs/tui/{name}_{time}.log`

- Register selection prefix (`"a`) - specify register for yank/delete/paste (#333)
  - `"ayy` yanks line into register 'a'
  - `"ap` pastes from register 'a'
  - `"add` deletes line into register 'a'
  - Supports a-z, A-Z (append), 0-9, special registers

- Search commands now functional (#335)
  - `/pattern<Enter>` - Forward search
  - `?pattern<Enter>` - Backward search
  - `n` / `N` - Next/previous match
  - `*` / `#` - Search word under cursor

- Operator-motion combinations (#336)
  - `dw`, `db`, `d$`, `dj`, `dk`, `dgg`, `dG` - Delete with motion
  - `cw`, `cb`, `c$`, `cj`, `ck` - Change with motion (enters insert mode)
  - `yw`, `yb`, `y$`, `yj`, `yk` - Yank with motion
  - Escape cancels pending operator
  - Full operator-pending mode support

- Kernel Timer API for delayed and periodic work scheduling (#343)
  - `Runtime::schedule_delayed(delay, callback)` - one-shot timer
  - `Runtime::schedule_periodic(interval, callback)` - repeating timer
  - RAII `TimerHandle` auto-cancels on drop
  - Timer wheel implementation with O(1) cancel
  - Thread-safe with configurable `max_timers` limit
  - Integrated with Runtime tick cycle and panic safety

- Timer FFI extension for external modules (#343)
  - `reovim_schedule_delayed()` - C FFI for one-shot timers
  - `reovim_schedule_periodic()` - C FFI for periodic timers
  - `reovim_cancel_timer()` - Cancel scheduled timer
  - `reovim_timer_is_pending()` - Check timer status
  - `reovim_timer_count()` - Get active timer count
  - Added timer types to `include/reovim.h`

- Mechanism/Policy separation for key lookup (#353)
  - `KeyLookupState` enum reports FACTS about bindings (no policy decisions)
  - `KeyLookupPolicy` trait for interpreting lookup results
  - `VimLookupPolicy` - wait for longer sequences (dd after d)
  - `EagerLookupPolicy` - execute exact matches immediately
  - `KeymapQuery` trait for resolvers to query bindings
  - `lookup_with_policy()` method for policy-based lookup
  - Layered bindings: User > Policy > Base (for future user overrides)

- Vim policy module separation (#357)
  - Created `modules/vim/` with VimModule for all Vim keybindings
  - Moved bindings from keymap to vim module (normal, insert, visual, operator-pending, commandline)
  - Made `modules/keymap/` pure mechanism (InteractorRegistry, ComponentId, InteractorConfig only)
  - Enables future policy modules (emacs, kakoune) without kernel changes

- TUI log panel for real-time log streaming (#332)
  - `debug/log_subscribe` and `debug/log_unsubscribe` RPC methods
  - Real-time `LOG_ENTRY` notifications with level filtering
  - Channel-based async bridge (sync tracing -> async notifications)
  - TUI log panel with scroll, filter by level, and keybindings
  - `Ctrl+L` toggles panel, `1-5` filters levels, `j/k` scrolls

- Unified Port System with instance management (#350)
  - Transport abstraction layer with platform-agnostic `LocalAddr` type
  - Instance registry for server discovery (file-based JSON with auto-cleanup)
  - CLI flags: `-L`/`--instance` for named instances, `-S`/`--socket-path` for explicit paths
  - Port Manager daemon on 127.0.0.1:12521 with auto-start capability
  - Session commands: `:detach`, `:servers`, `:kill-server`
  - Keybindings: `<C-b>d` detach, `<C-b>s` servers, `<C-b>&` kill-server
  - DETACH notification for graceful client disconnect
  - Full backward compatibility with existing `--tcp` and `--socket` flags

- CLI log command enhancements (#324)
  - Log filtering: `reovim cli log-tail --level warn --target runner --grep error`
  - Dynamic log level: `reovim cli log-level debug` (changes at runtime)
  - Log streaming: `reovim cli log-tail --follow` (like `tail -f`)
  - Color-coded output: ERROR (red), WARN (yellow), INFO (green), DEBUG (cyan), TRACE (gray)
  - Uses `tracing_subscriber::reload` layer for runtime log level changes

- Command-line mode infrastructure (#338)
  - `EditorMode::CommandLine` variant with proper trait implementations
  - `CommandLineState` for tracking input buffer and mode state
  - `EnterCommandLineMode` and `ExitCommandLineMode` commands
  - Keybindings: `:` enters command-line mode, `Esc`/`Enter`/`C-c` exits

- Multi-client test infrastructure: `open_buffer()` method for TestClient (#339)

- Flexible mode system architecture (#348)
  - `ModeKeyResolver` trait for mode-specific key handling
  - `ResolveResult`, `ModeTransition`, `PopResult` for resolver responses
  - `ModeState`, `TransitionContext`, `ResolveContext` for state management
  - `ArgValue` for dynamic command arguments
  - Foundation for supporting different editing styles (Vim, Emacs, Kakoune)

- User keymap configuration via `~/.config/reovim/keymap.toml` (#361)
  - Override keybindings at User layer (highest priority)
  - `[bindings.normal]` section for adding/overriding bindings
  - `[remove.normal]` section for disabling bindings (e.g., disable `Q` key)
  - Supports mode names: "normal", "insert", "visual" or full form "editor:normal"
  - Key sequences: `"<C-s>"`, `"<Leader>ff"`, `"jk"`, etc.
  - Command IDs: `"buffer:save"`, `"editor:cursor-down"`, etc.
  - Validation with warnings at startup (invalid entries reported, valid applied)
  - Part of Epic #353 mechanism/policy separation (Phase 4)

### Changed

- Moved Vim policy code from editor to vim module (Epic #353)
  - Resolvers: `VimNormalResolver`, `VimInsertResolver`, `VimOperatorPendingResolver` -> `modules/vim/src/resolvers/`
  - Visual mode: entry, exit, manipulation, operators -> `modules/vim/src/visual/`
  - `modules/editor/` now pure mechanism (only `ResolverRegistry`)
  - Import from `reovim_module_vim` instead of `reovim_module_editor`

- Unified resolver state management (#360)
  - Resolvers now own pending_count, pending_register, pending_keys state
  - `resolve_with_keymap()` queries keymap and applies Vim policy
  - EventLoop delegates to resolver for keymap queries (no legacy fallback)
  - VimNormalResolver and VimOperatorPendingResolver now use keymap-aware resolution
  - Part of Epic #353 mechanism/policy separation (Phase 3)

- E2E tests: Enable 9 more tests after upstream changes (#308)
  - operators.rs: 19 enabled (was 12), 9 ignored
  - search.rs: 5 enabled (was 3), 10 ignored
  - Updated ignore messages to reference specific issues

- Multi-client tests: Enable all 3 tests (#314)
  - Add `with_buffer()` to `MultiClientTest` for shared buffer setup
  - Fix "No active buffer" error in multi-client tests

### Fixed

- **Process separation for integrated mode** ([#331](https://github.com/ds1sqe/reovim/issues/331))
  - Server now spawns as separate OS process instead of tokio task
  - Fixes garbled terminal output when server logs conflict with TUI rendering
  - Added `--ready-signal` flag for process coordination
  - Server prints "READY <addr>" to stdout when bound

- Multi-client tests now properly initialize buffers before operations (#339)

### Removed

- Deprecated `char_wait` field and `CharWaitState` struct (#359)
  - Replaced by unified `pending_char` field with `PendingCharOp` enum
  - Removed backward compatibility shims in `set_pending_char()`/`take_pending_char()`
  - Removed deprecated `Session::set_char_wait()` method
  - Part of Epic #353 mechanism/policy separation cleanup

---

## Version History

- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
