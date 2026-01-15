# keymap Module

Key sequence mapping and interactor system.

## Source Location

`modules/keymap/src/`

## Purpose

The keymap module maps key sequences to commands using an interactor pattern. It supports:

- Single key bindings (`j`, `k`)
- Multi-key sequences (`gg`, `dd`)
- Modifier combinations (`<C-w>h`)
- Operator-pending sequences (`d{motion}`)

## Key Features

### Interactor Pattern

```rust
pub struct Interactor<K, V> {
    // Keymap trie for efficient prefix matching
    keymap: Keymap<K, V>,
    // Current input sequence
    buffer: Vec<K>,
}
```

The interactor accumulates keys until:
- A complete binding is matched → Execute command
- No prefix matches → Clear buffer, report unbound
- Partial match → Wait for more keys

### Key Sequence Parsing

```rust
// Parse vim-style key notation
KeySequence::parse("<C-w>h")  // Ctrl+W then H
KeySequence::parse("gg")      // Two G's
KeySequence::parse("<Esc>")   // Escape key
```

## Dependencies

- `reovim-driver-input` - Key event types

## Exports

```rust
pub use interactor::{Interactor, InteractorConfig, InteractorResponse};
pub use keymap::Keymap;
```

## Related Documents

- [Module System Overview](../overview.md)
- [Input Driver](../../drivers/input/overview.md) - Key event types
