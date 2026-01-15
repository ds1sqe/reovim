# editor Module

Core editing fallback handler.

## Source Location

`modules/editor/src/`

## Purpose

The editor module provides fallback handling for unmatched key events. When no other module handles a key, the editor module processes it as:

- Character insertion in insert mode
- Command feedback in normal mode

## Key Features

- Fallback key handling
- Character insertion
- Mode-aware behavior

## Dependencies

None (base module)

## Exports

```rust
// Fallback handler
pub struct EditorFallbackHandler;
```

## Related Documents

- [Module System Overview](../overview.md)
- [Keymap Module](./keymap.md) - Key sequence mapping
