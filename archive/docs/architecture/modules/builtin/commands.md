# commands Module

Ex-commands implementation (:w, :q, :e, :wq, :colorscheme, session commands).

## Source Location

`ext/server/modules/commands/src/`

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
- **Mechanism (Driver)**: `ExCommandHandler` trait definition (`reovim-driver-command`)
- **Policy (This Module)**: Implements `ExCommandHandler` for each command

## Key Types

```rust
/// Trait for implementing ex-commands (defined in reovim-driver-command)
pub trait ExCommandHandler: Send + Sync {
    fn id(&self) -> &'static str;
    fn names(&self) -> &[&'static str];
    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str])
        -> Result<(), ExCommandError>;
    fn complete(&self, _partial: &str) -> Vec<String> { vec![] }
    fn help(&self) -> &'static str { "" }
}

/// Context passed to command execution
pub struct ExCommandContext<'a> {
    pub kernel: &'a KernelContext,
    pub buffer_id: Option<BufferId>,
    pub window_id: Option<WindowId>,
    pub bang: bool,
    pub range: Option<ExCommandRange>,
    pub vfs: Option<Arc<dyn VfsDriver>>,
}

/// Text range for command execution (line-based)
pub struct ExCommandRange {
    pub start: Position,
    pub end: Position,
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

## CommandQueryService (gRPC)

The `CommandService` gRPC service (defined in `uapi/protocol/proto/reovim/v2/command.proto`)
provides command-line tab completion and discovery for clients:

| RPC | Purpose |
|-----|---------|
| `SearchCommands` | Search commands by name prefix for tab completion |
| `CompleteArgs` | Complete arguments for a specific user command |

`SearchCommands` accepts a `CommandSource` filter to narrow results to user commands
(`COMMAND_SOURCE_USER`), keybinding commands (`COMMAND_SOURCE_KEYBINDING`), or both
(`COMMAND_SOURCE_ALL`). It returns matching `UserCommandEntry` and
`KeybindingCommandEntry` records containing id, names/aliases, and help text.

`CompleteArgs` delegates to the handler's `complete()` method, enabling per-command
argument suggestions (e.g., file paths for `:e`, theme names for `:colorscheme`).

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
