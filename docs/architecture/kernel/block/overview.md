# block/ - Block Operations

Undo/redo and transactional editing.

## Source Location

`lib/kernel/src/block/`

## Key Types

```rust
pub struct UndoTree {
    nodes: Vec<UndoNode>,
    current: NodeId,
}

pub struct Transaction {
    edits: Vec<Edit>,
    // Atomic group of edits
}
```

## UndoTree

Non-linear undo history (like Vim's undotree):

```
       [1]
       / \
     [2] [3]
     /     \
   [4]     [5] ← current
```

Unlike linear undo (stack-based), UndoTree preserves all branches. Users can navigate to any previous state.

## Transaction

Atomic edit groups:

```rust
let txn = Transaction::new();
txn.add(Edit::insert(pos, "hello"));
txn.add(Edit::delete(range));
buffer.apply(txn);  // All or nothing
```

Transactions ensure that multi-edit operations (like search-replace) are undone as a single unit.

## API Exports

```rust
use reovim_kernel::api::v1::{
    UndoTree, Transaction, History,
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Memory Management](../mm/overview.md) - Buffer, Edit types
