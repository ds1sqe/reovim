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

### Changed

### Fixed

### Removed

---

## Version History

- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
