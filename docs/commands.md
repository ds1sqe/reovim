# Command System

The command system defines and executes all editor actions.

## Overview

```
lib/core/src/command/
├── mod.rs          # Re-exports
├── action.rs       # Command enum
├── context.rs      # CommandContext
└── executor.rs     # BufferCommandExecutor
```

## Command Enum

All editor actions are defined in the `Command` enum:

```rust
pub enum Command {
    // Cursor Movement
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorLineStart,
    CursorLineEnd,
    CursorWordForward,
    CursorWordBackward,
    GotoFirstLine,
    GotoLastLine,

    // Mode Switching
    EnterNormalMode,
    EnterInsertMode,
    EnterInsertModeAfter,
    EnterInsertModeEndOfLine,
    OpenLineBelow,
    OpenLineAbove,
    EnterVisualMode,
    EnterCommandMode,

    // Text Operations
    InsertChar(char),
    DeleteCharBackward,
    DeleteCharForward,
    DeleteLine,

    // Visual Mode
    VisualExtendUp,
    VisualExtendDown,
    VisualExtendLeft,
    VisualExtendRight,
    VisualDelete,
    VisualYank,

    // Command Line
    CommandLineChar(char),
    CommandLineBackspace,
    CommandLineExecute,
    CommandLineCancel,

    // Clipboard
    Paste,
    PasteBefore,

    // System
    Quit,
    Noop,
}
```

## CommandContext

Execution context passed with each command:

```rust
pub struct CommandContext {
    pub buffer_id: usize,
    pub window_id: usize,
    pub count: Option<usize>,  // Repeat count (e.g., 5j)
}
```

**Usage:**
- `buffer_id`: Target buffer for the command
- `window_id`: Target window (for future multi-window support)
- `count`: Optional repeat count from key sequence

## CommandResult

Results returned by command execution:

```rust
pub enum CommandResult {
    Success,
    NeedsRender,
    ModeChange(Mod),
    VisualDeleteResult(String),
    VisualYankResult(String),
    CommandLineCommand,
    PasteCommand,
    Quit,
    Error(String),
}
```

**Result Handling:**
| Result | Runtime Action |
|--------|----------------|
| `Success` | No action needed |
| `NeedsRender` | Trigger screen render |
| `ModeChange(mode)` | Update runtime mode |
| `VisualDeleteResult(text)` | Store in clipboard, render |
| `VisualYankResult(text)` | Store in clipboard |
| `CommandLineCommand` | Parse and execute ex-command |
| `PasteCommand` | Paste from runtime clipboard |
| `Quit` | Exit editor |
| `Error(msg)` | Display error message |

## BufferCommandExecutor

Static executor for buffer commands:

```rust
impl BufferCommandExecutor {
    pub fn execute_on_buffer(
        buffer: &mut Buffer,
        command: Command,
        context: CommandContext,
    ) -> CommandResult {
        match command {
            // Handle each command...
        }
    }
}
```

### Execution Examples

**Cursor Movement:**
```rust
Command::CursorDown => {
    let count = context.count.unwrap_or(1);
    let new_pos = Motion::Down.apply(buffer, count);
    buffer.cur = new_pos;
    CommandResult::NeedsRender
}
```

**Mode Change:**
```rust
Command::EnterInsertMode => {
    CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
}
```

**Text Operation:**
```rust
Command::InsertChar(c) => {
    buffer.insert_char(c);
    CommandResult::NeedsRender
}
```

**Visual Mode:**
```rust
Command::VisualDelete => {
    let text = buffer.get_selected_text();
    buffer.delete_selection();
    CommandResult::VisualDeleteResult(text)
}
```

## Motion System

Cursor movements are calculated via the Motion enum:

```rust
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    WordForward,
    WordBackward,
    DocumentStart,
    DocumentEnd,
}

impl Motion {
    pub fn apply(&self, buffer: &Buffer, count: usize) -> Position {
        // Returns new position without modifying buffer
    }
}
```

**Separation of concerns:**
- Motion calculates target position
- Executor updates buffer state
- Result triggers render

## Ex-Commands

Command-line commands parsed from `:` input:

```rust
pub enum ExCommand {
    Quit,                           // :q
    Write { filename: Option<String> },  // :w [file]
    WriteQuit,                      // :wq
    Set { option: SetOption },      // :set option
    Unknown(String),
}

pub enum SetOption {
    Number(bool),          // :set number / :set nonumber
    RelativeNumber(bool),  // :set relativenumber
}
```

### CommandLine State

```rust
pub struct CommandLine {
    pub input: String,    // Current command text
    pub cursor: usize,    // Cursor position
    pub active: bool,     // In command mode?
}
```

**Methods:**
- `activate()` - Enter command mode
- `insert_char(c)` - Insert at cursor
- `delete_char()` - Backspace
- `execute()` - Parse and return ExCommand
- `clear()` - Reset state

## Command Flow

```
1. KeyEvent received
       │
       ▼
2. CommandHandler looks up key in keymap
       │
       ▼
3. Command found → Create CommandContext
       │
       ▼
4. Send CommandEvent to runtime
       │
       ▼
5. Runtime calls BufferCommandExecutor::execute_on_buffer()
       │
       ▼
6. Match on Command variant
       │
       ├── Cursor movement → apply Motion
       ├── Text operation → modify buffer
       ├── Mode change → return ModeChange result
       └── System command → return Quit/Error
       │
       ▼
7. Return CommandResult
       │
       ▼
8. Runtime handles result
       │
       ├── NeedsRender → Screen::render()
       ├── ModeChange → update runtime.current_mode
       ├── VisualDelete/Yank → store in clipboard
       └── Quit → exit loop
```

## Adding New Commands

1. Add variant to `Command` enum in `action.rs`
2. Add match arm in `BufferCommandExecutor::execute_on_buffer()`
3. Add key binding in `bind/mod.rs`
4. Handle any new `CommandResult` variants in runtime

Example adding a "delete word" command:

```rust
// action.rs
pub enum Command {
    // ...
    DeleteWord,
}

// executor.rs
Command::DeleteWord => {
    let word_end = Motion::WordForward.apply(buffer, 1);
    // Delete from cursor to word_end
    buffer.delete_range(buffer.cur, word_end);
    CommandResult::NeedsRender
}

// bind/mod.rs (in nmap initialization)
("dw", Command::DeleteWord),
```
