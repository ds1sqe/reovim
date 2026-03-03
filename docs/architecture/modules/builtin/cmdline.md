# cmdline Module

Command-line mode state and bridge for gRPC serialization.

## Source Location

`server/modules/cmdline/src/`

## Purpose

Owns all command-line mode state and behavior. Following the mechanism vs policy
principle, this module was moved from the driver layer (#468) because it defines
HOW command-line mode behaves (POLICY). The driver layer only provides the trait
contracts (`SessionExtension`, `ExtensionStateBridge`) as MECHANISM.

The module registers `CmdlineBridge` via the `BridgeProvider` service during
`init()`, enabling generic bridge detection without hardcoded server/driver code.

## Key Features

- Per-client command-line state (`CmdlineState` as a `SessionExtension`)
- Three prompt types: `:` (ex-command), `/` (forward search), `?` (backward search)
- Input buffer with cursor movement (left, right, home, end)
- Editing operations (backspace, delete, delete-word-back, delete-to-start)
- Separate command and search history with deduplication (max 100 entries)
- Tab completion cycling (next/previous) with prefix tracking
- JSON bridge (`CmdlineBridge`) for gRPC state transmission to clients
- `TextInputSink` implementation for character routing from the session layer

## Key Types

```rust
/// Prompt type enum (`:`, `/`, `?`)
pub enum CmdlinePrompt {
    Command,        // `:` - ex command prompt (default)
    SearchForward,  // `/` - forward search prompt
    SearchBackward, // `?` - backward search prompt
}

/// Per-client session extension for command-line state
pub struct CmdlineState {
    active: bool,
    prompt: CmdlinePrompt,
    cancelled: bool,
    input: String,
    cursor: usize,
    command_history: Vec<String>,
    search_history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
    completions: Vec<String>,
    completion_index: Option<usize>,
    completion_prefix: String,
}

impl SessionExtension for CmdlineState { /* ... */ }
impl TextInputSink for CmdlineState { /* ... */ }

/// Bridge that serializes CmdlineState to JSON for gRPC
pub struct CmdlineBridge;

impl ExtensionStateBridge for CmdlineBridge {
    fn kind(&self) -> &'static str { "cmdline" }
    fn scope(&self) -> ExtensionScope { ExtensionScope::Client }
}

/// Module entry point
pub struct CmdlineModule;

impl Module for CmdlineModule {
    fn id(&self) -> ModuleId { ModuleId::new("cmdline") }
    fn name(&self) -> &'static str { "cmdline" }
    fn version(&self) -> Version { Version::new(0, 1, 0) }
}
```

## Bridge JSON Schema

The `CmdlineBridge` snapshot produces the following JSON structure:

| Field | Type | Description |
|-------|------|-------------|
| `active` | bool | Whether command-line mode is active |
| `prompt` | string | Prompt character (`":"`, `"/"`, or `"?"`) |
| `input` | string | Current input buffer text |
| `cursor` | number | Cursor position within input |
| `completions` | string[] | Available completion candidates |
| `completion_index` | number/null | Currently selected completion index |

## Dependencies

- `reovim_kernel::api::v1` - Module trait, ModuleContext, ServiceRegistry
- `reovim_driver_session` - SessionExtension, TextInputSink, BridgeProvider, ExtensionStateBridge
- `serde_json` - JSON serialization for bridge snapshots

## Related Documents

- [Module System Overview](../overview.md)
- [Commands Module](./commands.md) - Ex-commands that consume cmdline input
- [Mode Manager Module](./mode-manager.md) - Mode transitions including Command mode
