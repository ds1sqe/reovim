# editor Module

Core editor command provider.

## Source Location

`ext/server/modules/editor/src/`

## Purpose

The editor module provides core editing commands including cursor movement, line operations, and mode-aware text manipulation. It registers commands like `editor:cursor-up`, `editor:cursor-down`, `editor:line-start`, `editor:line-end`, `editor:delete-char`, etc.

## Key Features

- Core cursor movement commands
- Line editing operations (delete char, delete to EOL)
- Mode-aware character input handling
- Basic editing primitives used by other modules

## Dependencies

- `reovim_kernel::api::v1` - Module trait, command registration

## Related Documents

- [Module System Overview](../overview.md)
- [Keymap Module](./keymap.md) - Key sequence mapping
