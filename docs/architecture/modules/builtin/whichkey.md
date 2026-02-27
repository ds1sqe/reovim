# whichkey Module

Which-key popup showing available keybinding completions.

## Source Location

`server/modules/whichkey/src/`

## Purpose

Shows available keybinding completions when the user has a pending key sequence
(e.g., after pressing `g` in normal mode). Added in #468.

This module reads `PendingBindings` from `reovim-driver-input` -- a generic
session extension populated by the session layer after any key resolver returns
`Pending`. It has zero dependency on vim and works with any editor module that
uses the keymap system.

The bridge is registered via `BridgeProvider` during `init()`, following the
same pattern as the cmdline module.

## Key Features

- Displays accumulated pending key prefix and available continuations
- Generic design with no vim-specific dependencies
- Combines `mode_prefix` (e.g., operator `d`) and `pending_keys` (e.g., `i`) into a unified display prefix
- Returns inactive state on deactivation so clients can clear the popup
- Client-scoped bridge (each client sees its own pending key state)

## Key Types

```rust
/// Bridge for which-key popup state
pub struct WhichKeyBridge;

impl ExtensionStateBridge for WhichKeyBridge {
    fn kind(&self) -> &'static str { "whichkey" }
    fn scope(&self) -> ExtensionScope { ExtensionScope::Client }
}

/// Module entry point
pub struct WhichKeyModule;

impl Module for WhichKeyModule {
    fn id(&self) -> ModuleId { ModuleId::new("whichkey") }
    fn name(&self) -> &'static str { "whichkey" }
    fn version(&self) -> Version { Version::new(0, 1, 0) }
}
```

The `PendingBindings` type (from `reovim-driver-input`) provides:

```rust
/// Extension populated by the session layer after a resolver returns Pending
pub struct PendingBindings {
    pub mode_prefix: KeySequence,   // Operator prefix (e.g., "d")
    pub pending_keys: KeySequence,  // Accumulated keys (e.g., "i")
    pub continuations: Vec<(KeySequence, CommandId)>,  // Available next keys
}
```

## Bridge JSON Schema

The `WhichKeyBridge` snapshot produces the following JSON structure:

| Field | Type | Description |
|-------|------|-------------|
| `active` | bool | Whether there are pending bindings to show |
| `prefix` | string | Combined `mode_prefix` + `pending_keys` display string |
| `hints` | object[] | Array of `{ key, command }` for available continuations |

Example snapshot for `g` pressed in normal mode:

```json
{
  "active": true,
  "prefix": "g",
  "hints": [
    { "key": "g", "command": "test:goto-top" },
    { "key": "d", "command": "test:goto-definition" }
  ]
}
```

## Dependencies

- `reovim_kernel::api::v1` - Module trait, ModuleContext, CommandId, ModuleId
- `reovim_driver_session` - BridgeProvider, ExtensionStateBridge, ExtensionMap
- `reovim_driver_input` - PendingBindings, KeySequence
- `serde_json` - JSON serialization for bridge snapshots

## Related Documents

- [Module System Overview](../overview.md)
- [Keymap Module](./keymap.md) - Key sequence resolution that triggers pending state
- [Cmdline Module](./cmdline.md) - Sister bridge module using same BridgeProvider pattern
