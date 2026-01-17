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

### Changed

- E2E tests: Enable 9 more tests after upstream changes (#308)
  - operators.rs: 19 enabled (was 12), 9 ignored
  - search.rs: 5 enabled (was 3), 10 ignored
  - Updated ignore messages to reference specific issues

- Multi-client tests: Enable all 3 tests (#314)
  - Add `with_buffer()` to `MultiClientTest` for shared buffer setup
  - Fix "No active buffer" error in multi-client tests

### Fixed

### Removed

---

## Version History

- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
