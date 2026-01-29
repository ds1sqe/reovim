# Command System

The command system defines and executes all editor actions using a trait-based architecture.

## Overview

The command system is distributed across kernel, drivers, and modules:

```
server/lib/drivers/command/src/
├── lib.rs          # CommandTrait, CommandRegistry
├── id.rs           # CommandId type
├── context.rs      # ExecutionContext, OperatorContext
└── result.rs       # CommandResult variants

server/lib/drivers/command-types/src/
├── lib.rs          # Command type definitions
└── registration.rs # CommandRegistration, KeybindingRegistration

server/modules/vim/src/
├── resolvers/      # Mode-specific key resolvers
│   ├── normal.rs   # Normal mode commands (h/j/k/l, d/y/c)
│   ├── insert.rs   # Insert mode commands
│   └── visual.rs   # Visual mode commands
└── commands/       # Vim-specific command implementations

server/modules/commands/src/
├── cursor.rs       # Cursor movement (h/j/k/l, w/b/e)
├── operator.rs     # Operators (d/y/c)
├── text.rs         # Text editing (x, o/O)
└── ...             # Other command implementations
```

## CommandTrait

All commands implement this trait:

```rust
pub trait CommandTrait: Debug + Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult;
    fn clone_box(&self) -> Box<dyn CommandTrait>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn valid_modes(&self) -> Option<Vec<ModeState>> { None }
    fn supports_count(&self) -> bool { true }  // Most commands support count
    fn is_jump(&self) -> bool { false }
    fn is_text_modifying(&self) -> bool { false }
    fn resulting_mode(&self) -> Option<ModeState> { None }
}
```

**Trait Methods:**
- `name()` - Unique identifier for the command
- `description()` - Human-readable description
- `execute()` - Performs the command action
- `clone_box()` - Creates a boxed clone (for trait object cloning)
- `as_any()` - Downcast support for type-specific operations
- `valid_modes()` - Optional mode restrictions
- `supports_count()` - Whether command accepts repeat count (e.g., `5j`) - default: `true`
- `is_jump()` - Whether to record position in jump list
- `is_text_modifying()` - Whether command modifies buffer text (for undo grouping)
- `resulting_mode()` - Mode to enter after command execution (if any)

## ExecutionContext

Context passed to command execution:

```rust
pub struct ExecutionContext<'a> {
    pub buffer: &'a mut Buffer,
    pub count: Option<usize>,
    pub buffer_id: usize,
    pub window_id: usize,
    pub operator_context: Option<OperatorContext>,
}
```

**Fields:**
- `buffer` - Mutable reference to target buffer
- `count` - Optional repeat count from key sequence
- `buffer_id` - Target buffer identifier
- `window_id` - Target window (for multi-window support)
- `operator_context` - Context for operator-pending mode (motion target, etc.)

## CommandResult

Results returned by command execution:

```rust
pub enum CommandResult {
    Success,
    NeedsRender,
    ModeChange(ModeState),
    Quit,
    ClipboardWrite {
        text: String,
        register: Option<char>,
        mode_change: Option<ModeState>,
        yank_type: Option<YankType>,
        yank_range: Option<(Position, Position)>,
    },
    DeferToRuntime(DeferredAction),
    Deferred(Box<dyn DeferredActionHandler>),
    EmitEvent(DynEvent),
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
| `ClipboardWrite` | Store text in register with optional mode change |
| `DeferToRuntime(action)` | Dispatch to runtime handler |
| `Deferred(handler)` | Execute handler with runtime access |
| `EmitEvent(event)` | Emit event via EventBus (for plugin commands) |
| `Error(msg)` | Display error message |

## DeferredAction

Actions that require Runtime access (can't be done with just buffer):

```rust
pub enum DeferredAction {
    Paste { before: bool, register: Option<char> },
    CommandLine(CommandLineAction),
    JumpOlder,
    JumpNewer,
    OperatorMotion(OperatorMotionAction),
    Window(WindowAction),
    Tab(TabAction),
    Buffer(BufferAction),
    File(FileAction),
}
```

These are dispatched to specialized handlers in the runtime.

> **Note:** Plugin actions (Completion, Explorer, Microscope) use `CommandResult::EmitEvent(DynEvent)` instead of `DeferredAction`. This decouples plugins from core.

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

### Range-Finder (Jump + Folding)

**Jump Navigation:**
| Command | Key | Description |
|---------|-----|-------------|
| `jump_search` | s | Multi-char search (2 chars + label) |
| `jump_find_char` | f | Enhanced find char (with labels) |
| `jump_find_char_back` | F | Enhanced find char backward |
| `jump_till_char` | t | Enhanced till char (with labels) |
| `jump_till_char_back` | T | Enhanced till char backward |

Jump features:
- Smart auto-jump: 1 match → instant, 2-676 → labels
- Works with operators: `d` + `s` + pattern + label = delete range
- Label priority: home row keys (s, f, n, j, k...)

**Folding:**
| Command | Key | Description |
|---------|-----|-------------|
| `fold_toggle` | za | Toggle fold at cursor line |
| `fold_open` | zo | Open (expand) fold at cursor |
| `fold_close` | zc | Close (collapse) fold at cursor |
| `fold_open_all` | zR | Open all folds in buffer |
| `fold_close_all` | zM | Close all folds in buffer |

Folds are computed from treesitter queries when a buffer is opened.
- Collapsed folds show a marker: `+-- 15 lines: fn example() ----`
- Folded lines are hidden from display

### Completion (5)
| Command | Key | Description |
|---------|-----|-------------|
| `completion_trigger` | Alt-Space | Show completion menu |
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

### Microscope (20+)

**Note:** Microscope is currently archived. See `archive/post_kernel/modules/` for reference.

**Pickers:**
| Command | Key | Description |
|---------|-----|-------------|
| `microscope_find_files` | Space ff | Find files |
| `microscope_find_buffers` | Space fb | Find open buffers |
| `microscope_live_grep` | Space fg | Live grep search |
| `microscope_find_recent` | Space fr | Recent files |
| `microscope_commands` | Space fc | Command palette |
| `microscope_help` | Space fh | Help tags |
| `microscope_keymaps` | Space fk | Show keymaps |
| `microscope_themes` | - | Theme picker |
| `microscope_profiles` | - | Profile picker |

**Navigation:**
| Command | Key | Description |
|---------|-----|-------------|
| `microscope_select_next` | j / Ctrl-n | Select next item |
| `microscope_select_prev` | k / Ctrl-p | Select previous item |
| `microscope_page_down` | Ctrl-d | Page down |
| `microscope_page_up` | Ctrl-u | Page up |
| `microscope_goto_first` | gg | Go to first item |
| `microscope_goto_last` | G | Go to last item |

**Actions:**
| Command | Key | Description |
|---------|-----|-------------|
| `microscope_confirm` | Enter | Confirm selection |
| `microscope_close` | Esc / q | Close microscope |
| `microscope_enter_insert` | i / a | Enter insert mode (for query) |
| `microscope_enter_normal` | Esc | Enter normal mode (for navigation) |
| `microscope_backspace` | Backspace | Delete char from query |
| `microscope_clear_query` | Ctrl-u | Clear entire query |
| `microscope_delete_word` | Ctrl-w | Delete word from query |

## Ex-Commands

Parsed from `:` command line input:

```rust
pub enum ExCommand {
    Quit,                              // :q or :quit
    Write { filename: Option<String> }, // :w [file] or :write
    WriteQuit,                          // :wq
    Edit { filename: String },          // :e file or :edit
    Set { option: SetOption },          // :set option
    Plugin { command: String },         // Plugin-registered commands
}

pub enum SetOption {
    Number(bool),          // :set number / :set nonumber
    RelativeNumber(bool),  // :set relativenumber / :set norelativenumber
    ColorMode(ColorMode),  // :set colormode=ansi|256|truecolor
}
```

### Plugin-Registered Ex-Commands

Plugins can register custom ex-commands dynamically using the `ExCommandRegistry`. When a command is not recognized as a built-in command, it's dispatched via the registry.

**Example plugin commands:**
- `:settings` - Opens the settings menu (registered by settings-menu plugin)
- `:profile list` - Lists available profiles (registered by profiles plugin)
- `:profile load <name>` - Loads a profile (registered by profiles plugin)
- `:profile save <name>` - Saves current settings (registered by profiles plugin)

**For plugin authors:**

See [Module System](../modules/overview.md) for complete documentation on:
- Zero-arg commands (`:mycommand`)
- Single-arg commands (`:mycommand arg`)
- Subcommand pattern (`:mycommand subcommand [arg]`)
- Event-based dispatch
- Best practices

**Command flow for plugin ex-commands:**

```
User types :settings
    ↓
ExCommand::parse() → ExCommand::Plugin { command: "settings" }
    ↓
Runtime handler → ex_command_registry.dispatch("settings")
    ↓
Returns DynEvent (SettingsMenuOpen)
    ↓
EventBus.dispatch() → Settings plugin opens
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
       └── DeferToRuntime → dispatch to runtime handler:
           ├── Paste → retrieve from registers
           ├── CommandLine → parse and execute ex-command
           ├── Window → window management operations
           ├── Tab → tab management operations
           ├── Buffer → buffer management operations
           ├── File → file operations
           ├── JumpOlder/Newer → jump list navigation
           └── OperatorMotion → execute operator+motion
      └── EmitEvent → dispatch to EventBus:
           ├── Completion events → completion plugin
           ├── Explorer events → explorer plugin
           ├── Microscope events → fuzzy finder plugin
           └── Custom plugin events → plugin handlers
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

## Command Declaration Macros

To reduce boilerplate, reovim provides macros for common command patterns. These macros are especially useful for plugin commands that emit events to the event bus.

### Available Macros

#### `declare_event_command!` - Unified Type (Recommended)

**The modern pattern:** Single type serves as both command and event.

```rust
use reovim_core::declare_event_command;

// Single unified type - both command AND event
declare_event_command! {
    Refresh,
    id: "refresh",
    description: "Refresh the view",
}

// Register as command
ctx.register_command(Refresh);

// Subscribe as event (same type!)
bus.subscribe::<Refresh, _>(100, |event, ctx| {
    // Handle refresh
    EventResult::Handled
});
```

**Benefits:**
- 50% less types (one instead of two)
- ~20 lines of boilerplate eliminated per command
- Clearer intent - action is both trigger and notification
- Zero-cost abstraction (zero-sized type)

**When to use:**
- ✅ Simple actions that don't need separate representations
- ✅ Most plugin commands (navigation, toggles, actions)
- ✅ Commands that just trigger an action in the event handler
- ✅ Preferred over separate Command/Event types

#### `declare_counted_event_command!` - Unified With Count (Recommended)

**The modern pattern for counted commands:**

```rust
use reovim_core::declare_counted_event_command;

// Single type with count - both command AND event
declare_counted_event_command! {
    MoveDown,
    id: "move_down",
    description: "Move down by count lines",
}

// The macro generates:
// - MoveDown { count: usize }
// - MoveDown::new(count: usize)
// - impl Default (count = 1)
// - impl CommandTrait (emits self with count from context)
// - impl Event

// Use it
ctx.register_command(MoveDown);

bus.subscribe::<MoveDown, _>(100, |event, ctx| {
    // Access event.count directly
    for _ in 0..event.count {
        // Move down
    }
    EventResult::Handled
});
```

### Migration Pattern

**Old style (separate types):**

```rust
// commands.rs (20+ lines)
pub struct ExplorerRefreshCommand;

impl CommandTrait for ExplorerRefreshCommand {
    fn name(&self) -> &'static str { "explorer_refresh" }
    fn description(&self) -> &'static str { "Refresh explorer view" }
    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerRefreshEvent))
    }
    fn clone_box(&self) -> Box<dyn CommandTrait> { Box::new(self.clone()) }
    fn as_any(&self) -> &dyn Any { self }
}

// events.rs (5+ lines)
#[derive(Debug, Clone, Copy, Default)]
pub struct ExplorerRefreshEvent;
impl Event for ExplorerRefreshEvent {}

// lib.rs - register separately
ctx.register_command(ExplorerRefreshCommand);
bus.subscribe::<ExplorerRefreshEvent, _>(100, |event, ctx| { ... });
```

**New style (unified):**

```rust
// commands.rs (4 lines)
declare_event_command! {
    ExplorerRefresh,
    id: "explorer_refresh",
    description: "Refresh explorer view",
}

// lib.rs - single type for both!
ctx.register_command(ExplorerRefresh);
bus.subscribe::<ExplorerRefresh, _>(100, |event, ctx| { ... });
```

**Savings:** ~21 lines eliminated, one type instead of two.

### When Commands Need ExecutionContext Data

Some commands need to extract data from the execution context (buffer state, cursor position, etc.) to create their events. For these, the unified macros may not work directly.

**Option 1: Custom execute() with unified event**

```rust
// Define unified type manually
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldToggle {
    pub buffer_id: usize,
    pub line: u32,
}

impl Event for FoldToggle {}

impl CommandTrait for FoldToggle {
    fn name(&self) -> &'static str { "fold_toggle" }
    fn description(&self) -> &'static str { "Toggle fold at cursor" }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Extract context data
        let event = Self {
            buffer_id: ctx.buffer_id,
            line: u32::from(ctx.buffer.cur.y),
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> { Box::new(*self) }
    fn as_any(&self) -> &dyn Any { self }
}
```

**Option 2: Builder pattern (for complex data)**

```rust
#[derive(Debug, Clone)]
pub struct ComplexAction {
    pub data: Vec<String>,
    // ... complex fields
}

impl ComplexAction {
    pub fn from_context(ctx: &ExecutionContext) -> Self {
        Self {
            data: extract_from_buffer(&ctx.buffer),
        }
    }
}

impl Event for ComplexAction {}

// Still use macro for command shell
impl CommandTrait for ComplexActionCommand {
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = ComplexAction::from_context(ctx);
        CommandResult::EmitEvent(DynEvent::new(event))
    }
    // ...
}
```

### Best Practices

1. **Prefer unified types** (`declare_event_command!`) for simple actions
2. **Use counted variant** when commands need repeat counts
3. **Custom impl** only when you need complex context extraction
4. **Consistent naming**: Drop "Command" and "Event" suffixes for unified types
   - Old: `ExplorerRefreshCommand` + `ExplorerRefreshEvent`
   - New: `ExplorerRefresh` (serves both roles)

5. **Event handlers don't need Default**: Unified commands only need `Default` for command instantiation, handlers receive fully formed events

### Example: Full Plugin Migration

**Before:**
```rust
// 5 separate event types
pub struct CursorUpEvent;
pub struct CursorDownEvent;
pub struct PageUpEvent;
pub struct PageDownEvent;
pub struct RefreshEvent;

// 5 separate command types (100+ lines total)
pub struct CursorUpCommand;
pub struct CursorDownCommand;
pub struct PageUpCommand;
pub struct PageDownCommand;
pub struct RefreshCommand;

// Registration
ctx.register_command(CursorUpCommand);
ctx.register_command(CursorDownCommand);
// ... etc

// Subscriptions
bus.subscribe::<CursorUpEvent, _>(...);
bus.subscribe::<CursorDownEvent, _>(...);
// ... etc
```

**After:**
```rust
// 5 unified types (20 lines total)
declare_event_command!(CursorUp, id: "cursor_up", description: "Move cursor up");
declare_event_command!(CursorDown, id: "cursor_down", description: "Move cursor down");
declare_event_command!(PageUp, id: "page_up", description: "Page up");
declare_event_command!(PageDown, id: "page_down", description: "Page down");
declare_event_command!(Refresh, id: "refresh", description: "Refresh view");

// Registration (same type!)
ctx.register_command(CursorUp);
ctx.register_command(CursorDown);
// ... etc

// Subscriptions (same type!)
bus.subscribe::<CursorUp, _>(...);
bus.subscribe::<CursorDown, _>(...);
// ... etc
```

**Result:** ~80% reduction in boilerplate, 50% fewer types, identical functionality.

### Deprecated Macros (Legacy)

#### `declare_command!` - Separate Command/Event (Deprecated)

**This pattern is deprecated.** Use `declare_event_command!` instead.

```rust
// DON'T USE - deprecated pattern
use reovim_core::declare_command;

// Define event first
#[derive(Debug, Clone, Copy, Default)]
pub struct RefreshEvent;
impl Event for RefreshEvent {}

// Declare command that emits this event
declare_command! {
    RefreshCommand => RefreshEvent,
    name: "refresh",
    description: "Refresh the view",
}
```

**Migration:** Replace with unified type using `declare_event_command!`

#### `declare_counted_command!` - Separate with Count (Deprecated)

**This pattern is deprecated.** Use `declare_counted_event_command!` instead.

```rust
// DON'T USE - deprecated pattern
use reovim_core::declare_counted_command;

// Event with count
#[derive(Debug, Clone)]
pub struct MoveDownEvent {
    pub count: usize,
}

impl MoveDownEvent {
    pub fn new(count: usize) -> Self { Self { count } }
}

impl Event for MoveDownEvent {}

// Command that extracts count from context
declare_counted_command! {
    MoveDownCommand => MoveDownEvent,
    name: "move_down",
    description: "Move down by count lines",
}
```

**Migration:** Replace with unified type using `declare_counted_event_command!`
```
