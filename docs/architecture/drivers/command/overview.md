# command/ - Command Driver

Command traits and execution infrastructure.

## Source Location

`lib/drivers/command/src/`

## Key Traits

```rust
/// Command metadata (identity, description, arguments)
pub trait Command {
    fn id(&self) -> CommandId;
    fn description(&self) -> &'static str;
    fn args(&self) -> Vec<ArgSpec>;
}

/// Command execution (takes KernelContext)
pub trait CommandHandler: Command + Send + Sync {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult;
}
```

## Argument System

```rust
/// Argument specification
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    pub description: &'static str,
    pub required: bool,
}

pub enum ArgKind {
    Count,     // Numeric count (e.g., 3j)
    Register,  // Register name (e.g., "a)
    Motion,    // Motion argument
    String,    // String argument
}

/// Command execution context (parsed arguments)
pub struct CommandContext {
    count: Option<usize>,
    register: Option<char>,
    // ...
}
```

## Command Result

```rust
pub enum CommandResult {
    Success,
    Error(String),
    Quit,
    ForceQuit,
}
```

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
- [Commands Reference](../../user-guide/commands.md) - Command system usage
