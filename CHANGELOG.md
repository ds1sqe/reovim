# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.1-dev

### Added

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
