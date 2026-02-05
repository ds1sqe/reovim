# commands Module

Ex-commands implementation (:w, :q, :e, :wq, :colorscheme, session commands).

## Source Location

`server/modules/commands/src/`

## Purpose

Implements the standard ex-commands (colon commands) for the editor:
- `:q` / `:quit` - Quit the editor
- `:w` / `:write` - Write the buffer to disk
- `:wq` - Write and quit
- `:e` / `:edit` - Edit a file
- `:colorscheme` - Change theme
- `:detach` - Detach client from session
- `:servers` - List running servers
- `:kill-server` - Terminate server

Following mechanism vs policy:
- **Mechanism (Kernel)**: Buffer management, position types
- **Policy (This Module)**: `ExCommandHandler` trait, what commands do

## Key Types

```rust
/// Trait for implementing ex-commands
pub trait ExCommandHandler: Send + Sync {
    fn id(&self) -> &'static str;
    fn names(&self) -> Vec<&'static str>;
    fn execute(&self, ctx: &ExCommandContext) -> Result<(), CommandError>;
}

/// Context passed to command execution
pub struct ExCommandContext {
    pub range: Option<Range>,
    pub args: Vec<String>,
    pub bang: bool,
    // ...
}

/// Commands module instance
pub struct CommandsModule;

impl Module for CommandsModule {
    fn id(&self) -> ModuleId { ModuleId::new("commands") }
    fn name(&self) -> &'static str { "Ex-Commands" }
}
```

## Available Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `edit` | `e` | Open file for editing |
| `quit` | `q` | Quit editor |
| `write` | `w` | Write buffer to disk |
| `write-quit` | `wq` | Write and quit |
| `colorscheme` | - | Change color theme |
| `detach` | - | Detach client from session |
| `servers` | - | List running servers |
| `kill-server` | - | Terminate server |

## Dependencies

- `reovim_kernel::api::v1` - Module trait
- `reovim_driver_command::ExCommandHandlerStore` - Command registration

## Example Usage

```rust
use reovim_module_commands::{commands, ExCommandHandler};

// Get all registered commands
let cmds = commands();
for cmd in &cmds {
    println!("{}: {:?}", cmd.id(), cmd.names());
}
```

## Related Documents

- [Module System Overview](../overview.md)
- [User Commands Guide](../../../user-guide/commands.md)
