# mm/ - Memory Management

Buffer storage and position tracking.

## Source Location

`lib/kernel/src/mm/`

## Key Types

```rust
pub struct Buffer {
    lines: Vec<Line>,
    cursor: Position,
    // ...
}

pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub struct Edit {
    pub range: Range,
    pub text: String,
}
```

## Type Reference

| Type | Purpose |
|------|---------|
| `Buffer` | Text storage with lines |
| `Position` | Line + column coordinate |
| `Range` | Start + end positions |
| `Edit` | Text modification operation |
| `Line` | Single line of text |
| `Selection` | Selection region with mode |
| `SelectionMode` | Character, Line, or Block selection |
| `BufferSnapshot` | Immutable buffer state for rendering |

## API Exports

These types are re-exported via `reovim_kernel::api::v1`:

```rust
use reovim_kernel::api::v1::{
    Buffer, BufferId, Position, Edit, Cursor, WindowId,
    Selection, SelectionMode, BufferSnapshot,
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Block Operations](../block/overview.md) - Undo/redo for buffer edits
