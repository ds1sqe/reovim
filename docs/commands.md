# Command System

The command system defines and executes all editor actions using a trait-based architecture.

## Overview

```
lib/core/src/command/
├── mod.rs          # Re-exports, CommandRef
├── id.rs           # CommandId constants
├── traits.rs       # CommandTrait, ExecutionContext, CommandResult
├── registry.rs     # CommandRegistry
├── builtin/        # Built-in command implementations
│   ├── mod.rs
│   ├── cursor.rs
│   ├── mode.rs
│   ├── text.rs
│   ├── visual.rs
│   ├── clipboard.rs
│   ├── command_line.rs
│   ├── completion.rs
│   ├── explorer.rs
│   ├── telescope.rs
│   ├── leap.rs
│   ├── operator.rs
│   ├── jump.rs
│   └── history.rs
└── deferred.rs     # DeferredAction enum
```

## CommandTrait

All commands implement this trait:

```rust
pub trait CommandTrait: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult;
    fn clone_box(&self) -> Box<dyn CommandTrait>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn valid_modes(&self) -> Option<Vec<ModeState>> { None }
    fn supports_count(&self) -> bool { false }
    fn is_jump(&self) -> bool { false }
}
```

**Trait Methods:**
- `name()` - Unique identifier for the command
- `description()` - Human-readable description
- `execute()` - Performs the command action
- `clone_box()` - Creates a boxed clone (for trait object cloning)
- `as_any()` - Downcast support for type-specific operations
- `valid_modes()` - Optional mode restrictions
- `supports_count()` - Whether command accepts repeat count (e.g., `5j`)
- `is_jump()` - Whether to record position in jump list

## ExecutionContext

Context passed to command execution:

```rust
pub struct ExecutionContext<'a> {
    pub buffer: &'a mut Buffer,
    pub count: Option<usize>,
    pub buffer_id: usize,
    pub window_id: usize,
}
```

**Fields:**
- `buffer` - Mutable reference to target buffer
- `count` - Optional repeat count from key sequence
- `buffer_id` - Target buffer identifier
- `window_id` - Target window (for multi-window support)

## CommandResult

Results returned by command execution:

```rust
pub enum CommandResult {
    Success,
    NeedsRender,
    ModeChange(ModeState),
    Quit,
    ClipboardWrite { text: String, register: char },
    DeferToRuntime(DeferredAction),
    Error(String),
}
```

**Result Handling:**
| Result | Runtime Action |
|--------|----------------|
| `Success` | No action needed |
| `NeedsRender` | Trigger screen render |
| `ModeChange(mode)` | Update runtime mode and render |
| `Quit` | Exit editor |
| `ClipboardWrite` | Store text in specified register |
| `DeferToRuntime(action)` | Dispatch to runtime handler |
| `Error(msg)` | Log warning message |

## DeferredAction

Actions that require Runtime access (can't be done with just buffer):

```rust
pub enum DeferredAction {
    Paste { before: bool, register: char },
    CommandLine(CommandLineAction),
    Completion(CompletionAction),
    Explorer(ExplorerAction),
    Telescope(TelescopeAction),
    JumpOlder,
    JumpNewer,
    OperatorMotion(OperatorMotionAction),
    Leap(LeapAction),
}
```

These are dispatched to specialized handlers in the runtime.

## CommandRegistry

Thread-safe registry for command lookup:

```rust
pub struct CommandRegistry {
    commands: RwLock<HashMap<CommandId, Arc<dyn CommandTrait>>>,
}

impl CommandRegistry {
    pub fn with_defaults() -> Self;
    pub fn register(&self, id: CommandId, cmd: Box<dyn CommandTrait>);
    pub fn get(&self, id: &CommandId) -> Option<Arc<dyn CommandTrait>>;
}
```

**Features:**
- Thread-safe via `RwLock`
- All builtins registered via `with_defaults()`
- Supports runtime registration of custom commands
- Commands stored as `Arc<dyn CommandTrait>` for efficient sharing

## CommandId

String-based identifiers for commands:

```rust
pub struct CommandId(&'static str);

impl CommandId {
    pub const CURSOR_UP: Self = Self::new("cursor_up");
    pub const CURSOR_DOWN: Self = Self::new("cursor_down");
    // ... 129 total constants
}
```

## Built-in Commands

### Cursor Movement (10)
| Command | Key | Description |
|---------|-----|-------------|
| `cursor_up` | k | Move cursor up |
| `cursor_down` | j | Move cursor down |
| `cursor_left` | h | Move cursor left |
| `cursor_right` | l | Move cursor right |
| `cursor_line_start` | 0 | Move to line start |
| `cursor_line_end` | $ | Move to line end |
| `cursor_word_forward` | w | Move to next word |
| `cursor_word_backward` | b | Move to previous word |
| `goto_first_line` | gg | Go to first line (or line N with count) |
| `goto_last_line` | G | Go to last line (or line N with count) |

### Mode Switching (9)
| Command | Key | Description |
|---------|-----|-------------|
| `enter_normal_mode` | Esc | Switch to normal mode |
| `enter_insert_mode` | i | Switch to insert mode |
| `enter_insert_mode_after` | a | Insert after cursor |
| `enter_insert_mode_eol` | A | Insert at end of line |
| `open_line_below` | o | Open line below and insert |
| `open_line_above` | O | Open line above and insert |
| `enter_visual_mode` | v | Switch to visual mode |
| `enter_visual_block_mode` | Ctrl-v | Switch to visual block mode |
| `enter_command_mode` | : | Switch to command mode |

### Text Operations (7)
| Command | Key | Description |
|---------|-----|-------------|
| `delete_char_forward` | x | Delete character under cursor |
| `delete_char_backward` | Backspace | Delete character before cursor |
| `delete_line` | dd | Delete current line |
| `yank_line` | yy | Copy current line |
| `yank_to_end` | Y | Copy from cursor to end of line |
| `insert_newline` | Enter | Insert newline |
| `undo` / `redo` | u / Ctrl-r | Undo / Redo |

### Operators (3)
| Command | Key | Description |
|---------|-----|-------------|
| `enter_delete_operator` | d | Enter delete operator-pending mode |
| `enter_yank_operator` | y | Enter yank operator-pending mode |
| `enter_change_operator` | c | Enter change operator-pending mode |

Operators combine with motions: `d` + `w` = delete word, `y` + `$` = yank to end of line.

### Leap (3)
| Command | Key | Description |
|---------|-----|-------------|
| `leap_forward` | s | Start forward leap (2-char jump) |
| `leap_backward` | S | Start backward leap |
| `leap_cancel` | Esc | Cancel active leap |

### Completion (5)
| Command | Key | Description |
|---------|-----|-------------|
| `completion_trigger` | Ctrl-Space | Show completion menu |
| `completion_next` | Ctrl-n | Select next completion item |
| `completion_prev` | Ctrl-p | Select previous completion item |
| `completion_confirm` | Tab | Insert selected completion |
| `completion_dismiss` | Ctrl-e | Close completion menu |

### Visual Mode (6)
| Command | Key | Description |
|---------|-----|-------------|
| `visual_extend_up` | k | Extend selection up |
| `visual_extend_down` | j | Extend selection down |
| `visual_extend_left` | h | Extend selection left |
| `visual_extend_right` | l | Extend selection right |
| `visual_delete` | d | Delete selection |
| `visual_yank` | y | Copy selection |

### Clipboard (2)
| Command | Key | Description |
|---------|-----|-------------|
| `paste` | p | Paste after cursor |
| `paste_before` | P | Paste before cursor |

### Jump List (2)
| Command | Key | Description |
|---------|-----|-------------|
| `jump_older` | Ctrl-o | Jump to older position |
| `jump_newer` | Ctrl-i | Jump to newer position |

### Explorer (25)

**Navigation:**
| Command | Key | Description |
|---------|-----|-------------|
| `explorer_cursor_up` | k | Move cursor up |
| `explorer_cursor_down` | j | Move cursor down |
| `explorer_page_up` | Ctrl-u | Page up |
| `explorer_page_down` | Ctrl-d | Page down |
| `explorer_goto_first` | gg | Go to first item |
| `explorer_goto_last` | G | Go to last item |

**Tree Operations:**
| Command | Key | Description |
|---------|-----|-------------|
| `explorer_toggle_node` | o | Toggle expand/collapse |
| `explorer_open_node` | Enter | Open file or directory |
| `explorer_close_parent` | x | Close parent directory |
| `explorer_go_to_parent` | u | Go to parent directory |
| `explorer_refresh` | R | Refresh file tree |

**File Operations:**
| Command | Key | Description |
|---------|-----|-------------|
| `explorer_create_file` | a | Create new file |
| `explorer_create_dir` | A | Create new directory |
| `explorer_rename` | r | Rename file/directory |
| `explorer_delete` | d | Delete file/directory |

**Window:**
| Command | Key | Description |
|---------|-----|-------------|
| `toggle_explorer` | Space e | Toggle explorer visibility |
| `explorer_close` | q | Close explorer |
| `explorer_focus_editor` | Tab/Esc | Focus back to editor |

### Telescope (18)

**Pickers:**
| Command | Key | Description |
|---------|-----|-------------|
| `telescope_find_files` | Space ff | Find files |
| `telescope_find_buffers` | Space fb | Find open buffers |
| `telescope_live_grep` | Space fg | Live grep search |
| `telescope_recent` | Space fr | Recent files |
| `telescope_commands` | Space fc | Command palette |
| `telescope_help` | Space fh | Help tags |
| `telescope_keymaps` | Space fk | Show keymaps |

**Navigation:**
| Command | Key | Description |
|---------|-----|-------------|
| `telescope_next` | j / Ctrl-n | Select next item |
| `telescope_prev` | k / Ctrl-p | Select previous item |
| `telescope_page_down` | Ctrl-d | Page down |
| `telescope_page_up` | Ctrl-u | Page up |
| `telescope_goto_first` | gg | Go to first item |
| `telescope_goto_last` | G | Go to last item |

**Actions:**
| Command | Key | Description |
|---------|-----|-------------|
| `telescope_confirm` | Enter | Confirm selection |
| `telescope_close` | Esc | Close telescope |
| `telescope_enter_insert` | i | Enter insert mode (for query) |
| `telescope_enter_normal` | Esc | Enter normal mode (for navigation) |

## Ex-Commands

Parsed from `:` command line input:

```rust
pub enum ExCommand {
    Quit,                              // :q or :quit
    Write { filename: Option<String> }, // :w [file] or :write
    WriteQuit,                          // :wq
    Edit { filename: String },          // :e file or :edit
    Set { option: SetOption },          // :set option
    Unknown(String),
}

pub enum SetOption {
    Number(bool),          // :set number / :set nonumber
    RelativeNumber(bool),  // :set relativenumber / :set norelativenumber
    ColorMode(ColorMode),  // :set colormode=ansi|256|truecolor
}
```

## Command Flow

```
1. KeyEvent received
       │
       ▼
2. CommandHandler looks up key in keymap
       │
       ▼
3. Key found → Get CommandRef (by ID or inline)
       │
       ▼
4. Resolve CommandRef to Arc<dyn CommandTrait> via Registry
       │
       ▼
5. Create ExecutionContext with buffer, count, ids
       │
       ▼
6. Call cmd.execute(&mut ctx)
       │
       ▼
7. Process CommandResult:
       │
       ├── NeedsRender → render()
       ├── ModeChange → set_mode() + render()
       ├── Quit → exit loop
       ├── ClipboardWrite → store in registers
       └── DeferToRuntime → dispatch to handler:
           ├── Paste → retrieve from registers
           ├── CommandLine → parse and execute ex-command
           ├── Completion → trigger/update/confirm completion
           ├── Explorer → explorer navigation/operations
           ├── Telescope → fuzzy finder operations
           ├── JumpOlder/Newer → jump list navigation
           ├── OperatorMotion → execute operator+motion
           └── Leap → leap motion execution
```

## Adding New Commands

1. Add `CommandId` constant in `id.rs`
2. Create command struct implementing `CommandTrait` in `builtin/`
3. Register in `CommandRegistry::with_defaults()`
4. Add key binding in `bind/mod.rs`

Example:

```rust
// id.rs
pub const MY_COMMAND: Self = Self::new("my_command");

// builtin/my_command.rs
pub struct MyCommand;

impl CommandTrait for MyCommand {
    fn name(&self) -> &'static str { "my_command" }
    fn description(&self) -> &'static str { "Does something useful" }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Modify ctx.buffer...
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(Self)
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

// registry.rs (in with_defaults)
registry.register(CommandId::MY_COMMAND, Box::new(MyCommand));

// bind/mod.rs (in normal map setup)
("mc", CommandRef::ById(CommandId::MY_COMMAND)),
```
