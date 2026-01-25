# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.2-dev

### Added

### Changed

- **BREAKING**: `REOVIM_MODULE_PATH` now prepends to search paths instead of
  appending. This gives the environment variable highest priority, matching
  the convention of `LD_LIBRARY_PATH`, `PYTHONPATH`, etc. Users who relied
  on the previous append behavior should adjust their module organization. (#433)

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

### Removed

---

## Version History

- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
