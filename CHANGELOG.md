# Changelog

All notable changes to the **new architecture** (`lib/arch`, `lib/kernel`, `lib/drivers/*`) will be documented in this file.

For legacy crate changes (`lib/core`, `lib/sys`, plugins), see [CHANGELOG-archive.md](docs/CHANGELOG-archive.md).

## [Unreleased] - v0.9.0-dev

### Added

- **Phase 2.1: Memory Management Module** (Issue #159) - Kernel mm/ subsystem
  - `BufferId` - Unique buffer identifier with atomic counter generation
  - `Position` - Text position with `line` and `column` fields (0-indexed, usize)
  - `Cursor` - Cursor state with position, selection anchor, and preferred column
  - `Edit` - Insert/Delete operations with position for undo/redo support
  - `Buffer` - Line-based text storage with Vec<String> backend
    - Constructors: `new()`, `from_string()`, `with_id()`
    - Line access: `line()`, `line_count()`, `line_len()`, `lines()`, `content()`
    - Edit operations: `insert()`, `insert_at()`, `delete()`, `delete_at()`, `delete_range()`
    - Position conversion: `position_to_byte()`, `byte_to_position()` (for tree-sitter)
  - 59 unit tests + 6 doctests covering all functionality

- **Phase 1: Platform Traits and Unix Backend** (Issue #156) - Architecture layer implementation
  - Defined platform-agnostic traits in `lib/arch/src/traits.rs`:
    - `Terminal` trait - terminal I/O abstraction (size, raw mode, cursor, screen, mouse)
    - `InputSource` trait - input event polling and reading
    - `SignalHandler` trait - signal registration (resize, interrupt, suspend)
  - Platform-agnostic types: `TerminalSize`, `KeyEvent`, `KeyCode`, `Modifiers`, `MouseEvent`, `InputEvent`, `ClearType`, `RawModeGuard`
  - Unix backend (`lib/arch/src/unix/`) using crossterm:
    - `UnixTerminal` - full Terminal trait implementation
    - `UnixInputSource` - event polling and reading
    - `UnixSignalHandler` - signal handler placeholder (crossterm handles signals internally)
    - Type conversion functions from crossterm types
  - Windows stubs (`lib/arch/src/windows/`) - all methods return `todo!()`
  - Platform-specific type aliases: `PlatformTerminal`, `PlatformInputSource`, `PlatformSignalHandler`

- **Phase 0: Architecture Skeleton** (Issue #152) - Foundation for Linux-inspired architecture
  - Created `lib/arch/` - Platform abstraction layer (no dependencies)
  - Created `lib/kernel/` - Core mechanisms layer (depends only on arch)
  - Created `lib/drivers/` - 6 driver crates (display, input, syntax, lsp, net, vfs)
  - All crates are empty skeletons that compile with zero warnings
  - Dependency graph enforced: arch → kernel → drivers
  - `lib/drivers/syntax` has NO tree-sitter dependency (trait definitions only)

---

## Version History

- v0.9.x - New architecture (lib/arch, lib/kernel, lib/drivers/*)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](docs/CHANGELOG-archive.md)
