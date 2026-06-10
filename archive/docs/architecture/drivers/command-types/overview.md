# command-types/ - Command Types Driver

Shared types for the command system.

## Source Location

`ext/server/drivers/command-types/src/`

## Purpose

Provides the fundamental types for the command system. These types are extracted
to a separate crate to break the circular dependency between `reovim-driver-command`
and `reovim-driver-session`. Both drivers can depend on this crate without creating
a cycle.

## Key Types

### CommandContext

Carries all inputs for command execution:

```rust
pub struct CommandContext {
    pub buffer_id: Option<BufferId>,
    pub cursor: Position,
    pub count: Option<usize>,
    pub register: Option<char>,
    pub motion: Option<Motion>,
    pub text_object: Option<TextObject>,
    // ...
}
```

### CommandResult

Result of command execution:

```rust
pub struct CommandResult {
    pub success: bool,
    pub message: Option<String>,
    pub cursor_moved: bool,
    pub buffer_modified: bool,
    // ...
}
```

### MotionType

Motion classification for operator-pending mode:

```rust
pub enum MotionType {
    Characterwise,
    Linewise,
    Blockwise,
}
```

### Argument Specifications

For command argument handling:

```rust
pub enum ArgKind {
    String,
    Number,
    Boolean,
    Path,
    // ...
}

pub struct ArgSpec {
    pub name: String,
    pub kind: ArgKind,
    pub required: bool,
    pub default: Option<ArgValue>,
}

pub enum ArgValue {
    String(String),
    Number(i64),
    Boolean(bool),
    // ...
}
```

## Dependencies

- `reovim_kernel::api::v1` - Position, BufferId, Motion, TextObject

## Related Documents

- [Driver Overview](../overview.md)
- [command Driver](../command/overview.md)
