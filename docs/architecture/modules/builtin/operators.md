# operators Module

Vim-style operators (delete, yank, change).

## Source Location

`modules/operators/src/`

## Purpose

The operators module implements vim's operator-pending mode commands:

- `d{motion}` - Delete
- `y{motion}` - Yank (copy)
- `c{motion}` - Change (delete + insert)

## Key Features

### Operator Pattern

```rust
pub trait Operator: Send + Sync {
    fn execute(&self, ctx: &mut KernelContext, region: TextRegion) -> OperatorResult;
}
```

Operators work with motions and text objects:

```
d     +     w      →   Delete word
↓           ↓
operator    motion
```

### Built-in Operators

| Operator | Key | Description |
|----------|-----|-------------|
| Delete | `d` | Delete region, save to register |
| Yank | `y` | Copy region to register |
| Change | `c` | Delete region, enter insert mode |

## Dependencies

- `reovim-kernel` - Register bank, text objects

## Exports

```rust
pub use delete::DeleteOperator;
pub use yank::YankOperator;
pub use change::ChangeOperator;
```

## Related Documents

- [Module System Overview](../overview.md)
- [Core Primitives](../../kernel/core/overview.md) - TextObject, Motion
- [Text Objects Reference](../../user-guide/text-objects.md)
