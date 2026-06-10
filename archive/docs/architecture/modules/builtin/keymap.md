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

### Keymap Trie

The keymap uses a trie structure for efficient prefix matching. Keys accumulate until:
- A complete binding is matched → Execute command
- No prefix matches → Clear buffer, report unbound
- Partial match → Wait for more keys

## Dependencies

- `reovim-driver-input` - Key event types
- `reovim_kernel::api::v1` - Module trait, keybinding registration

## Related Documents

- [Module System Overview](../overview.md)
- [Input Driver](../../drivers/input/overview.md) - Key event types
