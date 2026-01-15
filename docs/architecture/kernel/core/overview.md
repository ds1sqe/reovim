# core/ - Core Primitives

Editor-specific types that don't fit elsewhere.

## Source Location

`lib/kernel/src/core/`

## Key Types

```rust
pub enum Motion {
    Left, Right, Up, Down,
    WordStart, WordEnd,
    LineStart, LineEnd,
    // ...
}

pub enum TextObject {
    Word, WORD,
    Paragraph,
    Block(char, char),  // e.g., '(', ')'
    // ...
}
```

## Motion

Cursor movement types (mechanism only, no keybindings):

| Motion | Description |
|--------|-------------|
| `Left/Right/Up/Down` | Character/line movement |
| `WordStart/WordEnd` | Word boundaries |
| `LineStart/LineEnd` | Line boundaries |
| `DocumentStart/End` | Document boundaries |

## TextObject

Selection regions (mechanism only, no keybindings):

| TextObject | Description |
|------------|-------------|
| `Word` | Word under cursor |
| `Paragraph` | Paragraph block |
| `Block(c1, c2)` | Between delimiters |

## Mode System

```rust
pub struct ModeId {
    module: ModuleId,
    name: String,
}

pub struct ModeStack {
    stack: Vec<ModeId>,
}
```

## API Exports

```rust
use reovim_kernel::api::v1::{
    Motion, TextObject, MotionEngine, TextObjectEngine,
    Mode, ModeId, ModeStack, CommandId,
    RegisterBank, MarkBank, Jumplist,
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Module-Mode Inheritance](../../modules/mode-inheritance.md) - Mode system
- [Text Objects Reference](../../user-guide/text-objects.md) - Text object usage
