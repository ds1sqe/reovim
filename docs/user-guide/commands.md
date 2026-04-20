# Command System

The command system defines and executes all editor actions using a trait-based architecture.

## Overview

The command system is distributed across drivers and modules:

```
ext/server/drivers/command/src/
├── lib.rs          # Re-exports, CommandHandlerStore
├── traits.rs       # Command, CommandHandler traits
├── registry.rs     # CommandHandlerStore
├── provider.rs     # CommandProvider trait
├── query.rs        # CommandQueryService, CommandInfo
├── ex_handler.rs   # ExCommandHandler trait, ExCommandContext
├── ex_dispatch.rs  # ExCommandRegistry, ExCommandDispatcher
└── ex_registry.rs  # ExCommandHandlerStore

ext/server/drivers/command-types/src/
├── lib.rs          # Re-exports
├── args.rs         # ArgSpec, ArgKind, ArgValue
├── context.rs      # CommandContext
├── result.rs       # CommandResult variants
└── motion.rs       # MotionType

server/modules/vim/src/
├── resolvers/      # Mode-specific key resolvers
│   ├── normal.rs   # Normal mode commands (h/j/k/l, d/y/c)
│   ├── insert.rs   # Insert mode commands
│   └── visual.rs   # Visual mode commands
└── commands/       # Vim-specific command implementations

server/modules/commands/src/
└── ...             # Ex-command implementations (:w, :q, :e, :wq)
```

## Command and CommandHandler

Commands use two traits defined in `ext/server/drivers/command/src/traits.rs`:

### Command Trait (Metadata)

```rust
pub trait Command: Send + Sync + 'static {
    fn id(&self) -> CommandId;
    fn description(&self) -> &'static str;
    fn args(&self) -> Vec<ArgSpec> { vec![] }
    fn names(&self) -> &[&'static str] { &[] }
}
```

**Trait Methods:**
- `id()` - Unique `CommandId` (composed of `ModuleId` + command name)
- `description()` - Human-readable description
- `args()` - Argument specifications (`ArgSpec` with `ArgKind`) - default: empty
- `names()` - Ex-command aliases (e.g., `["w", "write"]`) - default: empty

### CommandHandler Trait (Execution)

```rust
pub trait CommandHandler: Command {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult;
}
```

**Trait Methods:**
- `execute()` - Performs the command action using `SessionRuntime` APIs

Commands receive a `SessionRuntime` which provides access to:
- `ModeApi` - Mode stack operations (push, pop, set)
- `BufferApi` - Buffer content and cursor operations
- `WindowApi` - Window management and focus
- `ExtensionApi` - Per-session module state
- `ChangeTracker` - State change accumulation

## CommandContext

Context carrying all command inputs, defined in `ext/server/drivers/command-types/src/context.rs`:

```rust
pub struct CommandContext {
    args: HashMap<&'static str, ArgValue>,
    vfs: Option<Arc<dyn VfsDriver>>,
    mode_name: Option<String>,
}
```

**Key Methods:**
- `count()` - Get the count argument (e.g., `3j` for 3 lines)
- `register()` - Get the register argument (e.g., `"a`)
- `has_bang()` - Check if the bang modifier is set (e.g., `:q!`)
- `string(name)` - Get a string/filepath/motion argument by name
- `range()` - Get a line range argument
- `buffer_id()` - Get the active buffer ID
- `cursor_position()` - Get the cursor position from the active window
- `mode_name()` - Get the current mode name
- `is_operator_pending()` - Check if in operator-pending mode
- `is_linewise()` - Check if this is a linewise operation
- `char(name)` - Get a single character argument
- `vfs()` - Get the VFS driver for file operations
- `set(name, value)` / `get(name)` - Generic argument access

## CommandResult

Results returned by command execution, defined in `ext/server/drivers/command-types/src/result.rs`:

```rust
pub enum CommandResult {
    Success,
    Error(String),
    Quit,
    ForceQuit,
    Detach,
}
```

**Result Handling:**
| Result | Runtime Action |
|--------|----------------|
| `Success` | Command executed successfully |
| `Error(msg)` | Command failed with an error message |
| `Quit` | Command requests editor to quit |
| `ForceQuit` | Command requests editor to quit without saving |
| `Detach` | Command requests client to detach (server continues running) |

**Helper Methods:**
- `is_success()` - Check if the result is success
- `is_error()` - Check if the result is an error
- `is_quit()` - Check if the result requests quit (matches both `Quit` and `ForceQuit`)
- `is_detach()` - Check if the result requests detach
- `error(msg)` - Factory method to create an error result

## CommandHandlerStore

Collects command handlers from modules during bootstrap:

```rust
pub struct CommandHandlerStore { /* ... */ }

impl CommandHandlerStore {
    pub fn new() -> Self;
    pub fn add(&self, handler: Box<dyn CommandHandler>);
    pub fn take_handlers(&self) -> Vec<Box<dyn CommandHandler>>;
}
```

Modules add their command handlers during initialization. The server takes
all handlers at startup to build the command dispatch system.

## CommandId

Module-scoped identifiers for commands:

```rust
use reovim_kernel::api::v1::{CommandId, ModuleId};

// Create a command ID scoped to a module
let id = CommandId::new(ModuleId::new("vim"), "enter-insert-mode");
assert_eq!(id.name(), "enter-insert-mode");
assert_eq!(id.module().as_str(), "vim");
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

### Window Operations (19)

Window commands are triggered with `<C-w>` prefix. Implemented in `server/modules/window-ops/`.

**Focus Navigation:**
| Command | Key | Description |
|---------|-----|-------------|
| `focus_left` | `<C-w>h` | Focus window to the left |
| `focus_down` | `<C-w>j` | Focus window below |
| `focus_up` | `<C-w>k` | Focus window above |
| `focus_right` | `<C-w>l` | Focus window to the right |

**Focus Cycling:**
| Command | Key | Description |
|---------|-----|-------------|
| `focus_next` | `<C-w>w` | Focus next window |
| `focus_prev` | `<C-w>W` | Focus previous window |

**Splitting:**
| Command | Key | Description |
|---------|-----|-------------|
| `split_horizontal` | `<C-w>s` | Split window horizontally |
| `split_vertical` | `<C-w>v` | Split window vertically |
| `split_new` | `<C-w>n` | New window with empty buffer |

**Closing:**
| Command | Key | Description |
|---------|-----|-------------|
| `close` | `<C-w>c` | Close current window |
| `close_others` | `<C-w>o` | Close all other windows |

**Resizing:**
| Command | Key | Description |
|---------|-----|-------------|
| `increase_height` | `<C-w>+` | Increase window height |
| `decrease_height` | `<C-w>-` | Decrease window height |
| `increase_width` | `<C-w>>` | Increase window width |
| `decrease_width` | `<C-w><` | Decrease window width |
| `equalize` | `<C-w>=` | Equalize all window sizes |

**Float Zone:**
| Command | Key | Description |
|---------|-----|-------------|
| `toggle_float` | `<C-w>f` | Toggle floating mode |
| `raise_float` | `<C-w>]` | Raise floating window |
| `lower_float` | `<C-w>[` | Lower floating window |

**Planned (Not Yet Implemented):**
- `<C-w>_` / `<C-w>|` - Maximize height/width
- `<C-w>H/J/K/L` - Move window to edge
- `<C-w>r/R/x` - Rotate/swap windows
- `<C-w>T` - Move to new tab


## Ex-Commands

Ex-commands (`:w`, `:q`, `:e`, etc.) use a trait-based dispatch system defined in
`ext/server/drivers/command/src/ex_handler.rs` and `ext/server/drivers/command/src/ex_dispatch.rs`.

### ExCommandHandler Trait

Modules implement this trait to define ex-command behavior:

```rust
pub trait ExCommandHandler: Send + Sync {
    fn id(&self) -> &'static str;
    fn names(&self) -> &[&'static str];
    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), ExCommandError>;
    fn complete(&self, _partial: &str) -> Vec<String> { vec![] }
    fn help(&self) -> &'static str { "" }
}
```

### ExCommandContext

Context passed to ex-command execution:

```rust
pub struct ExCommandContext<'a> {
    pub kernel: &'a KernelContext,
    pub buffer_id: Option<BufferId>,
    pub window_id: Option<WindowId>,
    pub bang: bool,
    pub range: Option<ExCommandRange>,
    pub vfs: Option<Arc<dyn VfsDriver>>,
}
```

### ExCommandRegistry

The `ExCommandRegistry` stores handlers indexed by name and provides dispatch:

```rust
// Dispatch flow: parse command line, find handler, execute
registry.dispatch("w filename.txt", &kernel, &ctx) // -> ExCommandResult::Success
registry.dispatch("q!", &kernel, &ctx)              // -> parses bang, executes quit
registry.dispatch("unknown", &kernel, &ctx)          // -> ExCommandResult::NotFound
```

`ExCommandResult` has three variants: `Success`, `NotFound(String)`, `Error(String)`.

### Module-Registered Ex-Commands

Modules register ex-command handlers via `ExCommandHandlerStore` during initialization.
The server collects all handlers and builds the `ExCommandRegistry` at startup.

**For module authors:**

See [Module System](../architecture/modules/overview.md) for complete documentation on
implementing `ExCommandHandler` for custom ex-commands.

## Command Flow

```
1. KeyEvent received
       |
       v
2. Keymap module resolves key to CommandId
       |
       v
3. Look up CommandHandler by CommandId
       |
       v
4. Build CommandContext with count, register, buffer_id, cursor, etc.
       |
       v
5. Call handler.execute(&mut runtime, &args)
       |
       v
6. Process CommandResult:
       |
       +-- Success   -> command completed, accumulate state changes
       +-- Error(msg) -> display error message
       +-- Quit       -> request editor quit
       +-- ForceQuit  -> request quit without saving
       +-- Detach     -> disconnect client, server continues
```

## Adding New Commands

1. Create a command struct implementing `Command` and `CommandHandler`
2. Register the handler via `CommandHandlerStore` in your module's initialization
3. Add key binding in the keymap module

Example:

```rust
use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult, ArgSpec, ArgKind};
use reovim_driver_session::SessionRuntime;
use reovim_kernel::api::v1::{CommandId, ModuleId};

const MY_MODULE: ModuleId = ModuleId::new_const("my-module");

pub struct MyCommand;

impl Command for MyCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MY_MODULE, "my-command")
    }

    fn description(&self) -> &'static str {
        "Does something useful"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Repeat count")]
    }
}

impl CommandHandler for MyCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        // Use runtime APIs to perform the action...
        CommandResult::Success
    }
}

// In module initialization:
handler_store.add(Box::new(MyCommand));
```

