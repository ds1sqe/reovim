# Changelog

All notable changes to Reovim will be documented in this file.

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
